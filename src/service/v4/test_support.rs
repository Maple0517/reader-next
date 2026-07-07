use sqlx::SqlitePool;

pub struct V4ProcessorTestContext {
    pub pool: SqlitePool,
    pub chapter_id: String,
    pub segment_id: String,
    pub span_id: String,
    pub run_id: String,
}

pub async fn setup_v4_processor_test(
    name: &str,
    chapter_index: i64,
    raw_text: &str,
    text_excerpt: &str,
) -> V4ProcessorTestContext {
    let dir = std::env::temp_dir().join(format!("reader-v4-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
    let pool = crate::storage::db::init_pool(&database_url).await.unwrap();
    crate::storage::db::v4::init_v4(&pool).await.unwrap();

    let chapter_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', ?, ?, 'hash', datetime('now'))",
    )
    .bind(&chapter_id)
    .bind(chapter_index)
    .bind(raw_text)
    .execute(&pool)
    .await
    .unwrap();

    let segment_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'hash', 0, datetime('now'))",
    )
    .bind(&segment_id)
    .bind(&chapter_id)
    .execute(&pool)
    .await
    .unwrap();

    let span_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 0, 0, 10, ?, datetime('now'))",
    )
    .bind(&span_id)
    .bind(&chapter_id)
    .bind(&segment_id)
    .bind(text_excerpt)
    .execute(&pool)
    .await
    .unwrap();

    let run_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, 'b1', ?, 'extract', 'gpt-4', 'v1', 1, 'hash', 'running', datetime('now'))",
    )
    .bind(&run_id)
    .bind(&chapter_id)
    .execute(&pool)
    .await
    .unwrap();

    V4ProcessorTestContext {
        pool,
        chapter_id,
        segment_id,
        span_id,
        run_id,
    }
}
