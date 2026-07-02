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
}
