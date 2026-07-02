use crate::service::v4::extractor::{Observation, RiskLevel};
use crate::service::v4::resolver::{Resolution, ResolvedObservation};
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
/// - MinorEvent → claim with status='proposed', but reducer won't consume
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
                let status = match obs.risk_level {
                    RiskLevel::High => {
                        // High risk: quarantine or mark uncertain
                        if is_critical_high_risk(&obs.observation) {
                            "quarantined"
                        } else {
                            "uncertain"
                        }
                    }
                    RiskLevel::Low | RiskLevel::Medium => "proposed",
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
        | Observation::MinorEvent { confidence, .. } => *confidence,
        Observation::Summary { .. } => 0.0,
    }
}

/// Get object mention if applicable.
fn get_object_mention(obs: &Observation) -> Option<String> {
    match obs {
        Observation::Alias { alias, .. } => Some(alias.clone()),
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
            short_summary,
            aliases,
            ..
        } => {
            let json = serde_json::json!({
                "short_summary": short_summary,
                "aliases": aliases,
            });
            (Some(short_summary.clone()), Some(json.to_string()))
        }
        _ => (None, None),
    }
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
}
