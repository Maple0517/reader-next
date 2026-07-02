use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct AiRunRecord {
    pub id: String,
    pub book_id: String,
    pub chapter_id: String,
    pub segment_id: Option<String>,
    pub run_type: String,
    pub model: String,
    pub prompt_version: String,
    pub schema_version: i64,
    pub input_hash: String,
    pub output_json: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

pub struct AiRunRepo {
    pool: SqlitePool,
}

impl AiRunRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new AI run with status='running'.
    pub async fn create_run(
        &self,
        book_id: &str,
        chapter_id: &str,
        segment_id: Option<&str>,
        run_type: &str,
        model: &str,
        prompt_version: &str,
        schema_version: i64,
        input_hash: &str,
    ) -> anyhow::Result<AiRunRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO ai_runs (id, book_id, chapter_id, segment_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'running', ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(chapter_id)
        .bind(segment_id)
        .bind(run_type)
        .bind(model)
        .bind(prompt_version)
        .bind(schema_version)
        .bind(input_hash)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(AiRunRecord {
            id,
            book_id: book_id.to_string(),
            chapter_id: chapter_id.to_string(),
            segment_id: segment_id.map(|s| s.to_string()),
            run_type: run_type.to_string(),
            model: model.to_string(),
            prompt_version: prompt_version.to_string(),
            schema_version,
            input_hash: input_hash.to_string(),
            output_json: None,
            status: "running".to_string(),
            error: None,
            started_at: now,
            finished_at: None,
        })
    }

    /// Update run status, output, error, and finished_at.
    pub async fn update_run_status(
        &self,
        id: &str,
        status: &str,
        output_json: Option<&str>,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE ai_runs SET status = ?, output_json = ?, error = ?, finished_at = ? WHERE id = ?"
        )
        .bind(status)
        .bind(output_json)
        .bind(error)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a run by ID.
    pub async fn get_run(&self, id: &str) -> anyhow::Result<Option<AiRunRecord>> {
        let row = sqlx::query_as::<_, AiRunRow>(
            "SELECT id, book_id, chapter_id, segment_id, run_type, model, prompt_version, schema_version, input_hash, output_json, status, error, started_at, finished_at
             FROM ai_runs WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    /// List runs for a specific chapter.
    pub async fn list_runs_by_chapter(
        &self,
        book_id: &str,
        chapter_id: &str,
    ) -> anyhow::Result<Vec<AiRunRecord>> {
        let rows = sqlx::query_as::<_, AiRunRow>(
            "SELECT id, book_id, chapter_id, segment_id, run_type, model, prompt_version, schema_version, input_hash, output_json, status, error, started_at, finished_at
             FROM ai_runs WHERE book_id = ? AND chapter_id = ? ORDER BY started_at ASC"
        )
        .bind(book_id)
        .bind(chapter_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }
}

// SQLx row types

#[derive(sqlx::FromRow)]
struct AiRunRow {
    id: String,
    book_id: String,
    chapter_id: String,
    segment_id: Option<String>,
    run_type: String,
    model: String,
    prompt_version: String,
    schema_version: i64,
    input_hash: String,
    output_json: Option<String>,
    status: String,
    error: Option<String>,
    started_at: String,
    finished_at: Option<String>,
}

impl From<AiRunRow> for AiRunRecord {
    fn from(r: AiRunRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            chapter_id: r.chapter_id,
            segment_id: r.segment_id,
            run_type: r.run_type,
            model: r.model,
            prompt_version: r.prompt_version,
            schema_version: r.schema_version,
            input_hash: r.input_hash,
            output_json: r.output_json,
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

    async fn setup() -> (SqlitePool, AiRunRepo) {
        let dir = std::env::temp_dir().join(format!("reader-v4-airun-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();

        // Insert a chapter for FK reference
        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 1, 'text', 'hash', datetime('now'))")
            .bind(&chapter_id)
            .execute(&pool)
            .await
            .unwrap();

        let repo = AiRunRepo::new(pool.clone());
        (pool, repo)
    }

    async fn get_chapter_id(pool: &SqlitePool) -> String {
        let row: (String,) =
            sqlx::query_as("SELECT id FROM chapters WHERE book_id = 'b1' AND chapter_index = 1")
                .fetch_one(pool)
                .await
                .unwrap();
        row.0
    }

    #[tokio::test]
    async fn create_run_sets_running_status() {
        let (pool, repo) = setup().await;
        let chapter_id = get_chapter_id(&pool).await;

        let run = repo
            .create_run(
                "b1",
                &chapter_id,
                None,
                "extract",
                "gpt-4",
                "v1",
                1,
                "input_hash_1",
            )
            .await
            .unwrap();

        assert_eq!(run.status, "running");
        assert_eq!(run.book_id, "b1");
        assert_eq!(run.run_type, "extract");
        assert_eq!(run.model, "gpt-4");
        assert!(run.output_json.is_none());
        assert!(run.error.is_none());
        assert!(run.finished_at.is_none());
    }

    #[tokio::test]
    async fn create_run_with_segment_id() {
        let (pool, repo) = setup().await;
        let chapter_id = get_chapter_id(&pool).await;

        let run = repo
            .create_run(
                "b1",
                &chapter_id,
                Some("seg-123"),
                "extract",
                "gpt-4",
                "v1",
                1,
                "hash",
            )
            .await
            .unwrap();

        assert_eq!(run.segment_id, Some("seg-123".to_string()));
    }

    #[tokio::test]
    async fn get_run_by_id() {
        let (pool, repo) = setup().await;
        let chapter_id = get_chapter_id(&pool).await;

        let run = repo
            .create_run("b1", &chapter_id, None, "extract", "gpt-4", "v1", 1, "hash")
            .await
            .unwrap();

        let fetched = repo.get_run(&run.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, run.id);
        assert_eq!(fetched.status, "running");

        let not_found = repo.get_run("nonexistent").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn update_run_status_to_success() {
        let (pool, repo) = setup().await;
        let chapter_id = get_chapter_id(&pool).await;

        let run = repo
            .create_run("b1", &chapter_id, None, "extract", "gpt-4", "v1", 1, "hash")
            .await
            .unwrap();

        let output = r#"{"observations": []}"#;
        repo.update_run_status(&run.id, "success", Some(output), None)
            .await
            .unwrap();

        let updated = repo.get_run(&run.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "success");
        assert_eq!(updated.output_json.as_deref(), Some(output));
        assert!(updated.error.is_none());
        assert!(updated.finished_at.is_some());
    }

    #[tokio::test]
    async fn update_run_status_to_failed() {
        let (pool, repo) = setup().await;
        let chapter_id = get_chapter_id(&pool).await;

        let run = repo
            .create_run("b1", &chapter_id, None, "extract", "gpt-4", "v1", 1, "hash")
            .await
            .unwrap();

        repo.update_run_status(&run.id, "failed", None, Some("timeout error"))
            .await
            .unwrap();

        let updated = repo.get_run(&run.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "failed");
        assert!(updated.output_json.is_none());
        assert_eq!(updated.error.as_deref(), Some("timeout error"));
        assert!(updated.finished_at.is_some());
    }

    #[tokio::test]
    async fn list_runs_by_chapter() {
        let (pool, repo) = setup().await;
        let chapter_id = get_chapter_id(&pool).await;

        repo.create_run("b1", &chapter_id, None, "extract", "gpt-4", "v1", 1, "h1")
            .await
            .unwrap();
        repo.create_run(
            "b1",
            &chapter_id,
            Some("seg1"),
            "extract",
            "gpt-4",
            "v1",
            1,
            "h2",
        )
        .await
        .unwrap();

        let runs = repo.list_runs_by_chapter("b1", &chapter_id).await.unwrap();
        assert_eq!(runs.len(), 2);
    }

    #[tokio::test]
    async fn list_runs_different_chapters() {
        let (pool, repo) = setup().await;
        let chapter_id = get_chapter_id(&pool).await;

        repo.create_run("b1", &chapter_id, None, "extract", "gpt-4", "v1", 1, "h1")
            .await
            .unwrap();

        // Insert second chapter
        let ch2_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 2, 'text', 'hash', datetime('now'))")
            .bind(&ch2_id)
            .execute(&pool)
            .await
            .unwrap();

        repo.create_run("b1", &ch2_id, None, "extract", "gpt-4", "v1", 1, "h2")
            .await
            .unwrap();

        let runs_ch1 = repo.list_runs_by_chapter("b1", &chapter_id).await.unwrap();
        assert_eq!(runs_ch1.len(), 1);

        let runs_ch2 = repo.list_runs_by_chapter("b1", &ch2_id).await.unwrap();
        assert_eq!(runs_ch2.len(), 1);
    }
}
