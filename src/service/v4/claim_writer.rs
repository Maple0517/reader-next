use crate::service::v4::extractor::{Observation, RiskLevel};
use crate::service::v4::resolver::ResolvedObservation;
use crate::storage::db::v4::chapter_repo::ChapterRepo;
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};

/// Result of writing claims for a segment.
#[derive(Debug, Default)]
pub struct ClaimWriteResult {
    /// Claims created (status = proposed for low/medium, quarantined/uncertain for high)
    pub claims_created: Vec<ClaimRecord>,
    /// Chapter summaries written (from Summary observations)
    pub summaries_written: Vec<String>,
    /// Skipped observations (Summary type writes to chapter_summaries, not claims)
    pub skipped_count: usize,
}

/// Write claims from resolved observations.
///
/// Rules:
/// - Summary → write chapter_summaries, no claim
/// - MinorEvent → claim with status='proposed' (ledger-only, intentionally not reduced;
///   does not mean pending canonical work; does not block processing_progress)
/// - High risk → claim with status='quarantined' or 'uncertain', not entering reducer
/// - Low/Medium risk → claim with status='proposed', enters reducer
pub async fn write_claims(
    resolved: &[ResolvedObservation],
    book_id: &str,
    chapter_index: i64,
    ai_run_id: &str,
    claim_repo: &ClaimRepo,
    chapter_repo: &ChapterRepo,
) -> anyhow::Result<ClaimWriteResult> {
    let mut result = ClaimWriteResult::default();

    for obs in resolved {
        match &obs.observation {
            // Summary: write to chapter_summaries, skip claim creation
            Observation::Summary {
                summary,
                key_points,
                ..
            } => {
                let key_points_json = serde_json::to_string(key_points).ok();
                chapter_repo
                    .upsert_chapter_summary(
                        book_id,
                        chapter_index,
                        summary,
                        key_points_json.as_deref(),
                    )
                    .await?;
                result.summaries_written.push(summary.clone());
                result.skipped_count += 1;
            }

            // All other types: create claims
            _ => {
                let claim_type = get_claim_type(&obs.observation);
                let subject_mention = obs.observation.subject_mention().map(|s| s.to_string());
                let object_mention = get_object_mention(&obs.observation);
                let predicate = build_predicate(&obs.observation);
                let (value_text, value_json) = get_value_fields(&obs.observation);
                let primary_source_span_id = obs
                    .observation
                    .evidence_span_ids()
                    .first()
                    .cloned()
                    .unwrap_or_default();

                // Determine status based on risk level and claim type
                let status = match &obs.observation {
                    // RelationshipUpdate always enters Relationship Judge (status="proposed"),
                    // never quarantined or uncertain — it's a Phase 2 special high-risk path.
                    Observation::RelationshipUpdate { .. } => "proposed",
                    // KnowledgeAssertion always enters Knowledge Revision Judge; risk is
                    // observability/priority only and must not bypass the Phase 4 path.
                    Observation::KnowledgeAssertion { .. } => "proposed",
                    // Location observations always enter the Phase 5 place/map path.
                    // They remain proposed until the place reducer transaction accepts them.
                    Observation::LocationIntroduction { .. } | Observation::LocationEdge { .. } => {
                        "proposed"
                    }
                    _ if is_identity_observation(&obs.observation) => "proposed",
                    _ => match obs.risk_level {
                        RiskLevel::High => {
                            // High risk: quarantine or mark uncertain
                            if is_critical_high_risk(&obs.observation) {
                                "quarantined"
                            } else {
                                "uncertain"
                            }
                        }
                        RiskLevel::Low | RiskLevel::Medium => "proposed",
                    },
                };

                let claim = claim_repo
                    .create_claim(
                        book_id,
                        chapter_index,
                        &claim_type,
                        subject_mention.as_deref(),
                        object_mention.as_deref(),
                        obs.subject_entity_id.as_deref(),
                        obs.object_entity_id.as_deref(),
                        &predicate,
                        value_text.as_deref(),
                        value_json.as_deref(),
                        &primary_source_span_id,
                        ai_run_id,
                        get_confidence(&obs.observation),
                        obs.risk_level.as_str(),
                    )
                    .await?;

                // Update status if not 'proposed' (create_claim defaults to 'proposed')
                if status != "proposed" {
                    claim_repo.update_claim_status(&claim.id, status).await?;
                }

                // Add claim_source_spans for all evidence spans
                for span_id in obs.observation.evidence_span_ids() {
                    let role = if span_id == &primary_source_span_id {
                        "primary"
                    } else {
                        "supporting"
                    };
                    claim_repo
                        .add_claim_source_span(&claim.id, span_id, role)
                        .await?;
                }

                // Update claim record with actual status for return value
                let mut claim = claim;
                claim.status = status.to_string();
                result.claims_created.push(claim);
            }
        }
    }

    Ok(result)
}

