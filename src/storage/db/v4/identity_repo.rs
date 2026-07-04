use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone)]
pub struct IdentityLinkRecord {
    pub id: String,
    pub book_id: String,
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub link_type: String,
    pub confidence: f64,
    pub source_claim_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct MergeOperationRecord {
    pub id: String,
    pub book_id: String,
    pub survivor_entity_id: String,
    pub victim_entity_id: String,
    pub source_identity_link_id: String,
    pub reason_code: String,
    pub confidence: f64,
    pub status: String,
    pub property_conflict_count: i64,
    pub relationship_merge_count: i64,
    pub result_json: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MergeConflictRecord {
    pub id: String,
    pub book_id: String,
    pub merge_operation_id: String,
    pub survivor_entity_id: String,
    pub victim_entity_id: String,
    pub conflict_type: String,
    pub dimension_key: Option<String>,
    pub survivor_value: Option<String>,
    pub victim_value: Option<String>,
    pub resolution: String,
    pub created_at: String,
}

pub struct IdentityRepo {
    pool: SqlitePool,
}

impl IdentityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_identity_link(
        &self,
        _book_id: &str,
        _entity_a_id: &str,
        _entity_b_id: &str,
        _link_type: &str,
        _confidence: f64,
        _source_claim_id: &str,
        _status: &str,
    ) -> anyhow::Result<IdentityLinkRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::find_or_create_identity_link_with_conn(
            &mut conn,
            _book_id,
            _entity_a_id,
            _entity_b_id,
            _link_type,
            _confidence,
            _source_claim_id,
            _status,
        )
        .await
    }

