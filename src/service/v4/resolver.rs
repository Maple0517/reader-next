use crate::service::v4::extractor::{self, Observation, RiskLevel};
use crate::storage::db::v4::entity_repo::{EntityRecord, EntityRepo};

/// Resolution result for a mention.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// Mention matched an existing entity.
    MatchExisting { entity_id: String },
    /// No match found; should create a new entity.
    CreateNew,
    /// Multiple candidates found; needs disambiguation.
    Ambiguous { candidates: Vec<String> },
    /// Unable to determine; needs more context.
    Uncertain,
}

/// A fully resolved observation with entity references and risk classification.
#[derive(Debug, Clone)]
pub struct ResolvedObservation {
    pub observation: Observation,
    pub subject_entity_id: Option<String>,
    pub object_entity_id: Option<String>,
    pub resolved_dimension_key: Option<String>,
    pub risk_level: RiskLevel,
    pub resolution: Resolution,
}

/// Resolves observations against candidate entities.
///
/// For each observation:
/// - EntityIntroduction: match by alias/name -> MatchExisting or CreateNew
/// - Alias: find entity by alias -> MatchExisting or Ambiguous
/// - PropertyUpdate: find entity + dimension -> MatchExisting + resolved_dimension_key
/// - MinorEvent/Summary: pass through with Uncertain
pub async fn resolve(
    observations: &[Observation],
    entity_repo: &EntityRepo,
    book_id: &str,
) -> anyhow::Result<Vec<ResolvedObservation>> {
    let mut results = Vec::new();

    for obs in observations {
        let risk_level = extractor::classify_risk(obs);

        let resolved = match obs {
            Observation::EntityIntroduction {
                subject_mention,
                aliases,
                ..
            } => {
                // Try to match by alias first, then by canonical name
                // Try aliases from the observation itself
                let mut matched =
                    find_entity_for_mention(entity_repo, book_id, subject_mention).await;
                if matched.is_none() {
                    for alias in aliases {
                        matched = find_entity_for_mention(entity_repo, book_id, alias).await;
                        if matched.is_some() {
                            break;
                        }
                    }
                }

                match matched {
                    Some(entity) => ResolvedObservation {
                        observation: obs.clone(),
                        subject_entity_id: Some(entity.id.clone()),
                        object_entity_id: None,
                        resolved_dimension_key: None,
                        risk_level,
                        resolution: Resolution::MatchExisting {
                            entity_id: entity.id,
                        },
                    },
                    None => ResolvedObservation {
                        observation: obs.clone(),
                        subject_entity_id: None,
                        object_entity_id: None,
                        resolved_dimension_key: None,
                        risk_level,
                        resolution: Resolution::CreateNew,
                    },
                }
            }

            Observation::Alias {
                subject_mention, ..
            } => {
                let entity = find_entity_for_mention(entity_repo, book_id, subject_mention).await;
                match entity {
                    Some(e) => ResolvedObservation {
                        observation: obs.clone(),
                        subject_entity_id: Some(e.id.clone()),
                        object_entity_id: None,
                        resolved_dimension_key: None,
                        risk_level,
                        resolution: Resolution::MatchExisting { entity_id: e.id },
                    },
                    None => {
                        // Try to find candidates for ambiguity reporting
                        let candidates = entity_repo.list_by_book(book_id).await?;
                        if candidates.is_empty() {
                            ResolvedObservation {
                                observation: obs.clone(),
                                subject_entity_id: None,
                                object_entity_id: None,
                                resolved_dimension_key: None,
                                risk_level,
                                resolution: Resolution::Uncertain,
                            }
                        } else {
                            let ids: Vec<String> =
                                candidates.iter().take(3).map(|e| e.id.clone()).collect();
                            ResolvedObservation {
                                observation: obs.clone(),
                                subject_entity_id: None,
                                object_entity_id: None,
                                resolved_dimension_key: None,
                                risk_level,
                                resolution: Resolution::Ambiguous { candidates: ids },
                            }
                        }
                    }
                }
            }

            Observation::PropertyUpdate {
                subject_mention,
                dimension_key,
                ..
            } => {
                let entity = find_entity_for_mention(entity_repo, book_id, subject_mention).await;
                match entity {
                    Some(e) => ResolvedObservation {
                        observation: obs.clone(),
                        subject_entity_id: Some(e.id.clone()),
                        object_entity_id: None,
                        resolved_dimension_key: Some(dimension_key.clone()),
                        risk_level,
                        resolution: Resolution::MatchExisting { entity_id: e.id },
                    },
                    None => ResolvedObservation {
                        observation: obs.clone(),
                        subject_entity_id: None,
                        object_entity_id: None,
                        resolved_dimension_key: Some(dimension_key.clone()),
                        risk_level,
                        resolution: Resolution::Uncertain,
                    },
                }
            }

            Observation::RelationshipUpdate {
                subject_mention,
                object_mention,
                ..
            } => {
                // Subject: best-effort resolve to character entity
                let subject_entity =
                    find_entity_for_mention(entity_repo, book_id, subject_mention).await;

                // Object: best-effort resolve to any entity (character or non-character)
                // object_entity_id can be Some(character), Some(non-character), or None
                let object_entity =
                    find_entity_for_mention(entity_repo, book_id, object_mention).await;

                let (subject_id, resolution) = match subject_entity {
                    Some(e) => (
                        Some(e.id.clone()),
                        Resolution::MatchExisting { entity_id: e.id },
                    ),
                    None => (None, Resolution::CreateNew),
                };

                ResolvedObservation {
                    observation: obs.clone(),
                    subject_entity_id: subject_id,
                    object_entity_id: object_entity.map(|e| e.id),
                    resolved_dimension_key: None,
                    risk_level,
                    resolution,
                }
            }

            Observation::IdentityReveal {
                revealed_mention,
                canonical_mention,
                ..
            } => {
                let (subject_id, object_id, resolution) = resolve_identity_pair(
                    entity_repo,
                    book_id,
                    revealed_mention,
                    canonical_mention,
                )
                .await;

                ResolvedObservation {
                    observation: obs.clone(),
                    subject_entity_id: subject_id,
                    object_entity_id: object_id,
                    resolved_dimension_key: None,
                    risk_level,
                    resolution,
                }
            }

            Observation::EntityMergeCandidate {
                entity_a_mention,
                entity_b_mention,
                ..
            }
            | Observation::EntitySplitCandidate {
                entity_a_mention,
                entity_b_mention,
                ..
            }
            | Observation::NotSameIdentity {
                entity_a_mention,
                entity_b_mention,
                ..
            } => {
                let (subject_id, object_id, resolution) =
                    resolve_identity_pair(entity_repo, book_id, entity_a_mention, entity_b_mention)
                        .await;

                ResolvedObservation {
                    observation: obs.clone(),
                    subject_entity_id: subject_id,
                    object_entity_id: object_id,
                    resolved_dimension_key: None,
                    risk_level,
                    resolution,
                }
            }

            Observation::KnowledgeAssertion {
                referenced_entity_mentions,
                ..
            } => {
                let mut updated = obs.clone();
                if let Observation::KnowledgeAssertion {
                    referenced_entity_mentions: updated_mentions,
                    ..
                } = &mut updated
                {
                    for (target, source) in updated_mentions
                        .iter_mut()
                        .zip(referenced_entity_mentions.iter())
                    {
                        target.resolved_entity_id =
                            find_entity_for_mention(entity_repo, book_id, &source.mention)
                                .await
                                .map(|entity| entity.id);
                    }
                }

                ResolvedObservation {
                    observation: updated,
                    subject_entity_id: None,
                    object_entity_id: None,
                    resolved_dimension_key: None,
                    risk_level,
                    resolution: Resolution::Uncertain,
                }
            }

            Observation::MinorEvent { .. } | Observation::Summary { .. } => ResolvedObservation {
                observation: obs.clone(),
                subject_entity_id: None,
                object_entity_id: None,
                resolved_dimension_key: None,
                risk_level,
                resolution: Resolution::Uncertain,
            },
        };

        results.push(resolved);
    }

    Ok(results)
}

