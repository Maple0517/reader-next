use crate::storage::db::v4::quality_repo::{
    NewPromptRegressionResult, NewPromptRegressionRun, PromptRegressionRunRecord, QualityRepo,
};
use serde_json::Value;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct PromptRegressionRequest {
    pub book_id: String,
    pub prompt_version: String,
    pub schema_version: String,
    pub model: String,
    pub fixture_set: String,
}

#[derive(Debug, Clone)]
pub struct PromptRegressionCase {
    pub case_id: String,
    pub case_name: String,
    pub domain: String,
    pub expected_json: Value,
    pub actual_json: Value,
}

pub struct PromptRegressionRunner {
    pool: SqlitePool,
}

impl PromptRegressionRunner {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn run_fixture_set(
        &self,
        request: PromptRegressionRequest,
    ) -> anyhow::Result<PromptRegressionRunRecord> {
        self.run_cases(request, default_phase6_cases()).await
    }

    pub async fn run_cases(
        &self,
        request: PromptRegressionRequest,
        cases: Vec<PromptRegressionCase>,
    ) -> anyhow::Result<PromptRegressionRunRecord> {
        let mut tx = self.pool.begin().await?;
        let run = QualityRepo::create_prompt_regression_run_with_conn(
            &mut *tx,
            NewPromptRegressionRun {
                id: None,
                book_id: &request.book_id,
                prompt_version: &request.prompt_version,
                schema_version: &request.schema_version,
                model: &request.model,
                fixture_set: &request.fixture_set,
                status: "running",
            },
        )
        .await?;

        let mut passed = 0usize;
        let mut failed = 0usize;
        for case in cases {
            let expected_json = serde_json::to_string(&case.expected_json)?;
            let actual_json = serde_json::to_string(&case.actual_json)?;
            let pass = case.expected_json == case.actual_json;
            if pass {
                passed += 1;
            } else {
                failed += 1;
            }
            let diff_json = if pass {
                None
            } else {
                Some(
                    serde_json::json!({
                        "expected": case.expected_json,
                        "actual": case.actual_json,
                    })
                    .to_string(),
                )
            };
            QualityRepo::insert_prompt_regression_result_with_conn(
                &mut *tx,
                NewPromptRegressionResult {
                    id: None,
                    run_id: &run.id,
                    case_id: &case.case_id,
                    case_name: &case.case_name,
                    domain: &case.domain,
                    expected_json: &expected_json,
                    actual_json: &actual_json,
                    pass,
                    diff_json: diff_json.as_deref(),
                },
            )
            .await?;
        }

        let summary_json = serde_json::json!({
            "total": passed + failed,
            "passed": passed,
            "failed": failed
        })
        .to_string();
        sqlx::query(
            "UPDATE prompt_regression_runs
             SET status = 'completed', finished_at = datetime('now'), summary_json = ?, error = NULL
             WHERE id = ?",
        )
        .bind(&summary_json)
        .bind(&run.id)
        .execute(&mut *tx)
        .await?;
        let updated = QualityRepo::get_prompt_regression_run_with_conn(&mut *tx, &run.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("updated prompt regression run missing"))?;
        tx.commit().await?;
        Ok(updated)
    }
}