/// Get claim type string from observation.
fn get_claim_type(obs: &Observation) -> String {
    match obs {
        Observation::EntityIntroduction { .. } => "entity_introduction".to_string(),
        Observation::Alias { .. } => "alias".to_string(),
        Observation::PropertyUpdate { .. } => "property_update".to_string(),
        Observation::RelationshipUpdate { .. } => "relationship_update".to_string(),
        Observation::KnowledgeAssertion { .. } => "knowledge_assertion".to_string(),
        Observation::LocationIntroduction { .. } => "location_introduction".to_string(),
        Observation::LocationEdge { .. } => "location_edge".to_string(),
        Observation::IdentityReveal { .. } => "identity_reveal".to_string(),
        Observation::EntityMergeCandidate { .. } => "entity_merge_candidate".to_string(),
        Observation::EntitySplitCandidate { .. } => "entity_split_candidate".to_string(),
        Observation::NotSameIdentity { .. } => "not_same_identity".to_string(),
        Observation::MinorEvent { .. } => "minor_event".to_string(),
        Observation::Summary { .. } => unreachable!("Summary should not create claims"),
    }
}

/// Get confidence from observation.
fn get_confidence(obs: &Observation) -> f64 {
    match obs {
        Observation::EntityIntroduction { confidence, .. }
        | Observation::Alias { confidence, .. }
        | Observation::PropertyUpdate { confidence, .. }
        | Observation::RelationshipUpdate { confidence, .. }
        | Observation::KnowledgeAssertion { confidence, .. }
        | Observation::LocationIntroduction { confidence, .. }
        | Observation::LocationEdge { confidence, .. }
        | Observation::IdentityReveal { confidence, .. }
        | Observation::EntityMergeCandidate { confidence, .. }
        | Observation::EntitySplitCandidate { confidence, .. }
        | Observation::NotSameIdentity { confidence, .. }
        | Observation::MinorEvent { confidence, .. } => *confidence,
        Observation::Summary { .. } => 0.0,
    }
}

/// Get object mention if applicable.
fn get_object_mention(obs: &Observation) -> Option<String> {
    match obs {
        Observation::Alias { alias, .. } => Some(alias.clone()),
        Observation::RelationshipUpdate { object_mention, .. } => Some(object_mention.clone()),
        Observation::IdentityReveal {
            canonical_mention, ..
        } => Some(canonical_mention.clone()),
        Observation::LocationEdge {
            to_place_mention, ..
        } => Some(to_place_mention.clone()),
        Observation::EntityMergeCandidate {
            entity_b_mention, ..
        }
        | Observation::EntitySplitCandidate {
            entity_b_mention, ..
        }
        | Observation::NotSameIdentity {
            entity_b_mention, ..
        } => Some(entity_b_mention.clone()),
        _ => None,
    }
}

/// Build predicate string from observation.
fn build_predicate(obs: &Observation) -> String {
    match obs {
        Observation::EntityIntroduction {
            entity_type,
            short_summary,
            ..
        } => {
            format!("is a {} — {}", entity_type, short_summary)
        }
        Observation::Alias {
            alias_type, alias, ..
        } => {
            format!("also known as {} ({})", alias, alias_type)
        }
        Observation::PropertyUpdate {
            dimension_key,
            value_text,
            ..
        } => {
            let value = value_text.as_deref().unwrap_or("unknown");
            format!("{} = {}", dimension_key, value)
        }
        Observation::RelationshipUpdate {
            subject_mention,
            object_mention,
            relation_label,
            ..
        } => {
            format!(
                "{} and {} — {}",
                subject_mention, object_mention, relation_label
            )
        }
        Observation::KnowledgeAssertion {
            category,
            topic,
            assertion_text,
            ..
        } => format!("{}:{} — {}", category, topic, assertion_text),
        Observation::LocationIntroduction {
            place_mention,
            place_type,
            parent_place_mention,
            ..
        } => {
            if let Some(parent) = parent_place_mention {
                format!("place {} ({}) under {}", place_mention, place_type, parent)
            } else {
                format!("place {} ({})", place_mention, place_type)
            }
        }
        Observation::LocationEdge {
            from_place_mention,
            to_place_mention,
            edge_type,
            ..
        } => {
            format!("{} {} {}", from_place_mention, edge_type, to_place_mention)
        }
        Observation::IdentityReveal {
            revealed_mention,
            canonical_mention,
            reveal_type,
            ..
        } => {
            format!(
                "{} is revealed as {} ({})",
                revealed_mention, canonical_mention, reveal_type
            )
        }
        Observation::EntityMergeCandidate {
            entity_a_mention,
            entity_b_mention,
            ..
        } => format!(
            "{} may be the same identity as {}",
            entity_a_mention, entity_b_mention
        ),
        Observation::EntitySplitCandidate {
            entity_a_mention,
            entity_b_mention,
            ..
        } => format!(
            "{} may have been merged with {}",
            entity_a_mention, entity_b_mention
        ),
        Observation::NotSameIdentity {
            entity_a_mention,
            entity_b_mention,
            ..
        } => format!(
            "{} is not the same identity as {}",
            entity_a_mention, entity_b_mention
        ),
        Observation::MinorEvent { description, .. } => description.clone(),
        Observation::Summary { .. } => unreachable!(),
    }
}

