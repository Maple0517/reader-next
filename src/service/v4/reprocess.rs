use crate::storage::db::v4::quality_repo::{NewReprocessJob, QualityRepo, ReprocessJobRecord};
use serde_json::Value;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct ReprocessRequest {
    pub book_id: String,
    pub scope_type: String,
    pub scope_json: String,
    pub mode: String,
    pub requested_by: String,
    pub reason: Option<String>,
    pub dry_run: bool,
    pub prompt_version: Option<String>,
    pub schema_version: Option<String>,
}

pub struct ReprocessService {
    pool: SqlitePool,
}

impl ReprocessService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn queue(&self, request: ReprocessRequest) -> anyhow::Result<ReprocessJobRecord> {
        self.validate_request(&request).await?;
        let mut tx = self.pool.begin().await?;
        let job = QualityRepo::create_reprocess_job_with_conn(
            &mut *tx,
            NewReprocessJob {
                id: None,
                book_id: &request.book_id,
                scope_type: &request.scope_type,
                scope_json: &request.scope_json,
                mode: &request.mode,
                requested_by: &request.requested_by,
                reason: request.reason.as_deref(),
                dry_run: request.dry_run,
                prompt_version: request.prompt_version.as_deref(),
                schema_version: request.schema_version.as_deref(),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(job)
    }

    pub async fn run_job(&self, job_id: &str) -> anyhow::Result<ReprocessJobRecord> {
        let mut tx = self.pool.begin().await?;
        let job = QualityRepo::get_reprocess_job_with_conn(&mut *tx, job_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("reprocess job not found"))?;
        QualityRepo::update_reprocess_job_status_with_conn(
            &mut *tx, &job.id, "running", None, None,
        )
        .await?;

        if job.dry_run == 1 || job.mode == "dry_run_compare" {
            let result_json = serde_json::json!({
                "dryRun": job.dry_run == 1,
                "mode": job.mode,
                "scopeType": job.scope_type,
                "mutations": 0
            })
            .to_string();
            QualityRepo::update_reprocess_job_status_with_conn(
                &mut *tx,
                &job.id,
                "completed",
                Some(&result_json),
                None,
            )
            .await?;
        } else if job.mode == "rebuild_projection" {
            sqlx::query("DELETE FROM view_model_cache WHERE book_id = ?")
                .bind(&job.book_id)
                .execute(&mut *tx)
                .await?;
            let result_json = serde_json::json!({
                "dryRun": false,
                "mode": job.mode,
                "scopeType": job.scope_type,
                "projectionCache": "invalidated",
                "mutations": 1
            })
            .to_string();
            QualityRepo::update_reprocess_job_status_with_conn(
                &mut *tx,
                &job.id,
                "completed",
                Some(&result_json),
                None,
            )
            .await?;
        } else {
            let error = format!("reprocess mode not implemented: {}", job.mode);
            QualityRepo::update_reprocess_job_status_with_conn(
                &mut *tx,
                &job.id,
                "failed",
                None,
                Some(&error),
            )
            .await?;
        }

        let updated = QualityRepo::get_reprocess_job_with_conn(&mut *tx, &job.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("updated reprocess job missing"))?;
        tx.commit().await?;
        Ok(updated)
    }

    async fn validate_request(&self, request: &ReprocessRequest) -> anyhow::Result<()> {
        validate_mode(&request.mode)?;
        let scope: Value = serde_json::from_str(&request.scope_json)
            .map_err(|err| anyhow::anyhow!("invalid scope_json: {err}"))?;
        let max_read = self.max_read_chapter(&request.book_id).await?;

        match request.scope_type.as_str() {
            "chapter" => {
                let chapter_index = json_i64(&scope, "chapterIndex")?;
                ensure_within_max_read(chapter_index, max_read)?;
            }
            "chapter_range" => {
                let start = json_i64(&scope, "startChapter")?;
                let end = json_i64(&scope, "endChapter")?;
                if start > end {
                    anyhow::bail!("chapter_range startChapter must be <= endChapter");
                }
                ensure_within_max_read(end, max_read)?;
            }
            "claim" => {
                let claim_id = json_str(&scope, "claimId")?;
                let row: Option<(i64,)> =
                    sqlx::query_as("SELECT chapter_index FROM claims WHERE book_id = ? AND id = ?")
                        .bind(&request.book_id)
                        .bind(claim_id)
                        .fetch_optional(&self.pool)
                        .await?;
                let chapter_index = row
                    .map(|row| row.0)
                    .ok_or_else(|| anyhow::anyhow!("claim not found for reprocess scope"))?;
                ensure_within_max_read(chapter_index, max_read)?;
            }
            "claim_type" | "domain" | "prompt_version" | "full_book" => {}
            other => anyhow::bail!("unsupported reprocess scope_type: {other}"),
        }

        Ok(())
    }

    async fn max_read_chapter(&self, book_id: &str) -> anyhow::Result<i64> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT max_read_chapter FROM reading_progress WHERE book_id = ?")
                .bind(book_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|row| row.0).unwrap_or(0))
    }
}

