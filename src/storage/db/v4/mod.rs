pub mod ai_run_repo;
pub mod cache_repo;
pub mod chapter_repo;
pub mod claim_repo;
pub mod entity_repo;
pub mod progress_repo;
pub mod property_repo;

use sqlx::SqlitePool;

/// Initialize V4 tables by running the migration.
/// The migration is already run by init_pool, but this function
/// also seeds built-in property_dimensions.
pub async fn init_v4(pool: &SqlitePool) -> anyhow::Result<()> {
    seed_property_dimensions(pool).await?;
    Ok(())
}

/// Seed built-in character property dimensions.
async fn seed_property_dimensions(pool: &SqlitePool) -> anyhow::Result<()> {
    let dimensions = vec![
        ("character", "identity", "Identity", "text", 0.9, "replace"),
        (
            "character",
            "life_status",
            "Life Status",
            "text",
            1.0,
            "replace",
        ),
        (
            "character",
            "affiliation",
            "Affiliation",
            "text",
            0.7,
            "replace",
        ),
        ("character", "rank", "Rank", "text", 0.6, "replace"),
        (
            "character",
            "occupation",
            "Occupation",
            "text",
            0.5,
            "replace",
        ),
        (
            "character",
            "realm",
            "Realm/Cultivation",
            "text",
            0.8,
            "replace",
        ),
        ("character", "ability", "Ability", "text", 0.5, "append"),
        ("character", "equipment", "Equipment", "text", 0.4, "append"),
        (
            "character",
            "location",
            "Current Location",
            "text",
            0.6,
            "replace",
        ),
        (
            "character",
            "mental_state",
            "Mental State",
            "text",
            0.5,
            "replace",
        ),
        ("character", "goal", "Goal", "text", 0.4, "replace"),
        ("character", "injury", "Injury", "text", 0.5, "replace"),
        (
            "character",
            "appearance",
            "Appearance",
            "text",
            0.3,
            "append",
        ),
        (
            "character",
            "background",
            "Background",
            "text",
            0.3,
            "append",
        ),
    ];

    for (entity_type, key, display, value_type, importance, strategy) in dimensions {
        sqlx::query(
            "INSERT OR IGNORE INTO property_dimensions (id, book_id, entity_type, dimension_key, display_name, value_type, importance, merge_strategy, created_by, status, first_seen_chapter)
             VALUES (lower(hex(randomblob(16))), '__global__', ?, ?, ?, ?, ?, ?, 'system', 'active', 0)"
        )
        .bind(entity_type)
        .bind(key)
        .bind(display)
        .bind(value_type)
        .bind(importance)
        .bind(strategy)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Reset V4 memory-derived tables in FK-safe order.
/// Preserves: chapters, chapter_segments, source_spans, ai_runs,
/// reading_progress, property_dimensions, book_memory_v4_settings.
pub async fn reset_v4(pool: &SqlitePool, book_id: &str) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;

    // FK-safe delete order (leaves first)
    sqlx::query("DELETE FROM view_model_cache WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM chapter_summaries WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM claim_source_spans WHERE claim_id IN (SELECT id FROM claims WHERE book_id = ?)")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM entity_current_properties WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM entity_properties WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM entity_aliases WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM claims WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM entities WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM chapter_processing_runs WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    // Reset processing_progress to idle
    sqlx::query("UPDATE processing_progress SET status = 'idle', target_chapter = NULL, current_chapter = NULL, current_segment_id = NULL, last_error = NULL, updated_at = datetime('now') WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup_test_db() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("reader-v4-schema-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    #[tokio::test]
    async fn v4_tables_exist_after_migration() {
        let pool = setup_test_db().await;

        // Check all V4 Phase 1 tables exist
        let tables = vec![
            "chapters",
            "chapter_segments",
            "source_spans",
            "ai_runs",
            "chapter_processing_runs",
            "chapter_summaries",
            "claims",
            "claim_source_spans",
            "entities",
            "entity_aliases",
            "property_dimensions",
            "entity_properties",
            "entity_current_properties",
            "reading_progress",
            "processing_progress",
            "book_memory_v4_settings",
            "view_model_cache",
        ];

        for table in tables {
            let result: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(result.0, 1, "Table '{}' should exist", table);
        }
    }

    #[tokio::test]
    async fn v4_indexes_exist_after_migration() {
        let pool = setup_test_db().await;

        let indexes = vec![
            "idx_chapters_book_chapter",
            "idx_segments_chapter",
            "idx_spans_segment",
            "idx_spans_chapter_status",
            "idx_claims_book_chapter_status",
            "idx_claims_book_ai_run",
            "idx_aliases_book_alias",
            "idx_entities_book_type_name",
            "idx_current_properties_entity",
            "idx_properties_entity_dimension",
            "idx_settings_book",
        ];

        for index in indexes {
            let result: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name = ?",
            )
            .bind(index)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(result.0, 1, "Index '{}' should exist", index);
        }
    }

    #[tokio::test]
    async fn fk_constraints_enforced() {
        let pool = setup_test_db().await;

        // Inserting a claim with invalid source_span_id should fail
        let result = sqlx::query(
            "INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at)
             VALUES ('test-claim', 'test-book', 1, 'entity_introduction', 'test', 'nonexistent-span', 'nonexistent-run', 0.9, 'low', 'proposed', datetime('now'), datetime('now'))"
        )
        .execute(&pool)
        .await;

        assert!(
            result.is_err(),
            "FK constraint should reject invalid source_span_id"
        );
    }

    #[tokio::test]
    async fn property_dimensions_seeded() {
        let pool = setup_test_db().await;
        init_v4(&pool).await.unwrap();

        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM property_dimensions WHERE book_id = '__global__'")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(count.0, 14, "Should have 14 built-in character dimensions");
    }

    #[tokio::test]
    async fn property_dimensions_idempotent() {
        let pool = setup_test_db().await;
        init_v4(&pool).await.unwrap();
        init_v4(&pool).await.unwrap(); // second call should not fail

        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM property_dimensions WHERE book_id = '__global__'")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(
            count.0, 14,
            "Idempotent: still 14 dimensions after double init"
        );
    }

    #[tokio::test]
    async fn reset_v4_clears_memory_tables() {
        let pool = setup_test_db().await;
        init_v4(&pool).await.unwrap();

        // Insert test data
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e1', 'b1', 'character', 'Test', 'Test', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_summaries (id, book_id, chapter_index, summary, updated_at) VALUES ('s1', 'b1', 1, 'test', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO processing_progress (book_id, max_processed_chapter, status, updated_at) VALUES ('b1', 5, 'running', datetime('now'))")
            .execute(&pool).await.unwrap();

        reset_v4(&pool, "b1").await.unwrap();

        // Verify cleared
        let entity_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM entities WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(entity_count.0, 0, "entities should be cleared");

        let summary_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM chapter_summaries WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(summary_count.0, 0, "chapter_summaries should be cleared");

        // processing_progress should be reset to idle, not deleted
        let progress: (String,) =
            sqlx::query_as("SELECT status FROM processing_progress WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            progress.0, "idle",
            "processing_progress should be reset to idle"
        );

        // property_dimensions should be preserved
        let dim_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM property_dimensions WHERE book_id = '__global__'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(dim_count.0, 14, "property_dimensions should be preserved");
    }
}