/// Find an entity by alias or canonical name for a given mention.
async fn find_entity_for_mention(
    entity_repo: &EntityRepo,
    book_id: &str,
    mention: &str,
) -> Option<EntityRecord> {
    // 1. Try alias match
    if let Ok(Some(entity)) = entity_repo.find_entity_by_alias(book_id, mention).await {
        return Some(entity);
    }

    // 2. Try canonical name match
    if let Ok(Some(entity)) = entity_repo.get_by_canonical_name(book_id, mention).await {
        return Some(entity);
    }

    None
}

async fn resolve_identity_pair(
    entity_repo: &EntityRepo,
    book_id: &str,
    left_mention: &str,
    right_mention: &str,
) -> (Option<String>, Option<String>, Resolution) {
    let left = find_entity_for_mention(entity_repo, book_id, left_mention).await;
    let right = find_entity_for_mention(entity_repo, book_id, right_mention).await;

    let resolution = if let Some(entity) = left.as_ref() {
        Resolution::MatchExisting {
            entity_id: entity.id.clone(),
        }
    } else if let Some(entity) = right.as_ref() {
        Resolution::MatchExisting {
            entity_id: entity.id.clone(),
        }
    } else {
        Resolution::Uncertain
    };

    (left.map(|e| e.id), right.map(|e| e.id), resolution)
}