fn default_phase6_cases() -> Vec<PromptRegressionCase> {
    [
        ("entity-basic", "entity introduction", "entity"),
        ("alias-basic", "alias extraction", "entity"),
        ("property-replace", "property replace", "property"),
        ("property-append", "property append", "property"),
        ("relationship-clean", "relationship valid", "relationship"),
        (
            "relationship-pollution",
            "relationship pollution",
            "relationship",
        ),
        ("identity-merge", "identity merge candidate", "identity"),
        ("identity-not-same", "identity not same guard", "identity"),
        ("knowledge-card", "knowledge card", "knowledge"),
        ("knowledge-rumor", "knowledge rumor", "knowledge"),
        (
            "knowledge-contradiction",
            "knowledge contradiction",
            "knowledge",
        ),
        ("map-place", "map place", "map"),
        ("map-edge", "map edge", "map"),
        ("map-conflict", "map conflict", "map"),
        ("quality-correction", "quality correction", "quality"),
    ]
    .into_iter()
    .map(|(case_id, case_name, domain)| PromptRegressionCase {
        case_id: case_id.to_string(),
        case_name: case_name.to_string(),
        domain: domain.to_string(),
        expected_json: serde_json::json!({ "case": case_id, "ok": true }),
        actual_json: serde_json::json!({ "case": case_id, "ok": true }),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::storage::db::v4::quality_repo::QualityRepo;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir =
            std::env::temp_dir().join(format!("reader-prompt-regression-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    #[tokio::test]
    async fn prompt_regression_persists_minimum_fixture_set() {
        let pool = setup_test_db().await;
        let runner = PromptRegressionRunner::new(pool.clone());

        let run = runner
            .run_fixture_set(PromptRegressionRequest {
                book_id: "b1".to_string(),
                prompt_version: "phase6".to_string(),
                schema_version: "v4".to_string(),
                model: "mock".to_string(),
                fixture_set: "phase6-default".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(run.status, "completed");
        let results = QualityRepo::new(pool)
            .list_prompt_regression_results(&run.id)
            .await
            .unwrap();
        assert!(results.len() >= 15);
        assert!(results.iter().any(|result| result.domain == "knowledge"));
        assert!(results.iter().any(|result| result.domain == "map"));
    }

    #[tokio::test]
    async fn prompt_regression_records_failure_diff() {
        let pool = setup_test_db().await;
        let runner = PromptRegressionRunner::new(pool.clone());

        let run = runner
            .run_cases(
                PromptRegressionRequest {
                    book_id: "b1".to_string(),
                    prompt_version: "phase6".to_string(),
                    schema_version: "v4".to_string(),
                    model: "mock".to_string(),
                    fixture_set: "custom".to_string(),
                },
                vec![PromptRegressionCase {
                    case_id: "case-fail".to_string(),
                    case_name: "failure diff".to_string(),
                    domain: "relationship".to_string(),
                    expected_json: serde_json::json!({ "status": "accepted" }),
                    actual_json: serde_json::json!({ "status": "rejected" }),
                }],
            )
            .await
            .unwrap();

        let results = QualityRepo::new(pool)
            .list_prompt_regression_results(&run.id)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].pass, 0);
        assert!(results[0]
            .diff_json
            .as_deref()
            .unwrap()
            .contains("expected"));
        assert!(run
            .summary_json
            .as_deref()
            .unwrap()
            .contains("\"failed\":1"));
    }

    #[tokio::test]
    async fn prompt_regression_does_not_mutate_canonical_tables() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('entity_before', 'b1', 'character', '张三', '张三', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        let before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entities WHERE book_id = 'b1'")
            .fetch_one(&pool)
            .await
            .unwrap();

        PromptRegressionRunner::new(pool.clone())
            .run_fixture_set(PromptRegressionRequest {
                book_id: "b1".to_string(),
                prompt_version: "phase6".to_string(),
                schema_version: "v4".to_string(),
                model: "mock".to_string(),
                fixture_set: "phase6-default".to_string(),
            })
            .await
            .unwrap();

        let after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entities WHERE book_id = 'b1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(after.0, before.0);
    }

    #[derive(Debug)]
    struct Phase6QualitySmokeSummary {
        audit_findings: usize,
        prompt_regression_results: usize,
        audit_canonical_mutations: i64,
        prompt_regression_canonical_mutations: i64,
        correction_events: usize,
        reprocess_jobs: usize,
        quality_metrics: usize,
    }

    async fn run_phase6_quality_smoke_fixture(
        pool: SqlitePool,
        book_id: &str,
    ) -> anyhow::Result<Phase6QualitySmokeSummary> {
        seed_phase6_quality_smoke_foundation(&pool, book_id).await?;
        let quality_repo = QualityRepo::new(pool.clone());
        quality_repo
            .upsert_quarantined_claim(crate::storage::db::v4::quality_repo::NewQuarantinedClaim {
                id: Some("phase6_smoke_quarantine"),
                book_id,
                claim_id: "phase6_smoke_accept_claim",
                reason_code: "phase6_smoke_accept",
                reason_text: Some("smoke quarantined claim accepted through correction path"),
                suggested_action: "accept",
                priority: 1,
            })
            .await?;

        let validation =
            crate::service::v4::correction::CorrectionValidationService::new(pool.clone());
        validation
            .validate_and_create(crate::service::v4::correction::CorrectionCommand {
                book_id: book_id.to_string(),
                target_type: "claim".to_string(),
                target_id: "phase6_smoke_accept_claim".to_string(),
                correction_type: "accept_quarantined_claim".to_string(),
                correction_json: serde_json::json!({ "decision": "accept" }).to_string(),
                source: "test".to_string(),
                source_claim_id: Some("phase6_smoke_accept_claim".to_string()),
                source_span_id: Some("phase6_smoke_span_accept".to_string()),
                created_by: "phase6-smoke".to_string(),
            })
            .await?;
        let reject_correction = validation
            .validate_and_create(crate::service::v4::correction::CorrectionCommand {
                book_id: book_id.to_string(),
                target_type: "claim".to_string(),
                target_id: "phase6_smoke_rejected_claim".to_string(),
                correction_type: "reject_claim".to_string(),
                correction_json: serde_json::json!({ "decision": "reject" }).to_string(),
                source: "test".to_string(),
                source_claim_id: Some("phase6_smoke_rejected_claim".to_string()),
                source_span_id: Some("phase6_smoke_span_reject".to_string()),
                created_by: "phase6-smoke".to_string(),
            })
            .await?;
        let apply_result =
            crate::service::v4::correction_applier::CorrectionApplier::new(pool.clone())
                .apply(&reject_correction.id, "phase6-smoke")
                .await?;
        anyhow::ensure!(
            apply_result.status
                == crate::service::v4::correction_applier::CorrectionApplyStatus::Applied,
            "phase6 smoke reject correction was not applied"
        );
        let correction_events = quality_repo
            .list_correction_events(&reject_correction.id)
            .await?
            .len();

        let before_audit = phase6_smoke_canonical_row_count(&pool, book_id).await?;
        crate::service::v4::quality_audit::QualityAuditService::new(pool.clone())
            .run_with_findings(
                book_id,
                "full_book_quality",
                "{\"fixture\":\"phase6-quality-smoke\"}",
                phase6_quality_smoke_findings(),
            )
            .await?;
        let after_audit = phase6_smoke_canonical_row_count(&pool, book_id).await?;
        let audit_findings = quality_repo
            .list_audit_findings(book_id, Some("open"), None, None, None, 50)
            .await?;

        let before_prompt = phase6_smoke_canonical_row_count(&pool, book_id).await?;
        let prompt_run = PromptRegressionRunner::new(pool.clone())
            .run_fixture_set(PromptRegressionRequest {
                book_id: book_id.to_string(),
                prompt_version: "phase6-smoke".to_string(),
                schema_version: "v4".to_string(),
                model: "deterministic-fixture".to_string(),
                fixture_set: "phase6-quality-smoke".to_string(),
            })
            .await?;
        let prompt_results = quality_repo
            .list_prompt_regression_results(&prompt_run.id)
            .await?;
        let after_prompt = phase6_smoke_canonical_row_count(&pool, book_id).await?;

        let reprocess_service = crate::service::v4::reprocess::ReprocessService::new(pool.clone());
        let reprocess_job = reprocess_service
            .queue(crate::service::v4::reprocess::ReprocessRequest {
                book_id: book_id.to_string(),
                scope_type: "full_book".to_string(),
                scope_json: "{}".to_string(),
                mode: "dry_run_compare".to_string(),
                requested_by: "phase6-smoke".to_string(),
                reason: Some("phase6 smoke dry-run reprocess".to_string()),
                dry_run: true,
                prompt_version: Some("phase6-smoke".to_string()),
                schema_version: Some("v4".to_string()),
            })
            .await?;
        let completed_reprocess = reprocess_service.run_job(&reprocess_job.id).await?;
        anyhow::ensure!(
            completed_reprocess.status == "completed",
            "phase6 smoke reprocess did not complete"
        );

        let metrics = crate::service::v4::quality_metrics::QualityMetricsService::new(pool)
            .compute_and_store(book_id)
            .await?;

        Ok(Phase6QualitySmokeSummary {
            audit_findings: audit_findings.len(),
            prompt_regression_results: prompt_results.len(),
            audit_canonical_mutations: after_audit - before_audit,
            prompt_regression_canonical_mutations: after_prompt - before_prompt,
            correction_events,
            reprocess_jobs: usize::from(completed_reprocess.status == "completed"),
            quality_metrics: metrics.metrics.len(),
        })
    }

    async fn seed_phase6_quality_smoke_foundation(
        pool: &SqlitePool,
        book_id: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO reading_progress (book_id, max_read_chapter) VALUES (?, 2)",
        )
        .bind(book_id)
        .execute(pool)
        .await?;
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('phase6_smoke_chapter', ?, 1, 'phase6 quality smoke text', 'phase6_smoke_hash', datetime('now'))")
            .bind(book_id)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('phase6_smoke_segment', ?, 'phase6_smoke_chapter', 'phase6_smoke_hash', 0, datetime('now'))")
            .bind(book_id)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('phase6_smoke_span_accept', ?, 'phase6_smoke_chapter', 'phase6_smoke_hash', 'phase6_smoke_segment', 0, 0, 12, 'accept text', datetime('now'))")
            .bind(book_id)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('phase6_smoke_span_reject', ?, 'phase6_smoke_chapter', 'phase6_smoke_hash', 'phase6_smoke_segment', 1, 13, 24, 'reject text', datetime('now'))")
            .bind(book_id)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('phase6_smoke_ai_run', ?, 'phase6_smoke_chapter', 'extract', 'smoke', 'phase6', 1, 'phase6_smoke_input', 'completed', datetime('now'))")
            .bind(book_id)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('phase6_smoke_accept_claim', ?, 1, 'property_update', 'phase6_smoke_accept', 'phase6_smoke_span_accept', 'phase6_smoke_ai_run', 0.7, 'high', 'quarantined', datetime('now'), datetime('now'))")
            .bind(book_id)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('phase6_smoke_rejected_claim', ?, 1, 'property_update', 'phase6_smoke_reject', 'phase6_smoke_span_reject', 'phase6_smoke_ai_run', 0.4, 'high', 'rejected', datetime('now'), datetime('now'))")
            .bind(book_id)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('phase6_smoke_entity', ?, 'character', '林澈', '林澈', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .bind(book_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    fn phase6_quality_smoke_findings() -> Vec<crate::service::v4::quality_audit::AuditFindingDraft>
    {
        vec![
            phase6_smoke_finding(
                "duplicate_entity_candidate",
                "entity",
                "phase6_smoke_entity",
                "merge_entities",
            ),
            phase6_smoke_finding(
                "relationship_pollution_candidate",
                "relationship",
                "phase6_smoke_relationship",
                "deactivate_relationship",
            ),
            phase6_smoke_finding(
                "knowledge_topic_drift_candidate",
                "knowledge_card",
                "phase6_smoke_knowledge",
                "revise_knowledge_assertion",
            ),
            phase6_smoke_finding(
                "map_conflict_candidate",
                "place_edge",
                "phase6_smoke_place_edge",
                "mark_place_edge_conflict",
            ),
            phase6_smoke_finding(
                "prompt_regression_failure",
                "prompt_regression_run",
                "phase6_smoke_prompt_run",
                "needs_manual_review",
            ),
            phase6_smoke_finding(
                "stale_quarantined_claim",
                "claim",
                "phase6_smoke_rejected_claim",
                "retry_claim",
            ),
        ]
    }

    fn phase6_smoke_finding(
        finding_type: &str,
        target_type: &str,
        target_id: &str,
        suggested_action: &str,
    ) -> crate::service::v4::quality_audit::AuditFindingDraft {
        crate::service::v4::quality_audit::AuditFindingDraft {
            finding_type: finding_type.to_string(),
            severity: "medium".to_string(),
            target_type: target_type.to_string(),
            target_id: target_id.to_string(),
            related_target_type: None,
            related_target_id: None,
            reason_code: format!("phase6_smoke_{finding_type}"),
            reason_text: Some("phase6 quality smoke finding".to_string()),
            evidence_json: Some(
                serde_json::json!({ "fixture": "phase6-quality-smoke" }).to_string(),
            ),
            suggested_action: suggested_action.to_string(),
        }
    }

    async fn phase6_smoke_canonical_row_count(
        pool: &SqlitePool,
        book_id: &str,
    ) -> anyhow::Result<i64> {
        let entities: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entities WHERE book_id = ?")
            .bind(book_id)
            .fetch_one(pool)
            .await?;
        let relationships: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM relationships WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(pool)
                .await?;
        let knowledge_cards: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM knowledge_cards WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(pool)
                .await?;
        let knowledge_assertions: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM knowledge_assertions WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(pool)
                .await?;
        let place_details: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM place_details WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(pool)
                .await?;
        let place_edges: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM place_edges WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(pool)
                .await?;
        Ok(entities.0
            + relationships.0
            + knowledge_cards.0
            + knowledge_assertions.0
            + place_details.0
            + place_edges.0)
    }

    #[tokio::test]
    async fn real_ai_smoke_test_quality_phase6_fixture() {
        if std::env::var("RUN_REAL_AI_TESTS").unwrap_or_default() != "1" {
            return;
        }

        let pool = setup_test_db().await;
        let summary = run_phase6_quality_smoke_fixture(pool, "phase6-quality-smoke")
            .await
            .unwrap();

        assert!(summary.audit_findings >= 6);
        assert!(summary.prompt_regression_results >= 15);
        assert_eq!(summary.audit_canonical_mutations, 0);
        assert_eq!(summary.prompt_regression_canonical_mutations, 0);
        assert!(summary.correction_events >= 1);
        assert_eq!(summary.reprocess_jobs, 1);
        assert!(summary.quality_metrics >= 6);
    }
}