fn validate_mode(mode: &str) -> anyhow::Result<()> {
    match mode {
        "retry_failed" | "reprocess_claims" | "reprocess_chapters" | "rebuild_projection"
        | "rebuild_domain" | "dry_run_compare" => Ok(()),
        other => anyhow::bail!("unsupported reprocess mode: {other}"),
    }
}

fn json_i64(scope: &Value, key: &str) -> anyhow::Result<i64> {
    scope
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow::anyhow!("scope_json missing integer {key}"))
}

fn json_str<'a>(scope: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    scope
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("scope_json missing string {key}"))
}

fn ensure_within_max_read(chapter_index: i64, max_read: i64) -> anyhow::Result<()> {
    if chapter_index > max_read {
        anyhow::bail!(
            "reprocess scope chapter {} exceeds max_read_chapter {}",
            chapter_index,
            max_read
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::storage::db::v4::cache_repo::CacheRepo;
    use crate::storage::db::v4::quality_repo::QualityRepo;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("reader-reprocess-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    async fn set_max_read(pool: &SqlitePool, book_id: &str, max_read: i64) {
        sqlx::query(
            "INSERT OR REPLACE INTO reading_progress (book_id, max_read_chapter) VALUES (?, ?)",
        )
        .bind(book_id)
        .bind(max_read)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn seed_claim(pool: &SqlitePool, claim_id: &str, chapter_index: i64, status: &str) {
        sqlx::query("INSERT OR IGNORE INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch_reprocess', 'b1', 1, 'text', 'hash_reprocess', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg_reprocess', 'b1', 'ch_reprocess', 'hash_reprocess', 0, datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span_reprocess', 'b1', 'ch_reprocess', 'hash_reprocess', 'seg_reprocess', 0, 0, 10, 'text', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run_reprocess', 'b1', 'ch_reprocess', 'extract', 'test', 'v1', 1, 'input', 'completed', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', ?, 'property_update', 'test', 'span_reprocess', 'run_reprocess', 0.8, 'medium', ?, datetime('now'), datetime('now'))")
            .bind(claim_id)
            .bind(chapter_index)
            .bind(status)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reprocess_rejects_chapter_scope_past_max_read_without_job() {
        let pool = setup_test_db().await;
        set_max_read(&pool, "b1", 3).await;
        let service = ReprocessService::new(pool.clone());

        let err = service
            .queue(ReprocessRequest {
                book_id: "b1".to_string(),
                scope_type: "chapter".to_string(),
                scope_json: serde_json::json!({ "chapterIndex": 4 }).to_string(),
                mode: "reprocess_chapters".to_string(),
                requested_by: "tester".to_string(),
                reason: Some("too far".to_string()),
                dry_run: false,
                prompt_version: None,
                schema_version: None,
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("max_read_chapter"));
        let jobs = QualityRepo::new(pool)
            .list_reprocess_jobs("b1", None, None, 10)
            .await
            .unwrap();
        assert!(jobs.is_empty());
    }

    #[tokio::test]
    async fn reprocess_dry_run_compare_completes_without_mutating_claims() {
        let pool = setup_test_db().await;
        set_max_read(&pool, "b1", 5).await;
        seed_claim(&pool, "claim_dry_run", 2, "accepted").await;
        let service = ReprocessService::new(pool.clone());
        let job = service
            .queue(ReprocessRequest {
                book_id: "b1".to_string(),
                scope_type: "chapter".to_string(),
                scope_json: serde_json::json!({ "chapterIndex": 2 }).to_string(),
                mode: "dry_run_compare".to_string(),
                requested_by: "tester".to_string(),
                reason: None,
                dry_run: true,
                prompt_version: Some("v2".to_string()),
                schema_version: Some("v4".to_string()),
            })
            .await
            .unwrap();

        let completed = service.run_job(&job.id).await.unwrap();

        assert_eq!(completed.status, "completed");
        assert!(completed.result_json.unwrap().contains("\"mutations\":0"));
        let status: (String,) =
            sqlx::query_as("SELECT status FROM claims WHERE id = 'claim_dry_run'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status.0, "accepted");
    }

    #[tokio::test]
    async fn reprocess_rebuild_projection_invalidates_projection_cache_without_claim_mutation() {
        let pool = setup_test_db().await;
        set_max_read(&pool, "b1", 5).await;
        seed_claim(&pool, "claim_rebuild_projection", 2, "accepted").await;
        let cache_repo = CacheRepo::new(pool.clone());
        cache_repo
            .set_cached("b1", "character_list", "__book__", 10, "{\"stale\":true}")
            .await
            .unwrap();
        cache_repo
            .set_cached(
                "b1",
                "relationship_graph",
                "__book__",
                10,
                "{\"stale\":true}",
            )
            .await
            .unwrap();
        let service = ReprocessService::new(pool.clone());
        let job = service
            .queue(ReprocessRequest {
                book_id: "b1".to_string(),
                scope_type: "full_book".to_string(),
                scope_json: serde_json::json!({}).to_string(),
                mode: "rebuild_projection".to_string(),
                requested_by: "tester".to_string(),
                reason: Some("cache stale".to_string()),
                dry_run: false,
                prompt_version: None,
                schema_version: None,
            })
            .await
            .unwrap();

        let completed = service.run_job(&job.id).await.unwrap();

        assert_eq!(completed.status, "completed");
        assert!(completed
            .result_json
            .as_deref()
            .unwrap_or_default()
            .contains("projectionCache"));
        assert!(cache_repo
            .get_cached("b1", "character_list", "__book__", 10)
            .await
            .unwrap()
            .is_none());
        assert!(cache_repo
            .get_cached("b1", "relationship_graph", "__book__", 10)
            .await
            .unwrap()
            .is_none());
        let status: (String,) =
            sqlx::query_as("SELECT status FROM claims WHERE id = 'claim_rebuild_projection'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status.0, "accepted");
    }

    #[tokio::test]
    async fn reprocess_failed_job_records_error_without_claim_mutation() {
        let pool = setup_test_db().await;
        set_max_read(&pool, "b1", 5).await;
        seed_claim(&pool, "claim_failed_job", 2, "accepted").await;
        let service = ReprocessService::new(pool.clone());
        let job = service
            .queue(ReprocessRequest {
                book_id: "b1".to_string(),
                scope_type: "chapter".to_string(),
                scope_json: serde_json::json!({ "chapterIndex": 2 }).to_string(),
                mode: "reprocess_chapters".to_string(),
                requested_by: "tester".to_string(),
                reason: Some("not implemented yet".to_string()),
                dry_run: false,
                prompt_version: None,
                schema_version: None,
            })
            .await
            .unwrap();

        let failed = service.run_job(&job.id).await.unwrap();

        assert_eq!(failed.status, "failed");
        assert!(failed.error.unwrap().contains("not implemented"));
        let status: (String,) =
            sqlx::query_as("SELECT status FROM claims WHERE id = 'claim_failed_job'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status.0, "accepted");
    }
}
