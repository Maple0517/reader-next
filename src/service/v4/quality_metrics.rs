use crate::storage::db::v4::quality_repo::{NewQualityMetric, QualityMetricRecord, QualityRepo};
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct QualityMetricsSummary {
    pub book_id: String,
    pub metrics: Vec<QualityMetricRecord>,
}

impl QualityMetricsSummary {
    pub fn value(&self, metric_type: &str) -> Option<f64> {
        self.metrics
            .iter()
            .find(|metric| metric.metric_type == metric_type)
            .map(|metric| metric.metric_value)
    }
}

pub struct QualityMetricsService {
    pool: SqlitePool,
}

impl QualityMetricsService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn compute_and_store(&self, book_id: &str) -> anyhow::Result<QualityMetricsSummary> {
        let metrics = vec![
            (
                "quarantined_claim_count",
                self.count_claims(book_id, "quarantined").await?,
            ),
            (
                "uncertain_claim_count",
                self.count_claims(book_id, "uncertain").await?,
            ),
            (
                "rejected_claim_rate",
                self.rejected_claim_rate(book_id).await?,
            ),
            (
                "duplicate_entity_candidate_count",
                self.count_findings(book_id, "duplicate_entity_candidate")
                    .await?,
            ),
            (
                "relationship_pollution_candidate_count",
                self.count_findings(book_id, "relationship_pollution_candidate")
                    .await?,
            ),
            (
                "knowledge_topic_drift_count",
                self.count_findings(book_id, "knowledge_topic_drift_candidate")
                    .await?,
            ),
            (
                "map_conflict_count",
                self.count_findings(book_id, "map_conflict_candidate")
                    .await?,
            ),
            (
                "prompt_regression_failure_count",
                self.count_prompt_regression_failures(book_id).await?,
            ),
            (
                "processing_failure_rate",
                self.processing_failure_rate(book_id).await?,
            ),
            (
                "evidence_missing_count",
                self.evidence_missing_count(book_id).await?,
            ),
            (
                "projection_rebuild_failure_count",
                self.projection_rebuild_failure_count(book_id).await?,
            ),
        ];

