use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct ProcessingProgressRecord {
    pub book_id: String,
    pub max_processed_chapter: i64,
    pub status: String,
    pub target_chapter: Option<i64>,
    pub current_chapter: Option<i64>,
    pub current_segment_id: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ChapterProcessingRunRecord {
    pub id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub chapter_hash: String,
    pub prompt_version: String,
    pub schema_version: i64,
    pub status: String,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

pub struct ProgressRepo {
    pool: SqlitePool,
}

impl ProgressRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Processing Progress ---

    pub async fn get_progress(
        &self,
        book_id: &str,
    ) -> anyhow::Result<Option<ProcessingProgressRecord>> {
        let row = sqlx::query_as::<_, ProgressRow>(
            "SELECT book_id, max_processed_chapter, status, target_chapter, current_chapter, current_segment_id, last_error, updated_at
             FROM processing_progress WHERE book_id = ?"
        )
        .bind(book_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn init_progress(&self, book_id: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR IGNORE INTO processing_progress (book_id, max_processed_chapter, status, updated_at)
             VALUES (?, 0, 'idle', ?)"
        )
        .bind(book_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn set_status(
        &self,
        book_id: &str,
        status: &str,
        target_chapter: Option<i64>,
        current_chapter: Option<i64>,
        current_segment_id: Option<&str>,
        last_error: Option<&str>,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE processing_progress SET status = ?, target_chapter = ?, current_chapter = ?, current_segment_id = ?, last_error = ?, updated_at = ? WHERE book_id = ?"
        )
        .bind(status)
        .bind(target_chapter)
        .bind(current_chapter)
        .bind(current_segment_id)
        .bind(last_error)
        .bind(&now)
        .bind(book_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn advance_processed_chapter(
        &self,
        book_id: &str,
        chapter_index: i64,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE processing_progress SET max_processed_chapter = ?, updated_at = ? WHERE book_id = ? AND max_processed_chapter < ?"
        )
        .bind(chapter_index)
        .bind(&now)
        .bind(book_id)
        .bind(chapter_index)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // --- Chapter Processing Runs ---

    pub async fn create_run(
        &self,
        book_id: &str,
        chapter_index: i64,
        chapter_hash: &str,
        prompt_version: &str,
        schema_version: i64,
    ) -> anyhow::Result<ChapterProcessingRunRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Delete old non-success runs for same params to allow retry.
        // Only 'success' runs are preserved for idempotency skip.
        sqlx::query(
            "DELETE FROM chapter_processing_runs
             WHERE book_id = ? AND chapter_index = ? AND chapter_hash = ?
               AND prompt_version = ? AND schema_version = ? AND status != 'success'",
        )
        .bind(book_id)
        .bind(chapter_index)
        .bind(chapter_hash)
        .bind(prompt_version)
        .bind(schema_version)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO chapter_processing_runs (id, book_id, chapter_index, chapter_hash, prompt_version, schema_version, status, started_at)
             VALUES (?, ?, ?, ?, ?, ?, 'running', ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(chapter_index)
        .bind(chapter_hash)
        .bind(prompt_version)
        .bind(schema_version)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(ChapterProcessingRunRecord {
            id,
            book_id: book_id.to_string(),
            chapter_index,
            chapter_hash: chapter_hash.to_string(),
            prompt_version: prompt_version.to_string(),
            schema_version,
            status: "running".to_string(),
            error: None,
            started_at: now,
            finished_at: None,
        })
    }

    pub async fn complete_run(&self, run_id: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE chapter_processing_runs SET status = 'success', finished_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(run_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn fail_run(&self, run_id: &str, error: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE chapter_processing_runs SET status = 'failed', error = ?, finished_at = ? WHERE id = ?"
        )
        .bind(error)
        .bind(&now)
        .bind(run_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check idempotency: has this chapter been successfully processed with same prompt/schema?
    pub async fn is_already_processed(
        &self,
        book_id: &str,
        chapter_index: i64,
        chapter_hash: &str,
        prompt_version: &str,
        schema_version: i64,
    ) -> anyhow::Result<bool> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM chapter_processing_runs
             WHERE book_id = ? AND chapter_index = ? AND chapter_hash = ? AND prompt_version = ? AND schema_version = ? AND status = 'success'"
        )
        .bind(book_id)
        .bind(chapter_index)
        .bind(chapter_hash)
        .bind(prompt_version)
        .bind(schema_version)
        .fetch_one(&self.pool)
        .await?;

        Ok(count.0 > 0)
    }

    pub async fn get_run(
        &self,
        run_id: &str,
    ) -> anyhow::Result<Option<ChapterProcessingRunRecord>> {
        let row = sqlx::query_as::<_, RunRow>(
            "SELECT id, book_id, chapter_index, chapter_hash, prompt_version, schema_version, status, error, started_at, finished_at
             FROM chapter_processing_runs WHERE id = ?"
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }
}

// SQLx row types

#[derive(sqlx::FromRow)]
struct ProgressRow {
    book_id: String,
    max_processed_chapter: i64,
    status: String,
    target_chapter: Option<i64>,
    current_chapter: Option<i64>,
    current_segment_id: Option<String>,
    last_error: Option<String>,
    updated_at: String,
}

impl From<ProgressRow> for ProcessingProgressRecord {
    fn from(r: ProgressRow) -> Self {
        Self {
            book_id: r.book_id,
            max_processed_chapter: r.max_processed_chapter,
            status: r.status,
            target_chapter: r.target_chapter,
            current_chapter: r.current_chapter,
            current_segment_id: r.current_segment_id,
            last_error: r.last_error,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct RunRow {
    id: String,
    book_id: String,
    chapter_index: i64,
    chapter_hash: String,
    prompt_version: String,
    schema_version: i64,
    status: String,
    error: Option<String>,
    started_at: String,
    finished_at: Option<String>,
}

impl From<RunRow> for ChapterProcessingRunRecord {
    fn from(r: RunRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            chapter_index: r.chapter_index,
            chapter_hash: r.chapter_hash,
            prompt_version: r.prompt_version,
            schema_version: r.schema_version,
            status: r.status,
            error: r.error,
            started_at: r.started_at,
            finished_at: r.finished_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup() -> (SqlitePool, ProgressRepo) {
        let dir = std::env::temp_dir().join(format!("reader-v4-prog-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        let repo = ProgressRepo::new(pool.clone());
        (pool, repo)
    }

    #[tokio::test]
    async fn init_and_get_progress() {
        let (_pool, repo) = setup().await;
        repo.init_progress("b1").await.unwrap();

        let prog = repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(prog.max_processed_chapter, 0);
        assert_eq!(prog.status, "idle");

        // Idempotent
        repo.init_progress("b1").await.unwrap();
    }

    #[tokio::test]
    async fn set_status_updates() {
        let (_pool, repo) = setup().await;
        repo.init_progress("b1").await.unwrap();

        repo.set_status("b1", "running", Some(10), Some(3), Some("seg1"), None)
            .await
            .unwrap();

        let prog = repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(prog.status, "running");
        assert_eq!(prog.target_chapter, Some(10));
        assert_eq!(prog.current_chapter, Some(3));
    }

    #[tokio::test]
    async fn advance_processed_chapter() {
        let (_pool, repo) = setup().await;
        repo.init_progress("b1").await.unwrap();

        repo.advance_processed_chapter("b1", 5).await.unwrap();
        let prog = repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(prog.max_processed_chapter, 5);

        // Should not decrease
        repo.advance_processed_chapter("b1", 3).await.unwrap();
        let prog = repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(prog.max_processed_chapter, 5);
    }

    #[tokio::test]
    async fn chapter_processing_run_lifecycle() {
        let (_pool, repo) = setup().await;

        let run = repo.create_run("b1", 1, "hash1", "v1", 1).await.unwrap();
        assert_eq!(run.status, "running");

        repo.complete_run(&run.id).await.unwrap();
        let completed = repo.get_run(&run.id).await.unwrap().unwrap();
        assert_eq!(completed.status, "success");
        assert!(completed.finished_at.is_some());
    }

    #[tokio::test]
    async fn chapter_processing_run_failure() {
        let (_pool, repo) = setup().await;

        let run = repo.create_run("b1", 1, "hash1", "v1", 1).await.unwrap();
        repo.fail_run(&run.id, "timeout error").await.unwrap();

        let failed = repo.get_run(&run.id).await.unwrap().unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.error.as_deref(), Some("timeout error"));
    }

    #[tokio::test]
    async fn idempotency_success_skips() {
        let (_pool, repo) = setup().await;

        // Not processed yet
        assert!(!repo
            .is_already_processed("b1", 1, "hash1", "v1", 1)
            .await
            .unwrap());

        // Process
        let run = repo.create_run("b1", 1, "hash1", "v1", 1).await.unwrap();
        repo.complete_run(&run.id).await.unwrap();

        // Now should be processed
        assert!(repo
            .is_already_processed("b1", 1, "hash1", "v1", 1)
            .await
            .unwrap());

        // Different hash → not processed
        assert!(!repo
            .is_already_processed("b1", 1, "hash2", "v1", 1)
            .await
            .unwrap());

        // Different prompt → not processed
        assert!(!repo
            .is_already_processed("b1", 1, "hash1", "v2", 1)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn failed_run_can_retry_same_chapter() {
        let (_pool, repo) = setup().await;

        // Fail first run
        let run1 = repo.create_run("b1", 1, "hash1", "v1", 1).await.unwrap();
        repo.fail_run(&run1.id, "error").await.unwrap();

        // Not "already processed" (only success counts)
        assert!(!repo
            .is_already_processed("b1", 1, "hash1", "v1", 1)
            .await
            .unwrap());

        // Retry with same params should succeed (old failed row deleted)
        let run2 = repo.create_run("b1", 1, "hash1", "v1", 1).await.unwrap();
        assert_eq!(run2.status, "running");
        assert_ne!(run2.id, run1.id, "new run should have a different id");
        repo.complete_run(&run2.id).await.unwrap();

        assert!(repo
            .is_already_processed("b1", 1, "hash1", "v1", 1)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn success_run_prevents_duplicate() {
        let (_pool, repo) = setup().await;

        // Complete first run
        let run1 = repo.create_run("b1", 1, "hash1", "v1", 1).await.unwrap();
        repo.complete_run(&run1.id).await.unwrap();

        // is_already_processed should return true
        assert!(repo
            .is_already_processed("b1", 1, "hash1", "v1", 1)
            .await
            .unwrap());

        // Different params → not processed
        assert!(!repo
            .is_already_processed("b1", 1, "hash2", "v1", 1)
            .await
            .unwrap());
        assert!(!repo
            .is_already_processed("b1", 1, "hash1", "v2", 1)
            .await
            .unwrap());
    }
}