/// Retrieves candidate entities for a mention.
pub struct CandidateRetriever {
    entity_repo: EntityRepo,
}

impl CandidateRetriever {
    pub fn new(entity_repo: EntityRepo) -> Self {
        Self { entity_repo }
    }

    /// Retrieve candidate entities for a mention in a book.
    ///
    /// Strategy (in priority order):
    /// 1. Exact alias match
    /// 2. Normalized canonical name match
    /// 3. Recent active entities (by last_seen_chapter DESC)
    /// 4. High importance entities
    pub async fn retrieve(
        &self,
        book_id: &str,
        mention: &str,
    ) -> anyhow::Result<Vec<EntityRecord>> {
        // 1. Try exact alias match first
        if let Some(entity) = self
            .entity_repo
            .find_entity_by_alias(book_id, mention)
            .await?
        {
            return Ok(vec![entity]);
        }

        // 2. Try canonical name match
        if let Some(entity) = self
            .entity_repo
            .get_by_canonical_name(book_id, mention)
            .await?
        {
            return Ok(vec![entity]);
        }

        // 3. Try recent active entities (most recently seen first)
        let recent = self.entity_repo.list_recent_active(book_id).await?;
        if !recent.is_empty() {
            return Ok(recent);
        }

        // 4. Fall back to high importance entities
        let all = self.entity_repo.list_by_book(book_id).await?;
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use sqlx::SqlitePool;

    async fn setup() -> (SqlitePool, CandidateRetriever, EntityRepo) {
        let dir = std::env::temp_dir().join(format!("reader-v4-resolver-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        let entity_repo = EntityRepo::new(pool.clone());
        let retriever = CandidateRetriever::new(EntityRepo::new(pool.clone()));
        (pool, retriever, entity_repo)
    }

    #[tokio::test]
    async fn retrieve_by_alias_match() {
        let (_pool, retriever, entity_repo) = setup().await;
        let entity = entity_repo
            .create_entity("book1", "character", "张三", "张三", None, 0.5, 1)
            .await
            .unwrap();
        entity_repo
            .create_alias("book1", &entity.id, "小三", "nickname", 1, 0.8, None)
            .await
            .unwrap();

        let candidates = retriever.retrieve("book1", "小三").await.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, entity.id);
    }

    #[tokio::test]
    async fn retrieve_by_canonical_name() {
        let (_pool, retriever, entity_repo) = setup().await;
        let entity = entity_repo
            .create_entity("book1", "character", "李四", "李四", None, 0.5, 1)
            .await
            .unwrap();

        let candidates = retriever.retrieve("book1", "李四").await.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, entity.id);
    }

    #[tokio::test]
    async fn retrieve_no_match_returns_all_active() {
        let (_pool, retriever, entity_repo) = setup().await;
        entity_repo
            .create_entity("book1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        entity_repo
            .create_entity("book1", "character", "李四", "李四", None, 0.5, 1)
            .await
            .unwrap();

        let candidates = retriever.retrieve("book1", "不存在的人").await.unwrap();
        assert_eq!(candidates.len(), 2);
        // Both have same last_seen_chapter, so secondary sort is importance DESC
        assert!(candidates[0].importance_score >= candidates[1].importance_score);
    }

    #[tokio::test]
    async fn retrieve_no_match_recent_before_important() {
        let (_pool, retriever, entity_repo) = setup().await;
        // High importance entity seen in chapter 1
        let e_old = entity_repo
            .create_entity("book1", "character", "远古强者", "远古强者", None, 0.95, 1)
            .await
            .unwrap();
        // Lower importance entity seen in chapter 10
        let e_recent = entity_repo
            .create_entity("book1", "character", "新角色", "新角色", None, 0.3, 10)
            .await
            .unwrap();
        entity_repo
            .update_last_seen(&e_recent.id, 10)
            .await
            .unwrap();

        let candidates = retriever.retrieve("book1", "不存在的人").await.unwrap();
        assert_eq!(candidates.len(), 2);
        // Recent (last_seen_chapter=10) should come before important (last_seen_chapter=1)
        assert_eq!(
            candidates[0].id, e_recent.id,
            "recent entity should rank first"
        );
        assert_eq!(
            candidates[1].id, e_old.id,
            "old important entity should rank second"
        );
    }

    #[tokio::test]
    async fn retrieve_empty_book() {
        let (_pool, retriever, _entity_repo) = setup().await;
        let candidates = retriever.retrieve("empty_book", "任何人").await.unwrap();
        assert!(candidates.is_empty());
    }

    #[tokio::test]
    async fn retrieve_alias_takes_priority_over_canonical() {
        let (_pool, retriever, entity_repo) = setup().await;
        // Create two entities: one whose alias matches, one whose canonical matches
        let e1 = entity_repo
            .create_entity("book1", "character", "王五", "王五", None, 0.3, 1)
            .await
            .unwrap();
        entity_repo
            .create_alias("book1", &e1.id, "张三", "alias", 1, 0.7, None)
            .await
            .unwrap();
        let _e2 = entity_repo
            .create_entity("book1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        // "张三" matches both alias (e1) and canonical (e2); alias should win
        let candidates = retriever.retrieve("book1", "张三").await.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, e1.id, "alias match should take priority");
    }

    #[tokio::test]
    async fn retrieve_canonical_match_returns_one() {
        let (_pool, retriever, entity_repo) = setup().await;
        entity_repo
            .create_entity("book1", "character", "独孤求败", "独孤求败", None, 0.9, 1)
            .await
            .unwrap();
        entity_repo
            .create_entity("book1", "character", "东方不败", "东方不败", None, 0.8, 1)
            .await
            .unwrap();

        let candidates = retriever.retrieve("book1", "独孤求败").await.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].canonical_name, "独孤求败");
    }