/// Get value_text and value_json from observation.
fn get_value_fields(obs: &Observation) -> (Option<String>, Option<String>) {
    match obs {
        Observation::PropertyUpdate {
            value_text,
            value_json,
            ..
        } => {
            let vtext = value_text.clone();
            let vjson = value_json
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default());
            (vtext, vjson)
        }
        Observation::EntityIntroduction {
            entity_type,
            short_summary,
            aliases,
            ..
        } => {
            let json = serde_json::json!({
                "entity_type": entity_type,
                "short_summary": short_summary,
                "aliases": aliases,
            });
            (Some(short_summary.clone()), Some(json.to_string()))
        }
        Observation::RelationshipUpdate {
            relation_hint,
            relation_group,
            relation_label,
            directionality,
            importance_hint,
            is_long_term_or_significant_hint,
            ..
        } => {
            let json = serde_json::json!({
                "relation_hint": relation_hint,
                "relation_group": relation_group,
                "relation_label": relation_label,
                "directionality": directionality,
                "importance_hint": importance_hint,
                "is_long_term_or_significant_hint": is_long_term_or_significant_hint,
            });
            (None, Some(json.to_string()))
        }
        Observation::KnowledgeAssertion {
            category,
            topic,
            assertion_text,
            importance_score,
            referenced_entity_mentions,
            status_hint,
            reason_hint,
            ..
        } => {
            let json = serde_json::json!({
                "category": category,
                "raw_topic": topic,
                "topic_display": topic,
                "assertion_text": assertion_text,
                "importance_score": importance_score,
                "referenced_entity_mentions": referenced_entity_mentions,
                "status_hint": status_hint,
                "reason_hint": reason_hint,
            });
            (Some(assertion_text.clone()), Some(json.to_string()))
        }
        Observation::LocationIntroduction {
            place_type,
            parent_place_mention,
            aliases,
            description,
            importance_score,
            map_visible_hint,
            ..
        } => {
            let json = serde_json::json!({
                "place_type": place_type,
                "parent_place_mention": parent_place_mention,
                "aliases": aliases,
                "description": description,
                "importance_score": importance_score,
                "map_visible_hint": map_visible_hint,
            });
            (description.clone(), Some(json.to_string()))
        }
        Observation::LocationEdge {
            edge_type,
            direction_hint,
            distance_hint,
            is_topological_hint,
            ..
        } => {
            let json = serde_json::json!({
                "edge_type": edge_type,
                "direction_hint": direction_hint,
                "distance_hint": distance_hint,
                "is_topological_hint": is_topological_hint,
            });
            (None, Some(json.to_string()))
        }
        Observation::IdentityReveal {
            reveal_type,
            reason_hint,
            ..
        } => {
            let json = build_identity_payload(
                "same_identity",
                reason_hint.as_deref(),
                Some(("reveal_type", reveal_type.as_str())),
            );
            (reason_hint.clone(), Some(json.to_string()))
        }
        Observation::EntityMergeCandidate { reason_hint, .. } => {
            let json = build_identity_payload(
                "same_identity",
                reason_hint.as_deref(),
                Some(("candidate_type", "entity_merge_candidate")),
            );
            (reason_hint.clone(), Some(json.to_string()))
        }
        Observation::EntitySplitCandidate { reason_hint, .. } => {
            let json = build_identity_payload(
                "mistaken_identity",
                reason_hint.as_deref(),
                Some(("candidate_type", "entity_split_candidate")),
            );
            (reason_hint.clone(), Some(json.to_string()))
        }
        Observation::NotSameIdentity { reason_hint, .. } => {
            let json = build_identity_payload(
                "not_same_identity",
                reason_hint.as_deref(),
                Some(("candidate_type", "not_same_identity")),
            );
            (reason_hint.clone(), Some(json.to_string()))
        }
        _ => (None, None),
    }
}

