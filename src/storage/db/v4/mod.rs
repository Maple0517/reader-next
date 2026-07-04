pub mod ai_run_repo;
pub mod cache_repo;
pub mod chapter_repo;
pub mod claim_repo;
pub mod entity_repo;
pub mod identity_repo;
pub mod knowledge_repo;
pub mod place_repo;
pub mod progress_repo;
pub mod property_repo;
pub mod relationship_repo;

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
    sqlx::query("DELETE FROM map_layout_snapshots WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM place_edge_conflicts WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM place_edge_sources WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM place_edges WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM place_details WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM entity_links WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM knowledge_assertion_entities WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM knowledge_assertion_links WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM knowledge_assertions WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM knowledge_cards WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM entity_merge_conflicts WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM entity_merge_operations WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM entity_identity_links WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    // FK-safe: relationship_events references claims + relationships
    sqlx::query("DELETE FROM relationship_events WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    // FK-safe: relationships references entities
    sqlx::query("DELETE FROM relationships WHERE book_id = ?")
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
    // Reset processing_progress to idle with max_processed_chapter = 0
    sqlx::query("UPDATE processing_progress SET status = 'idle', max_processed_chapter = 0, target_chapter = NULL, current_chapter = NULL, current_segment_id = NULL, last_error = NULL, updated_at = datetime('now') WHERE book_id = ?")
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

    async fn assert_table_exists(pool: &SqlitePool, table: &str) {
        let result: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?")
                .bind(table)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(result.0, 1, "Table '{}' should exist", table);
    }

    async fn insert_phase5_schema_base(pool: &SqlitePool) {
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg1', 'b1', 'ch1', 'hash', 0, datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('ss1', 'b1', 'ch1', 'hash', 'seg1', 0, 0, 10, 'text', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'ch1', 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c1', 'b1', 1, 'location_edge', 'spatial', 'ss1', 'run1', 0.9, 'high', 'proposed', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c2', 'b1', 1, 'location_edge', 'spatial', 'ss1', 'run1', 0.8, 'high', 'proposed', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('p1', 'b1', 'place', '青云城', '青云城', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('p2', 'b1', 'place', '落霞山', '落霞山', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('org1', 'b1', 'organization', '青云门', '青云门', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
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

    #[tokio::test]
    async fn v4_relationship_tables_exist() {
        let pool = setup_test_db().await;

        for table in &["relationships", "relationship_events"] {
            let result: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            )
            .bind(*table)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(result.0, 1, "Table '{}' should exist", table);
        }
    }

    #[tokio::test]
    async fn v4_identity_tables_exist() {
        let pool = setup_test_db().await;

        for table in &[
            "entity_identity_links",
            "entity_merge_operations",
            "entity_merge_conflicts",
        ] {
            let result: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            )
            .bind(*table)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(result.0, 1, "Table '{}' should exist", table);
        }
    }

    #[tokio::test]
    async fn v4_identity_indexes_exist() {
        let pool = setup_test_db().await;

        let indexes = vec![
            "idx_identity_links_book_pair",
            "idx_identity_links_source_claim",
            "idx_identity_links_status",
            "idx_merge_ops_book_survivor",
            "idx_merge_ops_book_victim",
            "idx_merge_conflicts_op",
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
    async fn v4_relationship_indexes_exist() {
        let pool = setup_test_db().await;

        let indexes = vec![
            "idx_relationships_subject",
            "idx_relationships_object",
            "idx_relationships_group",
            "idx_relationship_events_rel",
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
    async fn v4_knowledge_tables_exist() {
        let pool = setup_test_db().await;

        for table in &[
            "knowledge_cards",
            "knowledge_assertions",
            "knowledge_assertion_links",
            "knowledge_assertion_entities",
        ] {
            let result: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            )
            .bind(*table)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(result.0, 1, "Table '{}' should exist", table);
        }
    }

    #[tokio::test]
    async fn v4_knowledge_indexes_exist() {
        let pool = setup_test_db().await;

        let indexes = vec![
            "idx_knowledge_cards_book_category_status",
            "idx_knowledge_cards_book_topic",
            "idx_knowledge_assertions_card_status",
            "idx_knowledge_assertions_source_claim",
            "idx_knowledge_assertions_book_chapter",
            "idx_knowledge_assertion_links_from",
            "idx_knowledge_assertion_links_to",
            "idx_knowledge_assertion_entities_assertion",
            "idx_knowledge_assertion_entities_entity",
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
    async fn v4_place_map_tables_exist() {
        let pool = setup_test_db().await;

        for table in &[
            "place_details",
            "place_edges",
            "place_edge_sources",
            "map_layout_snapshots",
            "place_edge_conflicts",
            "entity_links",
        ] {
            assert_table_exists(&pool, table).await;
        }
    }

    #[tokio::test]
    async fn v4_place_map_indexes_exist() {
        let pool = setup_test_db().await;

        let indexes = vec![
            "idx_place_details_book_status",
            "idx_place_details_book_type",
            "idx_place_details_parent",
            "idx_place_edges_book_status",
            "idx_place_edges_from",
            "idx_place_edges_to",
            "idx_place_edges_source_claim",
            "idx_place_edge_sources_edge",
            "idx_map_layout_snapshots_book_chapter",
            "idx_map_layout_snapshots_edge_hash",
            "idx_place_edge_conflicts_book_status",
            "idx_place_edge_conflicts_claim",
            "idx_entity_links_book_pair",
            "idx_entity_links_source_claim",
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
    async fn place_details_schema() {
        let pool = setup_test_db().await;
        assert_table_exists(&pool, "place_details").await;
        insert_phase5_schema_base(&pool).await;

        sqlx::query(
            "INSERT INTO place_details (entity_id, book_id, place_type, parent_place_id, first_seen_chapter, last_seen_chapter, created_at, updated_at)
             VALUES ('p1', 'b1', 'city', 'p2', 1, 1, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        let self_parent = sqlx::query(
            "INSERT INTO place_details (entity_id, book_id, place_type, parent_place_id, first_seen_chapter, last_seen_chapter, created_at, updated_at)
             VALUES ('p2', 'b1', 'mountain', 'p2', 1, 1, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await;
        assert!(
            self_parent.is_err(),
            "place_details must reject self parent"
        );
    }

    #[tokio::test]
    async fn place_edges_schema() {
        let pool = setup_test_db().await;
        assert_table_exists(&pool, "place_edges").await;
        insert_phase5_schema_base(&pool).await;

        sqlx::query(
            "INSERT INTO place_edges (id, book_id, from_place_id, to_place_id, edge_type, confidence, source_claim_id, first_seen_chapter, last_seen_chapter, created_at, updated_at)
             VALUES ('edge1', 'b1', 'p1', 'p2', 'near', 0.8, 'c1', 1, 1, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        let self_edge = sqlx::query(
            "INSERT INTO place_edges (id, book_id, from_place_id, to_place_id, edge_type, confidence, source_claim_id, first_seen_chapter, last_seen_chapter, created_at, updated_at)
             VALUES ('edge2', 'b1', 'p1', 'p1', 'near', 0.8, 'c1', 1, 1, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await;
        assert!(self_edge.is_err(), "place_edges must reject self edges");

        let invalid_status = sqlx::query(
            "INSERT INTO place_edges (id, book_id, from_place_id, to_place_id, edge_type, confidence, source_claim_id, first_seen_chapter, last_seen_chapter, status, created_at, updated_at)
             VALUES ('edge3', 'b1', 'p1', 'p2', 'near', 0.8, 'c1', 1, 1, 'uncertain', datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await;
        assert!(
            invalid_status.is_err(),
            "place_edges stores only accepted canonical statuses"
        );
    }

    #[tokio::test]
    async fn map_layout_snapshots_schema() {
        let pool = setup_test_db().await;
        assert_table_exists(&pool, "map_layout_snapshots").await;

        sqlx::query(
            "INSERT INTO map_layout_snapshots (id, book_id, max_chapter, layout_version, layout_json, source_edge_hash, created_at)
             VALUES ('layout1', 'b1', 1, 'v1', '{}', 'hash1', datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn place_edge_sources_schema() {
        let pool = setup_test_db().await;
        assert_table_exists(&pool, "place_edge_sources").await;
        insert_phase5_schema_base(&pool).await;

        sqlx::query(
            "INSERT INTO place_edges (id, book_id, from_place_id, to_place_id, edge_type, confidence, source_claim_id, first_seen_chapter, last_seen_chapter, created_at, updated_at)
             VALUES ('edge1', 'b1', 'p1', 'p2', 'near', 0.8, 'c1', 1, 1, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO place_edge_sources (id, book_id, edge_id, source_claim_id, chapter_index, confidence, created_at)
             VALUES ('src1', 'b1', 'edge1', 'c1', 1, 0.8, datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        let duplicate_source = sqlx::query(
            "INSERT INTO place_edge_sources (id, book_id, edge_id, source_claim_id, chapter_index, confidence, created_at)
             VALUES ('src2', 'b1', 'edge1', 'c1', 1, 0.8, datetime('now'))",
        )
        .execute(&pool)
        .await;
        assert!(
            duplicate_source.is_err(),
            "same edge/source claim should be unique"
        );
    }

    #[tokio::test]
    async fn entity_links_schema() {
        let pool = setup_test_db().await;
        assert_table_exists(&pool, "entity_links").await;
        insert_phase5_schema_base(&pool).await;

        sqlx::query(
            "INSERT INTO entity_links (id, book_id, entity_a_id, entity_b_id, link_type, source_claim_id, confidence, created_at, updated_at)
             VALUES ('el1', 'b1', 'org1', 'p1', 'headquarters_of', 'c1', 0.8, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        let self_link = sqlx::query(
            "INSERT INTO entity_links (id, book_id, entity_a_id, entity_b_id, link_type, source_claim_id, confidence, created_at, updated_at)
             VALUES ('el2', 'b1', 'p1', 'p1', 'related_entity', 'c1', 0.8, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await;
        assert!(self_link.is_err(), "entity_links must reject self links");

        let invalid_type = sqlx::query(
            "INSERT INTO entity_links (id, book_id, entity_a_id, entity_b_id, link_type, source_claim_id, confidence, created_at, updated_at)
             VALUES ('el3', 'b1', 'org1', 'p1', 'redirect', 'c1', 0.8, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await;
        assert!(
            invalid_type.is_err(),
            "generic entity_links must not accept identity redirect"
        );
    }

    #[tokio::test]
    async fn reset_v4_cleans_map_tables() {
        let pool = setup_test_db().await;
        init_v4(&pool).await.unwrap();
        insert_phase5_schema_base(&pool).await;

        sqlx::query("INSERT INTO place_details (entity_id, book_id, place_type, first_seen_chapter, last_seen_chapter, created_at, updated_at) VALUES ('p1', 'b1', 'city', 1, 1, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO place_details (entity_id, book_id, place_type, first_seen_chapter, last_seen_chapter, created_at, updated_at) VALUES ('p2', 'b1', 'mountain', 1, 1, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO place_edges (id, book_id, from_place_id, to_place_id, edge_type, confidence, source_claim_id, first_seen_chapter, last_seen_chapter, created_at, updated_at) VALUES ('edge1', 'b1', 'p1', 'p2', 'near', 0.8, 'c1', 1, 1, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO place_edge_sources (id, book_id, edge_id, source_claim_id, chapter_index, confidence, created_at) VALUES ('src1', 'b1', 'edge1', 'c1', 1, 0.8, datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO place_edge_conflicts (id, book_id, new_edge_claim_id, existing_edge_id, conflict_type, reason_code, status, created_at) VALUES ('conflict1', 'b1', 'c2', 'edge1', 'opposite_direction', 'test', 'open', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO map_layout_snapshots (id, book_id, max_chapter, layout_version, layout_json, source_edge_hash, created_at) VALUES ('layout1', 'b1', 1, 'v1', '{}', 'hash1', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO entity_links (id, book_id, entity_a_id, entity_b_id, link_type, source_claim_id, confidence, created_at, updated_at) VALUES ('el1', 'b1', 'org1', 'p1', 'headquarters_of', 'c1', 0.8, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();

        reset_v4(&pool, "b1").await.unwrap();

        for table in &[
            "map_layout_snapshots",
            "place_edge_conflicts",
            "place_edge_sources",
            "place_edges",
            "place_details",
            "entity_links",
        ] {
            let count: (i64,) = sqlx::query_as(&format!(
                "SELECT COUNT(*) FROM {} WHERE book_id = 'b1'",
                table
            ))
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(count.0, 0, "{} should be cleared", table);
        }
    }

    #[tokio::test]
    async fn knowledge_unique_topic_per_category() {
        let pool = setup_test_db().await;

        sqlx::query("INSERT INTO knowledge_cards (id, book_id, category, topic_key, topic_display, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at) VALUES ('k1', 'b1', 'power_system', 'cultivation-realms', 'Cultivation Realms', 0.8, 0.9, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();

        let duplicate_same_category = sqlx::query("INSERT INTO knowledge_cards (id, book_id, category, topic_key, topic_display, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at) VALUES ('k2', 'b1', 'power_system', 'cultivation-realms', 'Cultivation Levels', 0.7, 0.8, 2, 2, 'active', datetime('now'), datetime('now'))")
            .execute(&pool)
            .await;
        assert!(
            duplicate_same_category.is_err(),
            "UNIQUE(book_id, category, topic_key) should reject duplicates"
        );

        sqlx::query("INSERT INTO knowledge_cards (id, book_id, category, topic_key, topic_display, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at) VALUES ('k3', 'b1', 'history', 'cultivation-realms', 'Cultivation Realms History', 0.7, 0.8, 2, 2, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn knowledge_fk_and_link_constraints() {
        let pool = setup_test_db().await;

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg1', 'b1', 'ch1', 'hash', 0, datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('ss1', 'b1', 'ch1', 'hash', 'seg1', 0, 0, 10, 'text', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'ch1', 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c1', 'b1', 1, 'knowledge_assertion', 'test', 'ss1', 'run1', 0.9, 'high', 'proposed', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e1', 'b1', 'concept', 'Magic', 'Magic', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO knowledge_cards (id, book_id, category, topic_key, topic_display, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at) VALUES ('k1', 'b1', 'world_rule', 'magic-rules', 'Magic Rules', 0.8, 0.9, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO knowledge_assertions (id, book_id, card_id, source_claim_id, assertion_text, status, confidence, importance_score, chapter_index, created_at, updated_at) VALUES ('a1', 'b1', 'k1', 'c1', 'Magic has rules.', 'active', 0.9, 0.8, 1, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO knowledge_assertions (id, book_id, card_id, source_claim_id, assertion_text, status, confidence, importance_score, chapter_index, created_at, updated_at) VALUES ('a2', 'b1', 'k1', 'c1', 'Magic rules are incomplete.', 'active', 0.8, 0.7, 1, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();

        let invalid_assertion = sqlx::query("INSERT INTO knowledge_assertions (id, book_id, card_id, source_claim_id, assertion_text, status, confidence, importance_score, chapter_index, created_at, updated_at) VALUES ('bad-a', 'b1', 'missing-card', 'c1', 'Bad.', 'active', 0.9, 0.8, 1, datetime('now'), datetime('now'))")
            .execute(&pool)
            .await;
        assert!(
            invalid_assertion.is_err(),
            "assertion FK should reject missing card"
        );

        sqlx::query("INSERT INTO knowledge_assertion_links (id, book_id, from_assertion_id, to_assertion_id, link_type, created_at) VALUES ('l1', 'b1', 'a2', 'a1', 'clarifies', datetime('now'))")
            .execute(&pool).await.unwrap();
        let duplicate_link = sqlx::query("INSERT INTO knowledge_assertion_links (id, book_id, from_assertion_id, to_assertion_id, link_type, created_at) VALUES ('l2', 'b1', 'a2', 'a1', 'clarifies', datetime('now'))")
            .execute(&pool)
            .await;
        assert!(
            duplicate_link.is_err(),
            "UNIQUE(book_id, from_assertion_id, to_assertion_id, link_type) should reject duplicate links"
        );

        let self_link = sqlx::query("INSERT INTO knowledge_assertion_links (id, book_id, from_assertion_id, to_assertion_id, link_type, created_at) VALUES ('l3', 'b1', 'a1', 'a1', 'supports', datetime('now'))")
            .execute(&pool)
            .await;
        assert!(self_link.is_err(), "link CHECK should reject self-links");

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch2', 'b2', 1, 'text', 'hash2', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg2', 'b2', 'ch2', 'hash2', 0, datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('ss2', 'b2', 'ch2', 'hash2', 'seg2', 0, 0, 10, 'text', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run2', 'b2', 'ch2', 'extract', 'test', 'v1', 1, 'hash2', 'completed', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c2', 'b2', 1, 'knowledge_assertion', 'test', 'ss2', 'run2', 0.9, 'high', 'proposed', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO knowledge_cards (id, book_id, category, topic_key, topic_display, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at) VALUES ('k2', 'b2', 'world_rule', 'foreign-magic-rules', 'Foreign Magic Rules', 0.8, 0.9, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO knowledge_assertions (id, book_id, card_id, source_claim_id, assertion_text, status, confidence, importance_score, chapter_index, created_at, updated_at) VALUES ('a3', 'b2', 'k2', 'c2', 'Foreign magic has rules.', 'active', 0.9, 0.8, 1, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        let cross_book_link = sqlx::query("INSERT INTO knowledge_assertion_links (id, book_id, from_assertion_id, to_assertion_id, link_type, created_at) VALUES ('l4', 'b1', 'a2', 'a3', 'clarifies', datetime('now'))")
            .execute(&pool)
            .await;
        assert!(
            cross_book_link.is_err(),
            "link composite FK should reject cross-book assertion links"
        );

        sqlx::query("INSERT INTO knowledge_assertion_entities (id, book_id, assertion_id, entity_id, role, created_at) VALUES ('ae1', 'b1', 'a1', 'e1', 'related', datetime('now'))")
            .execute(&pool).await.unwrap();
        let duplicate_entity_ref = sqlx::query("INSERT INTO knowledge_assertion_entities (id, book_id, assertion_id, entity_id, role, created_at) VALUES ('ae2', 'b1', 'a1', 'e1', 'related', datetime('now'))")
            .execute(&pool)
            .await;
        assert!(
            duplicate_entity_ref.is_err(),
            "UNIQUE(assertion_id, entity_id, role) should reject duplicate references"
        );
    }

    #[tokio::test]
    async fn reset_v4_clears_knowledge_tables() {
        let pool = setup_test_db().await;
        init_v4(&pool).await.unwrap();

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg1', 'b1', 'ch1', 'hash', 0, datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('ss1', 'b1', 'ch1', 'hash', 'seg1', 0, 0, 10, 'text', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'ch1', 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c1', 'b1', 1, 'knowledge_assertion', 'test', 'ss1', 'run1', 0.9, 'high', 'accepted', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e1', 'b1', 'concept', 'Magic', 'Magic', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO knowledge_cards (id, book_id, category, topic_key, topic_display, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at) VALUES ('k1', 'b1', 'world_rule', 'magic-rules', 'Magic Rules', 0.8, 0.9, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO knowledge_assertions (id, book_id, card_id, source_claim_id, assertion_text, status, confidence, importance_score, chapter_index, created_at, updated_at) VALUES ('a1', 'b1', 'k1', 'c1', 'Magic has rules.', 'active', 0.9, 0.8, 1, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO knowledge_assertions (id, book_id, card_id, source_claim_id, assertion_text, status, confidence, importance_score, chapter_index, created_at, updated_at) VALUES ('a2', 'b1', 'k1', 'c1', 'Magic has hidden rules.', 'active', 0.8, 0.7, 1, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO knowledge_assertion_links (id, book_id, from_assertion_id, to_assertion_id, link_type, created_at) VALUES ('l1', 'b1', 'a2', 'a1', 'clarifies', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO knowledge_assertion_entities (id, book_id, assertion_id, entity_id, role, created_at) VALUES ('ae1', 'b1', 'a1', 'e1', 'related', datetime('now'))")
            .execute(&pool).await.unwrap();

        reset_v4(&pool, "b1").await.unwrap();

        for table in &[
            "knowledge_assertion_entities",
            "knowledge_assertion_links",
            "knowledge_assertions",
            "knowledge_cards",
        ] {
            let count: (i64,) = sqlx::query_as(&format!(
                "SELECT COUNT(*) FROM {} WHERE book_id = 'b1'",
                table
            ))
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(count.0, 0, "{} should be cleared", table);
        }

        let claim_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM claims WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(claim_count.0, 0, "claims should still be cleared");
    }

    #[tokio::test]
    async fn relationship_check_constraint() {
        let pool = setup_test_db().await;

        // Insert a valid entity first
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e1', 'b1', 'character', 'Test', 'Test', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();

        // Inserting a relationship where subject == object should fail
        let result = sqlx::query(
            "INSERT INTO relationships (id, book_id, subject_character_id, object_character_id, relation_group, relation_label, first_seen_chapter, last_changed_chapter, last_seen_chapter, created_at, updated_at)
             VALUES ('r1', 'b1', 'e1', 'e1', 'ally', 'test', 1, 1, 1, datetime('now'), datetime('now'))"
        )
        .execute(&pool)
        .await;

        assert!(
            result.is_err(),
            "CHECK constraint should reject subject_character_id == object_character_id"
        );
    }

    #[tokio::test]
    async fn relationship_fk_constraint() {
        let pool = setup_test_db().await;

        // Inserting a relationship with nonexistent entity_id should fail
        let result = sqlx::query(
            "INSERT INTO relationships (id, book_id, subject_character_id, object_character_id, relation_group, relation_label, first_seen_chapter, last_changed_chapter, last_seen_chapter, created_at, updated_at)
             VALUES ('r1', 'b1', 'nonexistent', 'also-nonexistent', 'ally', 'test', 1, 1, 1, datetime('now'), datetime('now'))"
        )
        .execute(&pool)
        .await;

        assert!(
            result.is_err(),
            "FK constraint should reject nonexistent entity IDs"
        );
    }

    #[tokio::test]
    async fn relationship_event_unique_source_claim() {
        let pool = setup_test_db().await;

        // Setup: chapter + source_span + ai_run + claim + entities + relationship
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(&pool).await.unwrap();
        // Need a segment for the FK on source_spans
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg1', 'b1', 'ch1', 'hash', 0, datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('ss1', 'b1', 'ch1', 'hash', 'seg1', 0, 0, 10, 'text', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'ch1', 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c1', 'b1', 1, 'relationship_update', 'test', 'ss1', 'run1', 0.9, 'low', 'proposed', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();

        // Two entities + relationship
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e1', 'b1', 'character', 'A', 'A', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e2', 'b1', 'character', 'B', 'B', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO relationships (id, book_id, subject_character_id, object_character_id, relation_group, relation_label, first_seen_chapter, last_changed_chapter, last_seen_chapter, created_at, updated_at) VALUES ('r1', 'b1', 'e1', 'e2', 'ally', 'allies', 1, 1, 1, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();

        // First event should succeed
        sqlx::query("INSERT INTO relationship_events (id, book_id, relationship_id, event_type, relation_group, relation_label, chapter_index, source_claim_id, confidence, created_at) VALUES ('ev1', 'b1', 'r1', 'established', 'ally', 'allies', 1, 'c1', 0.9, datetime('now'))")
            .execute(&pool).await.unwrap();

        // Second event with same source_claim_id should fail (UNIQUE)
        let result = sqlx::query("INSERT INTO relationship_events (id, book_id, relationship_id, event_type, relation_group, relation_label, chapter_index, source_claim_id, confidence, created_at) VALUES ('ev2', 'b1', 'r1', 'updated', 'ally', 'allies', 1, 'c1', 0.8, datetime('now'))")
            .execute(&pool)
            .await;

        assert!(
            result.is_err(),
            "UNIQUE constraint should reject duplicate source_claim_id"
        );
    }

    #[tokio::test]
    async fn reset_v4_clears_relationships() {
        let pool = setup_test_db().await;
        init_v4(&pool).await.unwrap();

        // Setup prerequisite data
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg1', 'b1', 'ch1', 'hash', 0, datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('ss1', 'b1', 'ch1', 'hash', 'seg1', 0, 0, 10, 'text', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'ch1', 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c1', 'b1', 1, 'relationship_update', 'test', 'ss1', 'run1', 0.9, 'low', 'proposed', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();

        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e1', 'b1', 'character', 'A', 'A', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e2', 'b1', 'character', 'B', 'B', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO relationships (id, book_id, subject_character_id, object_character_id, relation_group, relation_label, first_seen_chapter, last_changed_chapter, last_seen_chapter, created_at, updated_at) VALUES ('r1', 'b1', 'e1', 'e2', 'ally', 'allies', 1, 1, 1, datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO relationship_events (id, book_id, relationship_id, event_type, relation_group, relation_label, chapter_index, source_claim_id, confidence, created_at) VALUES ('ev1', 'b1', 'r1', 'established', 'ally', 'allies', 1, 'c1', 0.9, datetime('now'))")
            .execute(&pool).await.unwrap();

        reset_v4(&pool, "b1").await.unwrap();

        // Verify relationships cleared
        let rel_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM relationships WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(rel_count.0, 0, "relationships should be cleared");

        // Verify relationship_events cleared
        let ev_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM relationship_events WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(ev_count.0, 0, "relationship_events should be cleared");
    }

    #[tokio::test]
    async fn reset_v4_clears_identity_tables() {
        let pool = setup_test_db().await;
        init_v4(&pool).await.unwrap();

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg1', 'b1', 'ch1', 'hash', 0, datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('ss1', 'b1', 'ch1', 'hash', 'seg1', 0, 0, 10, 'text', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'ch1', 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c1', 'b1', 1, 'identity_reveal', 'same_identity', 'ss1', 'run1', 0.9, 'high', 'proposed', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();

        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e1', 'b1', 'character', 'A', 'A', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e2', 'b1', 'character', 'B', 'B', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();

        sqlx::query("INSERT INTO entity_identity_links (id, book_id, entity_a_id, entity_b_id, link_type, confidence, source_claim_id, status, created_at, updated_at) VALUES ('l1', 'b1', 'e1', 'e2', 'same_identity', 0.9, 'c1', 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO entity_merge_operations (id, book_id, survivor_entity_id, victim_entity_id, source_identity_link_id, reason_code, confidence, status, created_at) VALUES ('m1', 'b1', 'e1', 'e2', 'l1', 'identity_reveal', 0.9, 'pending', datetime('now'))")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO entity_merge_conflicts (id, book_id, merge_operation_id, survivor_entity_id, victim_entity_id, conflict_type, dimension_key, survivor_value, victim_value, resolution, created_at) VALUES ('mc1', 'b1', 'm1', 'e1', 'e2', 'property_conflict', 'identity', 'A', 'B', 'keep_survivor', datetime('now'))")
            .execute(&pool).await.unwrap();

        reset_v4(&pool, "b1").await.unwrap();

        let link_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM entity_identity_links WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(link_count.0, 0, "entity_identity_links should be cleared");

        let op_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM entity_merge_operations WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(op_count.0, 0, "entity_merge_operations should be cleared");

        let conflict_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM entity_merge_conflicts WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            conflict_count.0, 0,
            "entity_merge_conflicts should be cleared"
        );
    }
}
