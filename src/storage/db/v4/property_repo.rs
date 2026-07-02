use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone)]
pub struct DimensionRecord {
    pub id: String,
    pub book_id: String,
    pub entity_type: String,
    pub dimension_key: String,
    pub display_name: String,
    pub value_type: String,
    pub importance: f64,
    pub merge_strategy: String,
}

#[derive(Debug, Clone)]
pub struct PropertyRecord {
    pub id: String,
    pub book_id: String,
    pub entity_id: String,
    pub dimension_key: String,
    pub value_text: Option<String>,
    pub value_json: Option<String>,
    pub valid_from_chapter: i64,
    pub valid_to_chapter: Option<i64>,
    pub source_claim_id: String,
    pub confidence: f64,
    pub status: String,
    pub supersedes_property_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CurrentPropertyRecord {
    pub book_id: String,
    pub entity_id: String,
    pub dimension_key: String,
    pub property_id: String,
    pub value_text: Option<String>,
    pub value_json: Option<String>,
    pub updated_chapter: i64,
    pub confidence: f64,
}

pub struct PropertyRepo {
    pool: SqlitePool,
}

impl PropertyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Dimension operations ---

    pub async fn get_dimension(
        &self,
        book_id: &str,
        entity_type: &str,
        dimension_key: &str,
    ) -> anyhow::Result<Option<DimensionRecord>> {
        let row = sqlx::query_as::<_, DimensionRow>(
            "SELECT id, book_id, entity_type, dimension_key, display_name, value_type, importance, merge_strategy
             FROM property_dimensions WHERE book_id = ? AND entity_type = ? AND dimension_key = ?"
        )
        .bind(book_id)
        .bind(entity_type)
        .bind(dimension_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn list_dimensions(
        &self,
        book_id: &str,
        entity_type: &str,
    ) -> anyhow::Result<Vec<DimensionRecord>> {
        let rows = sqlx::query_as::<_, DimensionRow>(
            "SELECT id, book_id, entity_type, dimension_key, display_name, value_type, importance, merge_strategy
             FROM property_dimensions WHERE (book_id = ? OR book_id = '__global__') AND entity_type = ? AND status = 'active' ORDER BY importance DESC"
        )
        .bind(book_id)
        .bind(entity_type)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    // --- Property operations ---

    pub async fn insert_property(
        &self,
        conn: &mut SqliteConnection,
        book_id: &str,
        entity_id: &str,
        dimension_key: &str,
        value_text: Option<&str>,
        value_json: Option<&str>,
        valid_from_chapter: i64,
        source_claim_id: &str,
        confidence: f64,
        supersedes_property_id: Option<&str>,
    ) -> anyhow::Result<PropertyRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO entity_properties (id, book_id, entity_id, dimension_key, value_text, value_json, valid_from_chapter, source_claim_id, confidence, status, supersedes_property_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(entity_id)
        .bind(dimension_key)
        .bind(value_text)
        .bind(value_json)
        .bind(valid_from_chapter)
        .bind(source_claim_id)
        .bind(confidence)
        .bind(supersedes_property_id)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        Ok(PropertyRecord {
            id,
            book_id: book_id.to_string(),
            entity_id: entity_id.to_string(),
            dimension_key: dimension_key.to_string(),
            value_text: value_text.map(|s| s.to_string()),
            value_json: value_json.map(|s| s.to_string()),
            valid_from_chapter,
            valid_to_chapter: None,
            source_claim_id: source_claim_id.to_string(),
            confidence,
            status: "active".to_string(),
            supersedes_property_id: supersedes_property_id.map(|s| s.to_string()),
        })
    }

    pub async fn supersede_property(
        &self,
        conn: &mut SqliteConnection,
        property_id: &str,
        valid_to_chapter: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE entity_properties SET status = 'superseded', valid_to_chapter = ? WHERE id = ?",
        )
        .bind(valid_to_chapter)
        .bind(property_id)
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    pub async fn get_active_properties(
        &self,
        conn: &mut SqliteConnection,
        entity_id: &str,
        dimension_key: &str,
    ) -> anyhow::Result<Vec<PropertyRecord>> {
        let rows = sqlx::query_as::<_, PropertyRow>(
            "SELECT id, book_id, entity_id, dimension_key, value_text, value_json, valid_from_chapter, valid_to_chapter, source_claim_id, confidence, status, supersedes_property_id
             FROM entity_properties WHERE entity_id = ? AND dimension_key = ? AND status = 'active' ORDER BY valid_from_chapter DESC"
        )
        .bind(entity_id)
        .bind(dimension_key)
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    // --- Current property operations ---

    pub async fn upsert_current_property(
        &self,
        conn: &mut SqliteConnection,
        book_id: &str,
        entity_id: &str,
        dimension_key: &str,
        property_id: &str,
        value_text: Option<&str>,
        value_json: Option<&str>,
        updated_chapter: i64,
        confidence: f64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO entity_current_properties (book_id, entity_id, dimension_key, property_id, value_text, value_json, updated_chapter, confidence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(book_id, entity_id, dimension_key) DO UPDATE SET
               property_id = excluded.property_id,
               value_text = excluded.value_text,
               value_json = excluded.value_json,
               updated_chapter = excluded.updated_chapter,
               confidence = excluded.confidence"
        )
        .bind(book_id)
        .bind(entity_id)
        .bind(dimension_key)
        .bind(property_id)
        .bind(value_text)
        .bind(value_json)
        .bind(updated_chapter)
        .bind(confidence)
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    pub async fn get_current_property(
        &self,
        book_id: &str,
        entity_id: &str,
        dimension_key: &str,
    ) -> anyhow::Result<Option<CurrentPropertyRecord>> {
        let row = sqlx::query_as::<_, CurrentPropertyRow>(
            "SELECT book_id, entity_id, dimension_key, property_id, value_text, value_json, updated_chapter, confidence
             FROM entity_current_properties WHERE book_id = ? AND entity_id = ? AND dimension_key = ?"
        )
        .bind(book_id)
        .bind(entity_id)
        .bind(dimension_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn list_current_properties(
        &self,
        book_id: &str,
        entity_id: &str,
    ) -> anyhow::Result<Vec<CurrentPropertyRecord>> {
        let rows = sqlx::query_as::<_, CurrentPropertyRow>(
            "SELECT book_id, entity_id, dimension_key, property_id, value_text, value_json, updated_chapter, confidence
             FROM entity_current_properties WHERE book_id = ? AND entity_id = ?"
        )
        .bind(book_id)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    // --- High-level merge operations ---

    /// Apply replace strategy: supersede old property, insert new, update current.
    /// All operations are wrapped in a transaction for atomicity.
    pub async fn apply_replace(
        &self,
        book_id: &str,
        entity_id: &str,
        dimension_key: &str,
        value_text: Option<&str>,
        value_json: Option<&str>,
        valid_from_chapter: i64,
        source_claim_id: &str,
        confidence: f64,
    ) -> anyhow::Result<PropertyRecord> {
        let mut tx = self.pool.begin().await?;

        // Supersede existing active properties for this dimension
        let existing = self
            .get_active_properties(&mut *tx, entity_id, dimension_key)
            .await?;
        for prop in &existing {
            self.supersede_property(&mut *tx, &prop.id, valid_from_chapter - 1)
                .await?;
        }

        // Insert new property
        let new_prop = self
            .insert_property(
                &mut *tx,
                book_id,
                entity_id,
                dimension_key,
                value_text,
                value_json,
                valid_from_chapter,
                source_claim_id,
                confidence,
                existing.first().map(|p| p.id.as_str()),
            )
            .await?;

        // Update current
        self.upsert_current_property(
            &mut *tx,
            book_id,
            entity_id,
            dimension_key,
            &new_prop.id,
            value_text,
            value_json,
            valid_from_chapter,
            confidence,
        )
        .await?;

        tx.commit().await?;
        Ok(new_prop)
    }

    /// Apply append strategy: keep old properties, insert new, aggregate into current.
    /// All operations are wrapped in a transaction for atomicity.
    pub async fn apply_append(
        &self,
        book_id: &str,
        entity_id: &str,
        dimension_key: &str,
        value_text: Option<&str>,
        value_json: Option<&str>,
        valid_from_chapter: i64,
        source_claim_id: &str,
        confidence: f64,
    ) -> anyhow::Result<PropertyRecord> {
        let mut tx = self.pool.begin().await?;

        // Insert new property (don't supersede old ones)
        let new_prop = self
            .insert_property(
                &mut *tx,
                book_id,
                entity_id,
                dimension_key,
                value_text,
                value_json,
                valid_from_chapter,
                source_claim_id,
                confidence,
                None,
            )
            .await?;

        // Aggregate all active properties for current (within same tx to see uncommitted insert)
        let all_active = self
            .get_active_properties(&mut *tx, entity_id, dimension_key)
            .await?;
        let aggregated_values: Vec<serde_json::Value> = all_active
            .iter()
            .filter_map(|p| {
                p.value_text
                    .as_ref()
                    .map(|t| serde_json::Value::String(t.clone()))
            })
            .collect();
        let aggregated_json = serde_json::to_string(&aggregated_values).ok();
        let summary = aggregated_values
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join("；");

        self.upsert_current_property(
            &mut *tx,
            book_id,
            entity_id,
            dimension_key,
            &new_prop.id,
            if summary.is_empty() {
                None
            } else {
                Some(&summary)
            },
            aggregated_json.as_deref(),
            valid_from_chapter,
            confidence,
        )
        .await?;

        tx.commit().await?;
        Ok(new_prop)
    }

    /// Validate dimension_key is registered. Returns the dimension if valid.
    /// Checks both book-specific and global ('__global__') dimensions.
    pub async fn validate_dimension(
        &self,
        book_id: &str,
        entity_type: &str,
        dimension_key: &str,
    ) -> anyhow::Result<Option<DimensionRecord>> {
        // Check book-specific dimension first
        if let Some(dim) = self
            .get_dimension(book_id, entity_type, dimension_key)
            .await?
        {
            return Ok(Some(dim));
        }
        // Fall back to global dimensions
        self.get_dimension("__global__", entity_type, dimension_key)
            .await
    }

    // Transaction-aware methods for use within reducer

    pub async fn validate_dimension_with_conn(
        conn: &mut sqlx::SqliteConnection,
        book_id: &str,
        entity_type: &str,
        dimension_key: &str,
    ) -> anyhow::Result<Option<DimensionRecord>> {
        // Check book-specific dimension first
        let row = sqlx::query_as::<_, DimensionRow>(
            "SELECT id, book_id, entity_type, dimension_key, display_name, value_type, importance, merge_strategy
             FROM property_dimensions WHERE book_id = ? AND entity_type = ? AND dimension_key = ? AND status = 'active'"
        )
        .bind(book_id)
        .bind(entity_type)
        .bind(dimension_key)
        .fetch_optional(&mut *conn)
        .await?;

        if row.is_some() {
            return Ok(row.map(|r| r.into()));
        }

        // Fall back to global dimensions
        let row = sqlx::query_as::<_, DimensionRow>(
            "SELECT id, book_id, entity_type, dimension_key, display_name, value_type, importance, merge_strategy
             FROM property_dimensions WHERE book_id = '__global__' AND entity_type = ? AND dimension_key = ? AND status = 'active'"
        )
        .bind(entity_type)
        .bind(dimension_key)
        .fetch_optional(conn)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn apply_replace_with_conn(
        conn: &mut sqlx::SqliteConnection,
        book_id: &str,
        entity_id: &str,
        dimension_key: &str,
        value_text: Option<&str>,
        value_json: Option<&str>,
        valid_from_chapter: i64,
        source_claim_id: &str,
        confidence: f64,
    ) -> anyhow::Result<()> {
        // Supersede existing active properties for this dimension
        let active = sqlx::query_as::<_, PropertyRow>(
            "SELECT id, book_id, entity_id, dimension_key, value_text, value_json, valid_from_chapter, valid_to_chapter, source_claim_id, confidence, status, supersedes_property_id
             FROM entity_properties WHERE entity_id = ? AND dimension_key = ? AND status = 'active'"
        )
        .bind(entity_id)
        .bind(dimension_key)
        .fetch_all(&mut *conn)
        .await?;

        for prop in &active {
            sqlx::query("UPDATE entity_properties SET status = 'superseded', valid_to_chapter = ? WHERE id = ?")
                .bind(valid_from_chapter - 1)
                .bind(&prop.id)
                .execute(&mut *conn)
                .await?;
        }

        // Insert new property (with supersedes_property_id for audit trail)
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let supersedes_id = active.first().map(|p| p.id.as_str());
        sqlx::query(
            "INSERT INTO entity_properties (id, book_id, entity_id, dimension_key, value_text, value_json, valid_from_chapter, source_claim_id, confidence, status, supersedes_property_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(entity_id)
        .bind(dimension_key)
        .bind(value_text)
        .bind(value_json)
        .bind(valid_from_chapter)
        .bind(source_claim_id)
        .bind(confidence)
        .bind(supersedes_id)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        // Upsert current property
        sqlx::query(
            "INSERT INTO entity_current_properties (book_id, entity_id, dimension_key, property_id, value_text, value_json, updated_chapter, confidence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(book_id, entity_id, dimension_key) DO UPDATE SET
               property_id = excluded.property_id, value_text = excluded.value_text, value_json = excluded.value_json,
               updated_chapter = excluded.updated_chapter, confidence = excluded.confidence"
        )
        .bind(book_id)
        .bind(entity_id)
        .bind(dimension_key)
        .bind(&id)
        .bind(value_text)
        .bind(value_json)
        .bind(valid_from_chapter)
        .bind(confidence)
        .execute(conn)
        .await?;

        Ok(())
    }

    pub async fn apply_append_with_conn(
        conn: &mut sqlx::SqliteConnection,
        book_id: &str,
        entity_id: &str,
        dimension_key: &str,
        value_text: Option<&str>,
        value_json: Option<&str>,
        valid_from_chapter: i64,
        source_claim_id: &str,
        confidence: f64,
    ) -> anyhow::Result<()> {
        // Insert new property (don't supersede old ones)
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO entity_properties (id, book_id, entity_id, dimension_key, value_text, value_json, valid_from_chapter, source_claim_id, confidence, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(entity_id)
        .bind(dimension_key)
        .bind(value_text)
        .bind(value_json)
        .bind(valid_from_chapter)
        .bind(source_claim_id)
        .bind(confidence)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        // Get all active properties for aggregation
        let active = sqlx::query_as::<_, PropertyRow>(
            "SELECT id, book_id, entity_id, dimension_key, value_text, value_json, valid_from_chapter, valid_to_chapter, source_claim_id, confidence, status, supersedes_property_id
             FROM entity_properties WHERE entity_id = ? AND dimension_key = ? AND status = 'active' ORDER BY valid_from_chapter ASC"
        )
        .bind(entity_id)
        .bind(dimension_key)
        .fetch_all(&mut *conn)
        .await?;

        // Aggregate values
        let mut all_values: Vec<serde_json::Value> = Vec::new();
        let mut summary_parts: Vec<String> = Vec::new();
        for prop in &active {
            if let Some(vj) = &prop.value_json {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(vj) {
                    if let Some(arr) = parsed.as_array() {
                        all_values.extend(arr.clone());
                    } else {
                        all_values.push(parsed);
                    }
                }
            } else if let Some(vt) = &prop.value_text {
                all_values.push(serde_json::Value::String(vt.clone()));
                summary_parts.push(vt.clone());
            }
        }

        let aggregated_json = serde_json::to_string(&all_values).ok();
        let aggregated_text = if summary_parts.is_empty() {
            value_text.map(|s| s.to_string())
        } else {
            Some(summary_parts.join("; "))
        };

        // Upsert current property with aggregated values
        sqlx::query(
            "INSERT INTO entity_current_properties (book_id, entity_id, dimension_key, property_id, value_text, value_json, updated_chapter, confidence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(book_id, entity_id, dimension_key) DO UPDATE SET
               property_id = excluded.property_id, value_text = excluded.value_text, value_json = excluded.value_json,
               updated_chapter = excluded.updated_chapter, confidence = excluded.confidence"
        )
        .bind(book_id)
        .bind(entity_id)
        .bind(dimension_key)
        .bind(&id)
        .bind(aggregated_text.as_deref())
        .bind(aggregated_json.as_deref())
        .bind(valid_from_chapter)
        .bind(confidence)
        .execute(conn)
        .await?;

        Ok(())
    }
}

// SQLx row types

#[derive(sqlx::FromRow)]
struct DimensionRow {
    id: String,
    book_id: String,
    entity_type: String,
    dimension_key: String,
    display_name: String,
    value_type: String,
    importance: f64,
    merge_strategy: String,
}

impl From<DimensionRow> for DimensionRecord {
    fn from(r: DimensionRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            entity_type: r.entity_type,
            dimension_key: r.dimension_key,
            display_name: r.display_name,
            value_type: r.value_type,
            importance: r.importance,
            merge_strategy: r.merge_strategy,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PropertyRow {
    id: String,
    book_id: String,
    entity_id: String,
    dimension_key: String,
    value_text: Option<String>,
    value_json: Option<String>,
    valid_from_chapter: i64,
    valid_to_chapter: Option<i64>,
    source_claim_id: String,
    confidence: f64,
    status: String,
    supersedes_property_id: Option<String>,
}

impl From<PropertyRow> for PropertyRecord {
    fn from(r: PropertyRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            entity_id: r.entity_id,
            dimension_key: r.dimension_key,
            value_text: r.value_text,
            value_json: r.value_json,
            valid_from_chapter: r.valid_from_chapter,
            valid_to_chapter: r.valid_to_chapter,
            source_claim_id: r.source_claim_id,
            confidence: r.confidence,
            status: r.status,
            supersedes_property_id: r.supersedes_property_id,
        }
    }
}

#[derive(sqlx::FromRow)]
struct CurrentPropertyRow {
    book_id: String,
    entity_id: String,
    dimension_key: String,
    property_id: String,
    value_text: Option<String>,
    value_json: Option<String>,
    updated_chapter: i64,
    confidence: f64,
}

impl From<CurrentPropertyRow> for CurrentPropertyRecord {
    fn from(r: CurrentPropertyRow) -> Self {
        Self {
            book_id: r.book_id,
            entity_id: r.entity_id,
            dimension_key: r.dimension_key,
            property_id: r.property_id,
            value_text: r.value_text,
            value_json: r.value_json,
            updated_chapter: r.updated_chapter,
            confidence: r.confidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::storage::db::v4::entity_repo::EntityRepo;

    async fn setup() -> (SqlitePool, PropertyRepo, EntityRepo) {
        let dir = std::env::temp_dir().join(format!("reader-v4-prop-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        let prop_repo = PropertyRepo::new(pool.clone());
        let entity_repo = EntityRepo::new(pool.clone());
        (pool, prop_repo, entity_repo)
    }

    async fn create_test_entity(entity_repo: &EntityRepo, book_id: &str, name: &str) -> String {
        entity_repo
            .create_entity(book_id, "character", name, name, None, 0.5, 1)
            .await
            .unwrap()
            .id
    }

    use std::sync::atomic::{AtomicI64, Ordering};

    static CLAIM_COUNTER: AtomicI64 = AtomicI64::new(1);

    async fn create_test_claim(pool: &SqlitePool, book_id: &str) -> String {
        let n = CLAIM_COUNTER.fetch_add(1, Ordering::SeqCst);
        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, ?, ?, 'test', 'hash', datetime('now'))")
            .bind(&chapter_id).bind(book_id).bind(n).execute(pool).await.unwrap();

        let segment_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, ?, ?, 'hash', 0, datetime('now'))")
            .bind(&segment_id).bind(book_id).bind(&chapter_id).execute(pool).await.unwrap();

        let span_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, ?, ?, 'hash', ?, 0, 0, 10, 'test text', datetime('now'))")
            .bind(&span_id).bind(book_id).bind(&chapter_id).bind(&segment_id).execute(pool).await.unwrap();

        let run_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, ?, ?, 'extract', 'test', 'v1', 1, 'hash', 'success', datetime('now'))")
            .bind(&run_id).bind(book_id).bind(&chapter_id).execute(pool).await.unwrap();

        let claim_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, ?, ?, 'property_update', 'test', ?, ?, 0.9, 'low', 'proposed', datetime('now'), datetime('now'))")
            .bind(&claim_id).bind(book_id).bind(n).bind(&span_id).bind(&run_id).execute(pool).await.unwrap();

        claim_id
    }

    #[tokio::test]
    async fn dimensions_seeded_and_queryable() {
        let (_pool, prop_repo, _entity) = setup().await;

        let dims = prop_repo
            .list_dimensions("__global__", "character")
            .await
            .unwrap();
        assert_eq!(dims.len(), 14, "should have 14 built-in dimensions");

        let realm = prop_repo
            .get_dimension("__global__", "character", "realm")
            .await
            .unwrap();
        assert!(realm.is_some());
        let realm = realm.unwrap();
        assert_eq!(realm.display_name, "Realm/Cultivation");
        assert_eq!(realm.merge_strategy, "replace");
    }

    #[tokio::test]
    async fn replace_strategy_supersedes_old() {
        let (pool, prop_repo, entity_repo) = setup().await;
        let entity_id = create_test_entity(&entity_repo, "b1", "张三").await;
        let claim1 = create_test_claim(&pool, "b1").await;
        let claim2 = create_test_claim(&pool, "b1").await;

        // First value
        let prop1 = prop_repo
            .apply_replace(
                "b1",
                &entity_id,
                "realm",
                Some("练气期"),
                None,
                1,
                &claim1,
                0.9,
            )
            .await
            .unwrap();
        assert_eq!(prop1.value_text.as_deref(), Some("练气期"));

        let current = prop_repo
            .get_current_property("b1", &entity_id, "realm")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.value_text.as_deref(), Some("练气期"));

        // Replace with new value
        let prop2 = prop_repo
            .apply_replace(
                "b1",
                &entity_id,
                "realm",
                Some("筑基期"),
                None,
                5,
                &claim2,
                0.95,
            )
            .await
            .unwrap();
        assert_eq!(prop2.value_text.as_deref(), Some("筑基期"));

        // Old should be superseded
        let old_prop =
            sqlx::query_as::<_, (String, Option<i64>)>("SELECT status, valid_to_chapter FROM entity_properties WHERE id = ?")
                .bind(&prop1.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(old_prop.0, "superseded");
        assert_eq!(old_prop.1, Some(4), "valid_to_chapter should be new.valid_from_chapter - 1");

        // New property should have supersedes_property_id pointing to old
        let new_prop =
            sqlx::query_as::<_, (Option<String>,)>("SELECT supersedes_property_id FROM entity_properties WHERE id = ?")
                .bind(&prop2.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(new_prop.0, Some(prop1.id.clone()), "new property should reference superseded property");

        // Current should be new value
        let current = prop_repo
            .get_current_property("b1", &entity_id, "realm")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.value_text.as_deref(), Some("筑基期"));
        assert_eq!(current.property_id, prop2.id);
    }

    #[tokio::test]
    async fn append_strategy_aggregates() {
        let (pool, prop_repo, entity_repo) = setup().await;
        let entity_id = create_test_entity(&entity_repo, "b1", "李四").await;
        let claim1 = create_test_claim(&pool, "b1").await;
        let claim2 = create_test_claim(&pool, "b1").await;

        prop_repo
            .apply_append(
                "b1",
                &entity_id,
                "ability",
                Some("剑法"),
                None,
                1,
                &claim1,
                0.8,
            )
            .await
            .unwrap();

        prop_repo
            .apply_append(
                "b1",
                &entity_id,
                "ability",
                Some("拳法"),
                None,
                3,
                &claim2,
                0.7,
            )
            .await
            .unwrap();

        // Both properties should be active
        let mut conn = pool.acquire().await.unwrap();
        let active = prop_repo
            .get_active_properties(&mut conn, &entity_id, "ability")
            .await
            .unwrap();
        assert_eq!(active.len(), 2);

        // Current should aggregate
        let current = prop_repo
            .get_current_property("b1", &entity_id, "ability")
            .await
            .unwrap()
            .unwrap();
        assert!(current.value_text.as_deref().unwrap().contains("剑法"));
        assert!(current.value_text.as_deref().unwrap().contains("拳法"));
    }

    #[tokio::test]
    async fn validate_dimension_accepts_registered() {
        let (_pool, prop_repo, _entity) = setup().await;

        let result = prop_repo
            .validate_dimension("__global__", "character", "realm")
            .await
            .unwrap();
        assert!(result.is_some());

        let result = prop_repo
            .validate_dimension("__global__", "character", "nonexistent")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn validate_dimension_falls_back_to_global() {
        let (_pool, prop_repo, _entity) = setup().await;

        // With a specific book_id, should still find global dimensions
        let result = prop_repo
            .validate_dimension("b1", "character", "realm")
            .await
            .unwrap();
        assert!(result.is_some(), "should fall back to global dimensions");
        assert_eq!(result.unwrap().dimension_key, "realm");

        // Non-existent dimension still returns None
        let result = prop_repo
            .validate_dimension("b1", "character", "nonexistent")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn current_property_upsert_updates() {
        let (pool, prop_repo, entity_repo) = setup().await;
        let entity_id = create_test_entity(&entity_repo, "b1", "测试").await;
        let claim1 = create_test_claim(&pool, "b1").await;
        let claim2 = create_test_claim(&pool, "b1").await;

        // Create actual properties first
        let mut conn = pool.acquire().await.unwrap();
        let prop1 = prop_repo
            .insert_property(
                &mut conn,
                "b1",
                &entity_id,
                "realm",
                Some("练气期"),
                None,
                1,
                &claim1,
                0.9,
                None,
            )
            .await
            .unwrap();
        let prop2 = prop_repo
            .insert_property(
                &mut conn,
                "b1",
                &entity_id,
                "realm",
                Some("筑基期"),
                None,
                5,
                &claim2,
                0.95,
                None,
            )
            .await
            .unwrap();

        prop_repo
            .upsert_current_property(
                &mut conn,
                "b1",
                &entity_id,
                "realm",
                &prop1.id,
                Some("练气期"),
                None,
                1,
                0.9,
            )
            .await
            .unwrap();
        prop_repo
            .upsert_current_property(
                &mut conn,
                "b1",
                &entity_id,
                "realm",
                &prop2.id,
                Some("筑基期"),
                None,
                5,
                0.95,
            )
            .await
            .unwrap();

        let current = prop_repo
            .get_current_property("b1", &entity_id, "realm")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.property_id, prop2.id);
        assert_eq!(current.value_text.as_deref(), Some("筑基期"));
        assert_eq!(current.updated_chapter, 5);
    }
}