fn is_identity_observation(obs: &Observation) -> bool {
    matches!(
        obs,
        Observation::IdentityReveal { .. }
            | Observation::EntityMergeCandidate { .. }
            | Observation::EntitySplitCandidate { .. }
            | Observation::NotSameIdentity { .. }
    )
}

fn build_identity_payload(
    identity_kind: &str,
    reason_hint: Option<&str>,
    variant_field: Option<(&str, &str)>,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "reason_hint": reason_hint,
        "identity_kind": identity_kind,
        "judge_decision": serde_json::Value::Null,
        "judge_confidence": serde_json::Value::Null,
        "survivor_hint": serde_json::Value::Null,
    });
    if let Some((key, value)) = variant_field {
        payload[key] = serde_json::Value::String(value.to_string());
    }
    payload
}

/// Check if an observation is critical high risk (should be quarantined vs uncertain).
fn is_critical_high_risk(obs: &Observation) -> bool {
    match obs {
        Observation::PropertyUpdate {
            dimension_key,
            value_text,
            ..
        } => {
            let value = value_text.as_deref().unwrap_or("").to_lowercase();
            // Death, resurrection, identity reveal → quarantined
            (dimension_key == "life_status"
                && (value.contains("死") || value.contains("亡") || value.contains("复活")))
                || (dimension_key == "identity" && value.contains("真实身份"))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::resolver::Resolution;
    use crate::storage::db;
    use sqlx::SqlitePool;

    async fn setup() -> (SqlitePool, ClaimRepo, ChapterRepo) {
        let dir =
            std::env::temp_dir().join(format!("reader-v4-claimwriter-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();

        // Insert chapter for FK
        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 1, 'text', 'hash', datetime('now'))")
            .bind(&chapter_id).execute(&pool).await.unwrap();

        // Insert source span for FK
        let span_id = uuid::Uuid::new_v4().to_string();
        let seg_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'hash', 0, datetime('now'))")
            .bind(&seg_id).bind(&chapter_id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 0, 0, 10, 'test text', datetime('now'))")
            .bind(&span_id).bind(&chapter_id).bind(&seg_id).execute(&pool).await.unwrap();

        // Insert ai_run for FK
        let run_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, 'b1', ?, 'extract', 'gpt-4', 'v1', 1, 'hash', 'running', datetime('now'))")
            .bind(&run_id).bind(&chapter_id).execute(&pool).await.unwrap();

        let claim_repo = ClaimRepo::new(pool.clone());
        let chapter_repo = ChapterRepo::new(pool.clone());
        (pool, claim_repo, chapter_repo)
    }

    #[tokio::test]
    async fn summary_writes_to_chapter_summaries() {
        let (_pool, claim_repo, chapter_repo) = setup().await;

        let resolved = vec![ResolvedObservation {
            observation: Observation::Summary {
                summary: "张三突破到筑基期".to_string(),
                key_points: vec!["突破".to_string(), "筑基".to_string()],
                has_important_changes: true,
            },
            subject_entity_id: None,
            object_entity_id: None,
            resolved_dimension_key: None,
            risk_level: RiskLevel::Low,
            resolution: Resolution::Uncertain,
        }];

        let result = write_claims(&resolved, "b1", 1, "run-1", &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 0);
        assert_eq!(result.summaries_written.len(), 1);
        assert_eq!(result.summaries_written[0], "张三突破到筑基期");
        assert_eq!(result.skipped_count, 1);

        // Verify in DB
        let summary = chapter_repo.get_chapter_summary("b1", 1).await.unwrap();
        assert!(summary.is_some());
        assert_eq!(summary.unwrap().summary, "张三突破到筑基期");
    }

    #[tokio::test]
    async fn low_risk_creates_proposed_claim() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        // Get the span_id and run_id from setup
        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::PropertyUpdate {
                subject_mention: "张三".to_string(),
                dimension_key: "realm".to_string(),
                value_text: Some("筑基期".to_string()),
                value_json: None,
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.9,
            },
            subject_entity_id: Some("entity-1".to_string()),
            object_entity_id: None,
            resolved_dimension_key: Some("realm".to_string()),
            risk_level: RiskLevel::Low,
            resolution: Resolution::MatchExisting {
                entity_id: "entity-1".to_string(),
            },
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        assert_eq!(result.claims_created[0].status, "proposed");
        assert_eq!(result.claims_created[0].claim_type, "property_update");
        assert_eq!(
            result.claims_created[0].subject_entity_id.as_deref(),
            Some("entity-1")
        );
    }

    #[tokio::test]
    async fn location_introduction_creates_proposed_claim() {
        let (pool, claim_repo, chapter_repo) = setup().await;
        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::LocationIntroduction {
                place_mention: "青云城".to_string(),
                place_type: "city".to_string(),
                parent_place_mention: Some("东域".to_string()),
                aliases: vec!["青云古城".to_string()],
                description: Some("东域重城".to_string()),
                importance_score: 0.8,
                map_visible_hint: Some(true),
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.9,
            },
            subject_entity_id: None,
            object_entity_id: None,
            resolved_dimension_key: None,
            risk_level: RiskLevel::Low,
            resolution: Resolution::CreateNew,
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        let claim = &result.claims_created[0];
        assert_eq!(claim.status, "proposed");
        assert_eq!(claim.claim_type, "location_introduction");
        assert_eq!(claim.subject_mention.as_deref(), Some("青云城"));
        let value: serde_json::Value =
            serde_json::from_str(claim.value_json.as_deref().expect("value_json")).unwrap();
        assert_eq!(value["place_type"], "city");
        assert_eq!(value["parent_place_mention"], "东域");
        assert_eq!(value["aliases"][0], "青云古城");
        assert_eq!(value["map_visible_hint"], true);
    }

    #[tokio::test]
    async fn location_edge_creates_proposed_claim() {
        let (pool, claim_repo, chapter_repo) = setup().await;
        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::LocationEdge {
                from_place_mention: "黑风谷".to_string(),
                to_place_mention: "青云城".to_string(),
                edge_type: "north_of".to_string(),
                direction_hint: Some("north".to_string()),
                distance_hint: Some("百里".to_string()),
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.86,
                is_topological_hint: true,
            },
            subject_entity_id: None,
            object_entity_id: None,
            resolved_dimension_key: None,
            risk_level: RiskLevel::High,
            resolution: Resolution::Uncertain,
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        let claim = &result.claims_created[0];
        assert_eq!(claim.status, "proposed");
        assert_eq!(claim.claim_type, "location_edge");
        assert_eq!(claim.subject_mention.as_deref(), Some("黑风谷"));
        assert_eq!(claim.object_mention.as_deref(), Some("青云城"));
        let value: serde_json::Value =
            serde_json::from_str(claim.value_json.as_deref().expect("value_json")).unwrap();
        assert_eq!(value["edge_type"], "north_of");
        assert_eq!(value["direction_hint"], "north");
        assert_eq!(value["distance_hint"], "百里");
        assert_eq!(value["is_topological_hint"], true);
    }

    #[tokio::test]
    async fn location_claims_preserve_source_spans() {
        let (pool, claim_repo, chapter_repo) = setup().await;
        let first_span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let second_span_id = uuid::Uuid::new_v4().to_string();
        let chapter_id: (String,) =
            sqlx::query_as("SELECT id FROM chapters WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let segment_id: (String,) =
            sqlx::query_as("SELECT id FROM chapter_segments WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 1, 11, 20, 'more text', datetime('now'))")
            .bind(&second_span_id)
            .bind(&chapter_id.0)
            .bind(&segment_id.0)
            .execute(&pool)
            .await
            .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::LocationEdge {
                from_place_mention: "青云城".to_string(),
                to_place_mention: "黑风谷".to_string(),
                edge_type: "route_to".to_string(),
                direction_hint: None,
                distance_hint: None,
                evidence_span_ids: vec![first_span_id.0.clone(), second_span_id.clone()],
                confidence: 0.8,
                is_topological_hint: true,
            },
            subject_entity_id: None,
            object_entity_id: None,
            resolved_dimension_key: None,
            risk_level: RiskLevel::High,
            resolution: Resolution::Uncertain,
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();
        let claim_id = &result.claims_created[0].id;
        let links: Vec<(String, String)> = sqlx::query_as(
            "SELECT source_span_id, role FROM claim_source_spans WHERE claim_id = ? ORDER BY role DESC, source_span_id ASC",
        )
        .bind(claim_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(links.len(), 2);
        assert!(links.contains(&(first_span_id.0, "primary".to_string())));
        assert!(links.contains(&(second_span_id, "supporting".to_string())));
    }

    #[tokio::test]
    async fn high_risk_death_quarantined() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::PropertyUpdate {
                subject_mention: "张三".to_string(),
                dimension_key: "life_status".to_string(),
                value_text: Some("死亡".to_string()),
                value_json: None,
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.95,
            },
            subject_entity_id: Some("entity-1".to_string()),
            object_entity_id: None,
            resolved_dimension_key: Some("life_status".to_string()),
            risk_level: RiskLevel::High,
            resolution: Resolution::MatchExisting {
                entity_id: "entity-1".to_string(),
            },
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        assert_eq!(result.claims_created[0].status, "quarantined");
    }

    #[tokio::test]
    async fn high_risk_non_critical_uncertain() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // High risk but not critical (e.g., affiliation change)
        let resolved = vec![ResolvedObservation {
            observation: Observation::PropertyUpdate {
                subject_mention: "张三".to_string(),
                dimension_key: "affiliation".to_string(),
                value_text: Some("魔道".to_string()),
                value_json: None,
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.7,
            },
            subject_entity_id: Some("entity-1".to_string()),
            object_entity_id: None,
            resolved_dimension_key: Some("affiliation".to_string()),
            risk_level: RiskLevel::High,
            resolution: Resolution::MatchExisting {
                entity_id: "entity-1".to_string(),
            },
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        assert_eq!(result.claims_created[0].status, "uncertain");
    }

    #[tokio::test]
    async fn minor_event_creates_proposed_claim() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::MinorEvent {
                description: "张三和李四切磋".to_string(),
                involved_mentions: vec!["张三".to_string(), "李四".to_string()],
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.8,
            },
            subject_entity_id: None,
            object_entity_id: None,
            resolved_dimension_key: None,
            risk_level: RiskLevel::Low,
            resolution: Resolution::Uncertain,
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        assert_eq!(result.claims_created[0].status, "proposed");
        assert_eq!(result.claims_created[0].claim_type, "minor_event");
    }

    #[tokio::test]
    async fn entity_introduction_creates_claim_with_value() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::EntityIntroduction {
                subject_mention: "一个年轻人".to_string(),
                entity_type: "character".to_string(),
                aliases: vec!["小张".to_string()],
                short_summary: "新出场的角色".to_string(),
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.85,
            },
            subject_entity_id: None,
            object_entity_id: None,
            resolved_dimension_key: None,
            risk_level: RiskLevel::Low,
            resolution: Resolution::CreateNew,
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        assert_eq!(result.claims_created[0].status, "proposed");
        assert_eq!(result.claims_created[0].claim_type, "entity_introduction");
        assert!(result.claims_created[0].value_text.is_some());
    }

    #[tokio::test]
    async fn claim_source_spans_association_with_roles() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        // Get existing span and run
        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Insert a second source span for the same segment
        let chapter_id: (String,) =
            sqlx::query_as("SELECT id FROM chapters WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let seg_id: (String,) =
            sqlx::query_as("SELECT id FROM chapter_segments WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let span_id_2 = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 1, 10, 20, 'more text', datetime('now'))")
            .bind(&span_id_2).bind(&chapter_id.0).bind(&seg_id.0).execute(&pool).await.unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::PropertyUpdate {
                subject_mention: "李四".to_string(),
                dimension_key: "realm".to_string(),
                value_text: Some("金丹期".to_string()),
                value_json: None,
                evidence_span_ids: vec![span_id.0.clone(), span_id_2.clone()],
                confidence: 0.85,
            },
            subject_entity_id: Some("entity-2".to_string()),
            object_entity_id: None,
            resolved_dimension_key: Some("realm".to_string()),
            risk_level: RiskLevel::Low,
            resolution: Resolution::MatchExisting {
                entity_id: "entity-2".to_string(),
            },
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        let claim = &result.claims_created[0];

        // Verify primary_source_span_id is the first evidence span
        assert_eq!(claim.primary_source_span_id, span_id.0);

        // Verify claim_source_spans entries in DB
        let spans = claim_repo.list_claim_spans(&claim.id).await.unwrap();
        assert_eq!(spans.len(), 2);
        let span_ids: Vec<&str> = spans.iter().map(|s| s.id.as_str()).collect();
        assert!(span_ids.contains(&span_id.0.as_str()));
        assert!(span_ids.contains(&span_id_2.as_str()));
    }

    #[tokio::test]
    async fn relationship_update_has_correct_object_mention() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::RelationshipUpdate {
                subject_mention: "张三".to_string(),
                object_mention: "李四".to_string(),
                relation_hint: "师徒".to_string(),
                relation_group: "mentorship".to_string(),
                relation_label: "师父".to_string(),
                directionality: "directed".to_string(),
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.9,
                importance_hint: 0.8,
                is_long_term_or_significant_hint: true,
            },
            subject_entity_id: Some("entity-1".to_string()),
            object_entity_id: Some("entity-2".to_string()),
            resolved_dimension_key: None,
            risk_level: RiskLevel::High,
            resolution: Resolution::MatchExisting {
                entity_id: "entity-1".to_string(),
            },
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        assert_eq!(
            result.claims_created[0].object_mention.as_deref(),
            Some("李四")
        );
    }

    #[tokio::test]
    async fn relationship_update_has_correct_value_json() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::RelationshipUpdate {
                subject_mention: "张三".to_string(),
                object_mention: "李四".to_string(),
                relation_hint: "宿敌".to_string(),
                relation_group: "rivalry".to_string(),
                relation_label: "对手".to_string(),
                directionality: "undirected".to_string(),
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.85,
                importance_hint: 0.7,
                is_long_term_or_significant_hint: true,
            },
            subject_entity_id: Some("entity-1".to_string()),
            object_entity_id: Some("entity-2".to_string()),
            resolved_dimension_key: None,
            risk_level: RiskLevel::High,
            resolution: Resolution::MatchExisting {
                entity_id: "entity-1".to_string(),
            },
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        let vj = result.claims_created[0]
            .value_json
            .as_ref()
            .expect("value_json should be present");
        let parsed: serde_json::Value = serde_json::from_str(vj).unwrap();
        assert_eq!(parsed["relation_hint"], "宿敌");
        assert_eq!(parsed["relation_group"], "rivalry");
        assert_eq!(parsed["relation_label"], "对手");
        assert_eq!(parsed["directionality"], "undirected");
        assert_eq!(parsed["importance_hint"], 0.7);
        assert_eq!(parsed["is_long_term_or_significant_hint"], true);
    }

    #[tokio::test]
    async fn relationship_update_always_status_proposed() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // RelationshipUpdate with High risk — must be "proposed", not "uncertain" or "quarantined"
        let resolved = vec![ResolvedObservation {
            observation: Observation::RelationshipUpdate {
                subject_mention: "张三".to_string(),
                object_mention: "李四".to_string(),
                relation_hint: "敌对".to_string(),
                relation_group: "hostility".to_string(),
                relation_label: "宿敌".to_string(),
                directionality: "undirected".to_string(),
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.95,
                importance_hint: 0.9,
                is_long_term_or_significant_hint: true,
            },
            subject_entity_id: Some("entity-1".to_string()),
            object_entity_id: Some("entity-2".to_string()),
            resolved_dimension_key: None,
            risk_level: RiskLevel::High,
            resolution: Resolution::MatchExisting {
                entity_id: "entity-1".to_string(),
            },
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        assert_eq!(result.claims_created[0].status, "proposed");
        assert_eq!(result.claims_created[0].claim_type, "relationship_update");
    }

    #[tokio::test]
    async fn identity_reveal_is_proposed_and_preserves_all_evidence_spans() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        let span_id_1: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let chapter_id: (String,) =
            sqlx::query_as("SELECT id FROM chapters WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let seg_id: (String,) =
            sqlx::query_as("SELECT id FROM chapter_segments WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let span_id_2 = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 1, 10, 20, 'more text', datetime('now'))")
            .bind(&span_id_2).bind(&chapter_id.0).bind(&seg_id.0).execute(&pool).await.unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::IdentityReveal {
                revealed_mention: "黑衣人".to_string(),
                canonical_mention: "张三".to_string(),
                reveal_type: "disguise".to_string(),
                reason_hint: Some("摘下面具".to_string()),
                evidence_span_ids: vec![span_id_1.0.clone(), span_id_2.clone()],
                confidence: 0.95,
            },
            subject_entity_id: Some("entity-shadow".to_string()),
            object_entity_id: Some("entity-zhangsan".to_string()),
            resolved_dimension_key: None,
            risk_level: RiskLevel::High,
            resolution: Resolution::Uncertain,
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        let claim = &result.claims_created[0];
        assert_eq!(claim.claim_type, "identity_reveal");
        assert_eq!(claim.status, "proposed");
        assert_eq!(claim.primary_source_span_id, span_id_1.0);
        let spans = claim_repo.list_claim_spans(&claim.id).await.unwrap();
        assert_eq!(spans.len(), 2);

        let payload: serde_json::Value =
            serde_json::from_str(claim.value_json.as_deref().expect("value_json")).unwrap();
        assert_eq!(payload["identity_kind"], "same_identity");
        assert_eq!(payload["reveal_type"], "disguise");
        assert_eq!(payload["reason_hint"], "摘下面具");
    }

    #[tokio::test]
    async fn not_same_identity_maps_to_proposed_claim_type() {
        let (pool, claim_repo, chapter_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::NotSameIdentity {
                entity_a_mention: "此张三".to_string(),
                entity_b_mention: "彼张三".to_string(),
                reason_hint: Some("并非同一人".to_string()),
                evidence_span_ids: vec![span_id.0.clone()],
                confidence: 0.9,
            },
            subject_entity_id: Some("entity-a".to_string()),
            object_entity_id: Some("entity-b".to_string()),
            resolved_dimension_key: None,
            risk_level: RiskLevel::High,
            resolution: Resolution::Uncertain,
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        let claim = &result.claims_created[0];
        assert_eq!(claim.claim_type, "not_same_identity");
        assert_eq!(claim.status, "proposed");
        assert_eq!(claim.subject_entity_id.as_deref(), Some("entity-a"));
        assert_eq!(claim.object_entity_id.as_deref(), Some("entity-b"));
    }

    #[tokio::test]
    async fn knowledge_assertion_creates_proposed_claim_with_complete_value_json() {
        let (pool, claim_repo, chapter_repo) = setup().await;
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let resolved = vec![ResolvedObservation {
            observation: Observation::KnowledgeAssertion {
                category: "power_system".to_string(),
                topic: "修炼境界".to_string(),
                assertion_text: "修炼境界分为炼气、筑基、金丹。".to_string(),
                confidence: 0.9,
                importance_score: 0.8,
                evidence_span_ids: vec![span_id.0.clone()],
                referenced_entity_mentions: vec![
                    crate::service::v4::extractor::KnowledgeEntityMention {
                        mention: "金丹".to_string(),
                        entity_type_hint: Some("realm".to_string()),
                        role: "realm".to_string(),
                        confidence: 0.9,
                        resolved_entity_id: None,
                    },
                    crate::service::v4::extractor::KnowledgeEntityMention {
                        mention: "未解析实体".to_string(),
                        entity_type_hint: None,
                        role: "related".to_string(),
                        confidence: 0.4,
                        resolved_entity_id: None,
                    },
                ],
                status_hint: Some("fact".to_string()),
                reason_hint: Some("旁白说明境界体系".to_string()),
            },
            subject_entity_id: None,
            object_entity_id: None,
            resolved_dimension_key: None,
            risk_level: RiskLevel::High,
            resolution: Resolution::Uncertain,
        }];

        let result = write_claims(&resolved, "b1", 1, &run_id.0, &claim_repo, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.claims_created.len(), 1);
        let claim = &result.claims_created[0];
        assert_eq!(claim.claim_type, "knowledge_assertion");
        assert_eq!(claim.status, "proposed");
        assert_eq!(claim.risk_level, "high");
        assert_eq!(claim.primary_source_span_id, span_id.0);
        assert_eq!(
            claim.value_text.as_deref(),
            Some("修炼境界分为炼气、筑基、金丹。")
        );

        let value_json: serde_json::Value =
            serde_json::from_str(claim.value_json.as_deref().unwrap()).unwrap();
        assert_eq!(value_json["category"], "power_system");
        assert_eq!(value_json["raw_topic"], "修炼境界");
        assert_eq!(value_json["topic_display"], "修炼境界");
        assert_eq!(
            value_json["assertion_text"],
            "修炼境界分为炼气、筑基、金丹。"
        );
        assert_eq!(value_json["status_hint"], "fact");
        assert_eq!(value_json["importance_score"], 0.8);
        assert_eq!(
            value_json["referenced_entity_mentions"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        let links: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM claim_source_spans WHERE claim_id = ?")
                .bind(&claim.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(links.0, 1);
    }
}