    #[test]
    fn resolution_types() {
        let r1 = Resolution::MatchExisting {
            entity_id: "e1".to_string(),
        };
        let r2 = Resolution::CreateNew;
        let r3 = Resolution::Ambiguous {
            candidates: vec!["e1".to_string(), "e2".to_string()],
        };
        let r4 = Resolution::Uncertain;

        assert_ne!(r1, r2);
        assert_ne!(r2, r3);
        assert_ne!(r3, r4);
    }

    #[test]
    fn resolved_observation_construction() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "张三".to_string(),
            dimension_key: "realm".to_string(),
            value_text: Some("筑基".to_string()),
            value_json: None,
            evidence_span_ids: vec!["s1".to_string()],
            confidence: 0.8,
        };
        let risk = RiskLevel::Medium;
        let resolved = ResolvedObservation {
            observation: obs.clone(),
            subject_entity_id: Some("entity-1".to_string()),
            object_entity_id: None,
            resolved_dimension_key: Some("realm".to_string()),
            risk_level: risk,
            resolution: Resolution::MatchExisting {
                entity_id: "entity-1".to_string(),
            },
        };

        assert_eq!(resolved.subject_entity_id, Some("entity-1".to_string()));
        assert_eq!(resolved.risk_level, RiskLevel::Medium);
        assert_eq!(
            resolved.resolution,
            Resolution::MatchExisting {
                entity_id: "entity-1".to_string()
            }
        );
    }

    // --- RelationshipUpdate resolution tests ---

    fn make_relationship_obs(subject: &str, object: &str) -> Observation {
        Observation::RelationshipUpdate {
            subject_mention: subject.to_string(),
            object_mention: object.to_string(),
            relation_hint: "师徒".to_string(),
            relation_group: "mentorship".to_string(),
            relation_label: "师父".to_string(),
            directionality: "directed".to_string(),
            evidence_span_ids: vec!["s1".to_string()],
            confidence: 0.8,
            importance_hint: 0.7,
            is_long_term_or_significant_hint: true,
        }
    }

    #[tokio::test]
    async fn relationship_update_subject_resolves_to_character() {
        let (_pool, _retriever, entity_repo) = setup().await;
        let subject = entity_repo
            .create_entity("book1", "character", "张三", "张三", None, 0.8, 1)
            .await
            .unwrap();

        let obs = make_relationship_obs("张三", "李四");
        let results = resolve(&[obs], &entity_repo, "book1").await.unwrap();
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert_eq!(r.subject_entity_id, Some(subject.id.clone()));
        assert_eq!(
            r.resolution,
            Resolution::MatchExisting {
                entity_id: subject.id
            }
        );
    }

    #[tokio::test]
    async fn relationship_update_object_resolves_to_character() {
        let (_pool, _retriever, entity_repo) = setup().await;
        entity_repo
            .create_entity("book1", "character", "张三", "张三", None, 0.8, 1)
            .await
            .unwrap();
        let object = entity_repo
            .create_entity("book1", "character", "李四", "李四", None, 0.7, 1)
            .await
            .unwrap();

        let obs = make_relationship_obs("张三", "李四");
        let results = resolve(&[obs], &entity_repo, "book1").await.unwrap();
        let r = &results[0];
        assert_eq!(r.object_entity_id, Some(object.id));
    }

    #[tokio::test]
    async fn relationship_update_object_resolves_to_non_character() {
        let (_pool, _retriever, entity_repo) = setup().await;
        entity_repo
            .create_entity("book1", "character", "张三", "张三", None, 0.8, 1)
            .await
            .unwrap();
        let place = entity_repo
            .create_entity("book1", "place", "华山", "华山", None, 0.5, 1)
            .await
            .unwrap();

        let obs = make_relationship_obs("张三", "华山");
        let results = resolve(&[obs], &entity_repo, "book1").await.unwrap();
        let r = &results[0];
        // Object resolved to a non-character entity — still succeeds
        assert_eq!(r.object_entity_id, Some(place.id));
    }

    #[tokio::test]
    async fn relationship_update_object_unresolved_is_ok() {
        let (_pool, _retriever, entity_repo) = setup().await;
        entity_repo
            .create_entity("book1", "character", "张三", "张三", None, 0.8, 1)
            .await
            .unwrap();

        let obs = make_relationship_obs("张三", "路人甲");
        let results = resolve(&[obs], &entity_repo, "book1").await.unwrap();
        let r = &results[0];
        // Object unresolved — object_entity_id is None, but this is OK
        assert_eq!(r.object_entity_id, None);
        // Subject should still be resolved
        assert!(r.subject_entity_id.is_some());
    }

    #[tokio::test]
    async fn relationship_update_subject_unresolved_is_create_new() {
        let (_pool, _retriever, entity_repo) = setup().await;
        // No entities exist — subject cannot be resolved
        let obs = make_relationship_obs("新角色", "路人甲");
        let results = resolve(&[obs], &entity_repo, "book1").await.unwrap();
        let r = &results[0];
        assert_eq!(r.subject_entity_id, None);
        assert_eq!(r.resolution, Resolution::CreateNew);
    }

    #[tokio::test]
    async fn relationship_update_risk_level_is_high() {
        let (_pool, _retriever, entity_repo) = setup().await;
        let obs = make_relationship_obs("张三", "李四");
        let results = resolve(&[obs], &entity_repo, "book1").await.unwrap();
        assert_eq!(results[0].risk_level, RiskLevel::High);
    }

    #[tokio::test]
    async fn identity_reveal_best_effort_resolves_both_sides() {
        let (_pool, _retriever, entity_repo) = setup().await;
        let revealed = entity_repo
            .create_entity("book1", "character", "黑衣人", "黑衣人", None, 0.6, 1)
            .await
            .unwrap();
        let canonical = entity_repo
            .create_entity("book1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        let obs = Observation::IdentityReveal {
            revealed_mention: "黑衣人".to_string(),
            canonical_mention: "张三".to_string(),
            reveal_type: "disguise".to_string(),
            reason_hint: Some("摘下面具".to_string()),
            evidence_span_ids: vec!["s1".to_string()],
            confidence: 0.95,
        };
        let results = resolve(&[obs], &entity_repo, "book1").await.unwrap();
        let resolved = &results[0];

        assert_eq!(resolved.subject_entity_id, Some(revealed.id));
        assert_eq!(resolved.object_entity_id, Some(canonical.id));
        assert_eq!(resolved.risk_level, RiskLevel::High);
    }

    #[tokio::test]
    async fn not_same_identity_keeps_unresolved_side_in_ledger() {
        let (_pool, _retriever, entity_repo) = setup().await;
        let left = entity_repo
            .create_entity("book1", "character", "此张三", "此张三", None, 0.7, 1)
            .await
            .unwrap();

        let obs = Observation::NotSameIdentity {
            entity_a_mention: "此张三".to_string(),
            entity_b_mention: "彼张三".to_string(),
            reason_hint: Some("并非同一人".to_string()),
            evidence_span_ids: vec!["s1".to_string()],
            confidence: 0.88,
        };
        let results = resolve(&[obs], &entity_repo, "book1").await.unwrap();
        let resolved = &results[0];

        assert_eq!(resolved.subject_entity_id, Some(left.id));
        assert_eq!(resolved.object_entity_id, None);
        assert_eq!(resolved.risk_level, RiskLevel::High);
    }

    #[tokio::test]
    async fn knowledge_assertion_resolves_referenced_entities_best_effort() {
        let (_pool, _retriever, entity_repo) = setup().await;
        let realm = entity_repo
            .create_entity("book1", "realm", "金丹", "金丹", None, 0.8, 1)
            .await
            .unwrap();

        let obs = Observation::KnowledgeAssertion {
            category: "power_system".to_string(),
            topic: "修炼境界".to_string(),
            assertion_text: "修炼境界包括金丹。".to_string(),
            confidence: 0.9,
            importance_score: 0.8,
            evidence_span_ids: vec!["s1".to_string()],
            referenced_entity_mentions: vec![
                crate::service::v4::extractor::KnowledgeEntityMention {
                    mention: "金丹".to_string(),
                    entity_type_hint: Some("realm".to_string()),
                    role: "realm".to_string(),
                    confidence: 0.9,
                    resolved_entity_id: None,
                },
                crate::service::v4::extractor::KnowledgeEntityMention {
                    mention: "未出现概念".to_string(),
                    entity_type_hint: Some("concept".to_string()),
                    role: "related".to_string(),
                    confidence: 0.4,
                    resolved_entity_id: None,
                },
            ],
            status_hint: Some("fact".to_string()),
            reason_hint: None,
        };

        let results = resolve(&[obs], &entity_repo, "book1").await.unwrap();
        let resolved = &results[0];
        match &resolved.observation {
            Observation::KnowledgeAssertion {
                referenced_entity_mentions,
                ..
            } => {
                assert_eq!(
                    referenced_entity_mentions[0].resolved_entity_id.as_deref(),
                    Some(realm.id.as_str())
                );
                assert_eq!(referenced_entity_mentions[1].resolved_entity_id, None);
            }
            _ => panic!("expected knowledge assertion"),
        }
        assert_eq!(resolved.risk_level, RiskLevel::High);
        assert_eq!(resolved.resolution, Resolution::Uncertain);
    }
}