    pub async fn get_identity_link_by_id(
        &self,
        link_id: &str,
    ) -> anyhow::Result<Option<IdentityLinkRecord>> {
        let row = sqlx::query_as::<_, IdentityLinkRow>(
            "SELECT id, book_id, entity_a_id, entity_b_id, link_type, confidence, source_claim_id, status, created_at, updated_at
             FROM entity_identity_links WHERE id = ?",
        )
        .bind(link_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    pub async fn list_identity_links_by_book(
        &self,
        book_id: &str,
        status: Option<&str>,
    ) -> anyhow::Result<Vec<IdentityLinkRecord>> {
        let rows = if let Some(status) = status {
            sqlx::query_as::<_, IdentityLinkRow>(
                "SELECT id, book_id, entity_a_id, entity_b_id, link_type, confidence, source_claim_id, status, created_at, updated_at
                 FROM entity_identity_links
                 WHERE book_id = ? AND status = ?
                 ORDER BY created_at ASC, id ASC",
            )
            .bind(book_id)
            .bind(status)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, IdentityLinkRow>(
                "SELECT id, book_id, entity_a_id, entity_b_id, link_type, confidence, source_claim_id, status, created_at, updated_at
                 FROM entity_identity_links
                 WHERE book_id = ?
                 ORDER BY created_at ASC, id ASC",
            )
            .bind(book_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn resolve_redirect_target(
        &self,
        book_id: &str,
        entity_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT entity_b_id
             FROM entity_identity_links
             WHERE book_id = ? AND entity_a_id = ?
               AND link_type = 'redirect' AND status = 'active'
             LIMIT 1",
        )
        .bind(book_id)
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0))
    }

    pub async fn find_or_create_identity_link_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        entity_a_id: &str,
        entity_b_id: &str,
        link_type: &str,
        confidence: f64,
        source_claim_id: &str,
        status: &str,
    ) -> anyhow::Result<IdentityLinkRecord> {
        let (entity_a_id, entity_b_id) =
            canonicalize_identity_pair(entity_a_id, entity_b_id, link_type);

        let existing = if link_type == "redirect" {
            sqlx::query_as::<_, IdentityLinkRow>(
                "SELECT id, book_id, entity_a_id, entity_b_id, link_type, confidence, source_claim_id, status, created_at, updated_at
                 FROM entity_identity_links
                 WHERE book_id = ? AND entity_a_id = ? AND entity_b_id = ? AND link_type = ? AND status = ?
                 LIMIT 1",
            )
            .bind(book_id)
            .bind(&entity_a_id)
            .bind(&entity_b_id)
            .bind(link_type)
            .bind(status)
            .fetch_optional(&mut *conn)
            .await?
        } else {
            sqlx::query_as::<_, IdentityLinkRow>(
                "SELECT id, book_id, entity_a_id, entity_b_id, link_type, confidence, source_claim_id, status, created_at, updated_at
                 FROM entity_identity_links
                 WHERE book_id = ? AND entity_a_id = ? AND entity_b_id = ? AND link_type = ? AND status = ?
                 LIMIT 1",
            )
            .bind(book_id)
            .bind(&entity_a_id)
            .bind(&entity_b_id)
            .bind(link_type)
            .bind(status)
            .fetch_optional(&mut *conn)
            .await?
        };

        if let Some(row) = existing {
            return Ok(row.into());
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO entity_identity_links (id, book_id, entity_a_id, entity_b_id, link_type, confidence, source_claim_id, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(book_id)
        .bind(&entity_a_id)
        .bind(&entity_b_id)
        .bind(link_type)
        .bind(confidence)
        .bind(source_claim_id)
        .bind(status)
        .bind(&now)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        Ok(IdentityLinkRecord {
            id,
            book_id: book_id.to_string(),
            entity_a_id,
            entity_b_id,
            link_type: link_type.to_string(),
            confidence,
            source_claim_id: source_claim_id.to_string(),
            status: status.to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn update_identity_link_status_with_conn(
        conn: &mut SqliteConnection,
        link_id: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE entity_identity_links
             SET status = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(status)
        .bind(&now)
        .bind(link_id)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn create_merge_operation_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        survivor_entity_id: &str,
        victim_entity_id: &str,
        source_identity_link_id: &str,
        reason_code: &str,
        confidence: f64,
        status: &str,
    ) -> anyhow::Result<MergeOperationRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO entity_merge_operations (
                id, book_id, survivor_entity_id, victim_entity_id, source_identity_link_id,
                reason_code, confidence, status, property_conflict_count, relationship_merge_count,
                result_json, created_at, completed_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, 0, NULL, ?, NULL)",
        )
        .bind(&id)
        .bind(book_id)
        .bind(survivor_entity_id)
        .bind(victim_entity_id)
        .bind(source_identity_link_id)
        .bind(reason_code)
        .bind(confidence)
        .bind(status)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        Ok(MergeOperationRecord {
            id,
            book_id: book_id.to_string(),
            survivor_entity_id: survivor_entity_id.to_string(),
            victim_entity_id: victim_entity_id.to_string(),
            source_identity_link_id: source_identity_link_id.to_string(),
            reason_code: reason_code.to_string(),
            confidence,
            status: status.to_string(),
            property_conflict_count: 0,
            relationship_merge_count: 0,
            result_json: None,
            created_at: now,
            completed_at: None,
        })
    }

    pub async fn get_merge_operation_by_id(
        &self,
        operation_id: &str,
    ) -> anyhow::Result<Option<MergeOperationRecord>> {
        let row = sqlx::query_as::<_, MergeOperationRow>(
            "SELECT id, book_id, survivor_entity_id, victim_entity_id, source_identity_link_id, reason_code, confidence, status, property_conflict_count, relationship_merge_count, result_json, created_at, completed_at
             FROM entity_merge_operations
             WHERE id = ?",
        )
        .bind(operation_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    pub async fn list_merge_operations_by_book(
        &self,
        book_id: &str,
        status: Option<&str>,
    ) -> anyhow::Result<Vec<MergeOperationRecord>> {
        let rows = if let Some(status) = status {
            sqlx::query_as::<_, MergeOperationRow>(
                "SELECT id, book_id, survivor_entity_id, victim_entity_id, source_identity_link_id, reason_code, confidence, status, property_conflict_count, relationship_merge_count, result_json, created_at, completed_at
                 FROM entity_merge_operations
                 WHERE book_id = ? AND status = ?
                 ORDER BY created_at ASC, id ASC",
            )
            .bind(book_id)
            .bind(status)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, MergeOperationRow>(
                "SELECT id, book_id, survivor_entity_id, victim_entity_id, source_identity_link_id, reason_code, confidence, status, property_conflict_count, relationship_merge_count, result_json, created_at, completed_at
                 FROM entity_merge_operations
                 WHERE book_id = ?
                 ORDER BY created_at ASC, id ASC",
            )
            .bind(book_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn update_merge_operation_status_with_conn(
        conn: &mut SqliteConnection,
        operation_id: &str,
        status: &str,
        result_json: Option<&str>,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let completed_at = if status == "completed" || status == "failed" || status == "rolled_back"
        {
            Some(now.clone())
        } else {
            None
        };
        sqlx::query(
            "UPDATE entity_merge_operations
             SET status = ?, result_json = ?, completed_at = ?, created_at = created_at
             WHERE id = ?",
        )
        .bind(status)
        .bind(result_json)
        .bind(completed_at)
        .bind(operation_id)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub async fn create_merge_conflict_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        merge_operation_id: &str,
        survivor_entity_id: &str,
        victim_entity_id: &str,
        conflict_type: &str,
        dimension_key: Option<&str>,
        survivor_value: Option<&str>,
        victim_value: Option<&str>,
        resolution: &str,
    ) -> anyhow::Result<MergeConflictRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO entity_merge_conflicts (
                id, book_id, merge_operation_id, survivor_entity_id, victim_entity_id,
                conflict_type, dimension_key, survivor_value, victim_value, resolution, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(book_id)
        .bind(merge_operation_id)
        .bind(survivor_entity_id)
        .bind(victim_entity_id)
        .bind(conflict_type)
        .bind(dimension_key)
        .bind(survivor_value)
        .bind(victim_value)
        .bind(resolution)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        Ok(MergeConflictRecord {
            id,
            book_id: book_id.to_string(),
            merge_operation_id: merge_operation_id.to_string(),
            survivor_entity_id: survivor_entity_id.to_string(),
            victim_entity_id: victim_entity_id.to_string(),
            conflict_type: conflict_type.to_string(),
            dimension_key: dimension_key.map(|s| s.to_string()),
            survivor_value: survivor_value.map(|s| s.to_string()),
            victim_value: victim_value.map(|s| s.to_string()),
            resolution: resolution.to_string(),
            created_at: now,
        })
    }

    pub async fn list_merge_conflicts_by_operation(
        &self,
        merge_operation_id: &str,
    ) -> anyhow::Result<Vec<MergeConflictRecord>> {
        let rows = sqlx::query_as::<_, MergeConflictRow>(
            "SELECT id, book_id, merge_operation_id, survivor_entity_id, victim_entity_id, conflict_type, dimension_key, survivor_value, victim_value, resolution, created_at
             FROM entity_merge_conflicts
             WHERE merge_operation_id = ?
             ORDER BY created_at ASC, id ASC",
        )
        .bind(merge_operation_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

fn canonicalize_identity_pair(
    entity_a_id: &str,
    entity_b_id: &str,
    link_type: &str,
) -> (String, String) {
    if link_type == "redirect" || entity_a_id <= entity_b_id {
        (entity_a_id.to_string(), entity_b_id.to_string())
    } else {
        (entity_b_id.to_string(), entity_a_id.to_string())
    }
}

#[derive(sqlx::FromRow)]
struct IdentityLinkRow {
    id: String,
    book_id: String,
    entity_a_id: String,
    entity_b_id: String,
    link_type: String,
    confidence: f64,
    source_claim_id: String,
    status: String,
    created_at: String,
    updated_at: String,
}

impl From<IdentityLinkRow> for IdentityLinkRecord {
    fn from(row: IdentityLinkRow) -> Self {
        Self {
            id: row.id,
            book_id: row.book_id,
            entity_a_id: row.entity_a_id,
            entity_b_id: row.entity_b_id,
            link_type: row.link_type,
            confidence: row.confidence,
            source_claim_id: row.source_claim_id,
            status: row.status,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct MergeConflictRow {
    id: String,
    book_id: String,
    merge_operation_id: String,
    survivor_entity_id: String,
    victim_entity_id: String,
    conflict_type: String,
    dimension_key: Option<String>,
    survivor_value: Option<String>,
    victim_value: Option<String>,
    resolution: String,
    created_at: String,
}

impl From<MergeConflictRow> for MergeConflictRecord {
    fn from(row: MergeConflictRow) -> Self {
        Self {
            id: row.id,
            book_id: row.book_id,
            merge_operation_id: row.merge_operation_id,
            survivor_entity_id: row.survivor_entity_id,
            victim_entity_id: row.victim_entity_id,
            conflict_type: row.conflict_type,
            dimension_key: row.dimension_key,
            survivor_value: row.survivor_value,
            victim_value: row.victim_value,
            resolution: row.resolution,
            created_at: row.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct MergeOperationRow {
    id: String,
    book_id: String,
    survivor_entity_id: String,
    victim_entity_id: String,
    source_identity_link_id: String,
    reason_code: String,
    confidence: f64,
    status: String,
    property_conflict_count: i64,
    relationship_merge_count: i64,
    result_json: Option<String>,
    created_at: String,
    completed_at: Option<String>,
}

impl From<MergeOperationRow> for MergeOperationRecord {
    fn from(row: MergeOperationRow) -> Self {
        Self {
            id: row.id,
            book_id: row.book_id,
            survivor_entity_id: row.survivor_entity_id,
            victim_entity_id: row.victim_entity_id,
            source_identity_link_id: row.source_identity_link_id,
            reason_code: row.reason_code,
            confidence: row.confidence,
            status: row.status,
            property_conflict_count: row.property_conflict_count,
            relationship_merge_count: row.relationship_merge_count,
            result_json: row.result_json,
            created_at: row.created_at,
            completed_at: row.completed_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup() -> (SqlitePool, IdentityRepo) {
        let dir = std::env::temp_dir().join(format!("reader-v4-identity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        let repo = IdentityRepo::new(pool.clone());
        (pool, repo)
    }

    async fn setup_identity_infra(pool: &SqlitePool) {
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg1', 'b1', 'ch1', 'hash', 0, datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('ss1', 'b1', 'ch1', 'hash', 'seg1', 0, 0, 10, 'text', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'ch1', 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c1', 'b1', 1, 'identity_reveal', 'same_identity', 'ss1', 'run1', 0.9, 'high', 'proposed', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e1', 'b1', 'character', 'A', 'A', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e2', 'b1', 'character', 'B', 'B', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
    }

    async fn insert_identity_claim(
        pool: &SqlitePool,
        claim_id: &str,
        ai_run_id: &str,
        source_span_id: &str,
    ) {
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', 1, 'identity_reveal', 'same_identity', ?, ?, 0.9, 'high', 'proposed', datetime('now'), datetime('now'))")
            .bind(claim_id)
            .bind(source_span_id)
            .bind(ai_run_id)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn create_identity_link_canonicalizes_symmetric_pair() {
        let (pool, repo) = setup().await;
        setup_identity_infra(&pool).await;

        let link = repo
            .create_identity_link("b1", "e2", "e1", "same_identity", 0.9, "c1", "active")
            .await
            .unwrap();

        assert_eq!(link.entity_a_id, "e1");
        assert_eq!(link.entity_b_id, "e2");

        let persisted = repo
            .get_identity_link_by_id(&link.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(persisted.entity_a_id, "e1");
        assert_eq!(persisted.entity_b_id, "e2");
    }

    #[tokio::test]
    async fn find_or_create_identity_link_is_idempotent_for_same_source_type_and_pair() {
        let (pool, _repo) = setup().await;
        setup_identity_infra(&pool).await;
        let mut tx = pool.begin().await.unwrap();

        let first = IdentityRepo::find_or_create_identity_link_with_conn(
            &mut tx,
            "b1",
            "e1",
            "e2",
            "same_identity",
            0.9,
            "c1",
            "active",
        )
        .await
        .unwrap();

        let second = IdentityRepo::find_or_create_identity_link_with_conn(
            &mut tx,
            "b1",
            "e2",
            "e1",
            "same_identity",
            0.9,
            "c1",
            "active",
        )
        .await
        .unwrap();

        assert_eq!(first.id, second.id);
    }

    #[tokio::test]
    async fn create_merge_operation_and_conflict_records() {
        let (pool, repo) = setup().await;
        setup_identity_infra(&pool).await;
        let mut tx = pool.begin().await.unwrap();

        let link = IdentityRepo::find_or_create_identity_link_with_conn(
            &mut tx,
            "b1",
            "e1",
            "e2",
            "same_identity",
            0.9,
            "c1",
            "active",
        )
        .await
        .unwrap();

        let op = IdentityRepo::create_merge_operation_with_conn(
            &mut tx,
            "b1",
            "e1",
            "e2",
            &link.id,
            "identity_reveal",
            0.9,
            "pending",
        )
        .await
        .unwrap();

        let conflict = IdentityRepo::create_merge_conflict_with_conn(
            &mut tx,
            "b1",
            &op.id,
            "e1",
            "e2",
            "property_conflict",
            Some("identity"),
            Some("A"),
            Some("B"),
            "keep_survivor",
        )
        .await
        .unwrap();

        tx.commit().await.unwrap();

        let conflicts = repo
            .list_merge_conflicts_by_operation(&op.id)
            .await
            .unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].id, conflict.id);
        assert_eq!(conflicts[0].merge_operation_id, op.id);
    }

    #[tokio::test]
    async fn conflicting_active_redirect_target_returns_error() {
        let (pool, _repo) = setup().await;
        setup_identity_infra(&pool).await;
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e3', 'b1', 'character', 'C', 'C', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(&pool).await.unwrap();
        insert_identity_claim(&pool, "c2", "run1", "ss1").await;

        let mut tx = pool.begin().await.unwrap();
        IdentityRepo::find_or_create_identity_link_with_conn(
            &mut tx, "b1", "e1", "e2", "redirect", 0.9, "c1", "active",
        )
        .await
        .unwrap();

        let err = IdentityRepo::find_or_create_identity_link_with_conn(
            &mut tx, "b1", "e1", "e3", "redirect", 0.9, "c2", "active",
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string().contains("UNIQUE") || err.to_string().contains("constraint"),
            "expected conflicting redirect to surface a uniqueness error, got: {err}"
        );
    }

    #[tokio::test]
    async fn identity_links_can_be_listed_and_status_updated() {
        let (pool, repo) = setup().await;
        setup_identity_infra(&pool).await;
        let mut tx = pool.begin().await.unwrap();

        let link = IdentityRepo::find_or_create_identity_link_with_conn(
            &mut tx,
            "b1",
            "e1",
            "e2",
            "same_identity",
            0.9,
            "c1",
            "active",
        )
        .await
        .unwrap();

        IdentityRepo::update_identity_link_status_with_conn(&mut tx, &link.id, "inactive")
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let active = repo
            .list_identity_links_by_book("b1", Some("active"))
            .await
            .unwrap();
        let inactive = repo
            .list_identity_links_by_book("b1", Some("inactive"))
            .await
            .unwrap();

        assert!(active.is_empty(), "link should no longer be active");
        assert_eq!(inactive.len(), 1);
        assert_eq!(inactive[0].id, link.id);
        assert_eq!(inactive[0].status, "inactive");
    }

    #[tokio::test]
    async fn merge_operations_can_be_read_listed_and_completed() {
        let (pool, repo) = setup().await;
        setup_identity_infra(&pool).await;
        let mut tx = pool.begin().await.unwrap();

        let link = IdentityRepo::find_or_create_identity_link_with_conn(
            &mut tx,
            "b1",
            "e1",
            "e2",
            "same_identity",
            0.9,
            "c1",
            "active",
        )
        .await
        .unwrap();

        let op = IdentityRepo::create_merge_operation_with_conn(
            &mut tx,
            "b1",
            "e1",
            "e2",
            &link.id,
            "identity_reveal",
            0.9,
            "pending",
        )
        .await
        .unwrap();

        IdentityRepo::update_merge_operation_status_with_conn(
            &mut tx,
            &op.id,
            "completed",
            Some("{\"ok\":true}"),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let fetched = repo
            .get_merge_operation_by_id(&op.id)
            .await
            .unwrap()
            .unwrap();
        let completed = repo
            .list_merge_operations_by_book("b1", Some("completed"))
            .await
            .unwrap();

        assert_eq!(fetched.status, "completed");
        assert_eq!(fetched.result_json.as_deref(), Some("{\"ok\":true}"));
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, op.id);
    }
}