        let repo = QualityRepo::new(self.pool.clone());
        for (metric_type, metric_value) in metrics {
            repo.insert_metric(NewQualityMetric {
                id: None,
                book_id,
                metric_type,
                metric_value,
                metric_json: None,
            })
            .await?;
        }
        self.latest_summary(book_id).await
    }

    pub async fn latest_summary(&self, book_id: &str) -> anyhow::Result<QualityMetricsSummary> {
        let metrics = QualityRepo::new(self.pool.clone())
            .latest_metrics(book_id)
            .await?;
        Ok(QualityMetricsSummary {
            book_id: book_id.to_string(),
            metrics,
        })
    }

    async fn count_claims(&self, book_id: &str, status: &str) -> anyhow::Result<f64> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM claims WHERE book_id = ? AND status = ?")
                .bind(book_id)
                .bind(status)
                .fetch_one(&self.pool)
                .await?;
        Ok(count.0 as f64)
    }

    async fn rejected_claim_rate(&self, book_id: &str) -> anyhow::Result<f64> {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM claims WHERE book_id = ?")
            .bind(book_id)
            .fetch_one(&self.pool)
            .await?;
        if total.0 == 0 {
            return Ok(0.0);
        }
        let rejected = self.count_claims(book_id, "rejected").await?;
        Ok(rejected / total.0 as f64)
    }

    async fn count_findings(&self, book_id: &str, finding_type: &str) -> anyhow::Result<f64> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM quality_audit_findings
             WHERE book_id = ? AND finding_type = ? AND status = 'open'",
        )
        .bind(book_id)
        .bind(finding_type)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0 as f64)
    }

    async fn count_prompt_regression_failures(&self, book_id: &str) -> anyhow::Result<f64> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT
                (
                    SELECT COUNT(*)
                    FROM prompt_regression_results results
                    JOIN prompt_regression_runs runs ON runs.id = results.run_id
                    WHERE runs.book_id = ? AND results.pass = 0
                ) + (
                    SELECT COUNT(*)
                    FROM prompt_regression_runs runs
                    WHERE runs.book_id = ?
                      AND runs.status = 'failed'
                      AND NOT EXISTS (
                          SELECT 1
                          FROM prompt_regression_results results
                          WHERE results.run_id = runs.id AND results.pass = 0
                      )
                )
            "#,
        )
        .bind(book_id)
        .bind(book_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0 as f64)
    }

    async fn processing_failure_rate(&self, book_id: &str) -> anyhow::Result<f64> {
        let total: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM chapter_processing_runs WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(&self.pool)
                .await?;
        if total.0 == 0 {
            return Ok(0.0);
        }
        let failed: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM chapter_processing_runs WHERE book_id = ? AND status = 'failed'",
        )
        .bind(book_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(failed.0 as f64 / total.0 as f64)
    }

    async fn evidence_missing_count(&self, book_id: &str) -> anyhow::Result<f64> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM claims c
             LEFT JOIN claim_source_spans css ON css.claim_id = c.id
             WHERE c.book_id = ? AND css.claim_id IS NULL",
        )
        .bind(book_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0 as f64)
    }

    async fn projection_rebuild_failure_count(&self, book_id: &str) -> anyhow::Result<f64> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM reprocess_jobs
             WHERE book_id = ? AND mode = 'rebuild_projection' AND status = 'failed'",
        )
        .bind(book_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0 as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::storage::db::v4::quality_repo::{
        NewAuditFinding, NewAuditRun, NewPromptRegressionResult, NewPromptRegressionRun,
        QualityRepo,
    };
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir =
            std::env::temp_dir().join(format!("reader-quality-metrics-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    async fn seed_claim_status(pool: &SqlitePool, claim_id: &str, status: &str) {
        sqlx::query("INSERT OR IGNORE INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch_metrics', 'b1', 1, 'text', 'hash_metrics', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT OR IGNORE INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg_metrics', 'b1', 'ch_metrics', 'hash_metrics', 0, datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT OR IGNORE INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span_metrics', 'b1', 'ch_metrics', 'hash_metrics', 'seg_metrics', 0, 0, 10, 'text', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT OR IGNORE INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run_metrics', 'b1', 'ch_metrics', 'extract', 'test', 'v1', 1, 'input', 'completed', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', 1, 'property_update', 'test', 'span_metrics', 'run_metrics', 0.8, 'medium', ?, datetime('now'), datetime('now'))")
            .bind(claim_id).bind(status).execute(pool).await.unwrap();
    }

    async fn seed_finding(repo: &QualityRepo, finding_type: &str) {
        let run = repo
            .create_audit_run(NewAuditRun {
                id: None,
                book_id: "b1",
                audit_type: "full_book_quality",
                scope_json: "{}",
                status: "completed",
            })
            .await
            .unwrap();
        repo.create_audit_finding(NewAuditFinding {
            id: None,
            book_id: "b1",
            audit_run_id: &run.id,
            finding_type,
            severity: "medium",
            target_type: "entity",
            target_id: "target",
            related_target_type: None,
            related_target_id: None,
            reason_code: "test",
            reason_text: None,
            evidence_json: None,
            suggested_action: "needs_manual_review",
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn quality_metrics_computes_claim_and_finding_counts() {
        let pool = setup_test_db().await;
        seed_claim_status(&pool, "claim_q", "quarantined").await;
        seed_claim_status(&pool, "claim_u", "uncertain").await;
        let repo = QualityRepo::new(pool.clone());
        seed_finding(&repo, "duplicate_entity_candidate").await;
        seed_finding(&repo, "relationship_pollution_candidate").await;
        seed_finding(&repo, "knowledge_topic_drift_candidate").await;

        let summary = QualityMetricsService::new(pool.clone())
            .compute_and_store("b1")
            .await
            .unwrap();

        assert_eq!(summary.value("quarantined_claim_count"), Some(1.0));
        assert_eq!(summary.value("uncertain_claim_count"), Some(1.0));
        assert_eq!(summary.value("duplicate_entity_candidate_count"), Some(1.0));
        assert_eq!(
            summary.value("relationship_pollution_candidate_count"),
            Some(1.0)
        );
        assert_eq!(summary.value("knowledge_topic_drift_count"), Some(1.0));
    }

    #[tokio::test]
    async fn quality_metrics_latest_summary_returns_newest_values() {
        let pool = setup_test_db().await;
        let service = QualityMetricsService::new(pool.clone());
        service.compute_and_store("b1").await.unwrap();
        seed_claim_status(&pool, "claim_q2", "quarantined").await;
        service.compute_and_store("b1").await.unwrap();

        let summary = service.latest_summary("b1").await.unwrap();

        assert_eq!(summary.value("quarantined_claim_count"), Some(1.0));
        assert!(summary.metrics.len() >= 6);
    }

    #[tokio::test]
    async fn quality_metrics_counts_prompt_regression_failures() {
        let pool = setup_test_db().await;
        let repo = QualityRepo::new(pool.clone());
        repo.create_prompt_regression_run(NewPromptRegressionRun {
            id: Some("prompt_failed"),
            book_id: "b1",
            prompt_version: "v1",
            schema_version: "v4",
            model: "mock",
            fixture_set: "phase6",
            status: "failed",
        })
        .await
        .unwrap();

        let summary = QualityMetricsService::new(pool)
            .compute_and_store("b1")
            .await
            .unwrap();

        assert_eq!(summary.value("prompt_regression_failure_count"), Some(1.0));
    }

    #[tokio::test]
    async fn quality_metrics_counts_failed_prompt_regression_results() {
        let pool = setup_test_db().await;
        let repo = QualityRepo::new(pool.clone());
        let run = repo
            .create_prompt_regression_run(NewPromptRegressionRun {
                id: Some("prompt_completed_with_failure"),
                book_id: "b1",
                prompt_version: "v1",
                schema_version: "v4",
                model: "mock",
                fixture_set: "phase6",
                status: "completed",
            })
            .await
            .unwrap();
        repo.insert_prompt_regression_result(NewPromptRegressionResult {
            id: Some("prompt_failed_case"),
            run_id: &run.id,
            case_id: "case-failed",
            case_name: "failed case",
            domain: "quality",
            expected_json: "{\"ok\":true}",
            actual_json: "{\"ok\":false}",
            pass: false,
            diff_json: Some("{\"ok\":false}"),
        })
        .await
        .unwrap();

        let summary = QualityMetricsService::new(pool)
            .compute_and_store("b1")
            .await
            .unwrap();

        assert_eq!(summary.value("prompt_regression_failure_count"), Some(1.0));
    }
}
