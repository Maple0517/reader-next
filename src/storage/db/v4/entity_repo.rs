use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct EntityRecord {
    pub id: String,
    pub book_id: String,
    pub entity_type: String,
    pub canonical_name: String,
    pub display_name: String,
    pub short_summary: Option<String>,
    pub importance_score: f64,
    pub first_seen_chapter: i64,
    pub last_seen_chapter: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct AliasRecord {
    pub id: String,
    pub book_id: String,
    pub entity_id: String,
    pub alias: String,
    pub alias_type: String,
    pub first_seen_chapter: i64,
    pub last_seen_chapter: Option<i64>,
    pub confidence: f64,
    pub source_claim_id: Option<String>,
}

pub struct EntityRepo {
    pool: SqlitePool,
}

impl EntityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_entity(
        &self,
        book_id: &str,
        entity_type: &str,
        canonical_name: &str,
        display_name: &str,
        short_summary: Option<&str>,
        importance_score: f64,
        first_seen_chapter: i64,
    ) -> anyhow::Result<EntityRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let normalized = normalize_name(canonical_name);

        sqlx::query(
            "INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, short_summary, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(entity_type)
        .bind(&normalized)
        .bind(display_name)
        .bind(short_summary)
        .bind(importance_score)
        .bind(first_seen_chapter)
        .bind(first_seen_chapter)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(EntityRecord {
            id,
            book_id: book_id.to_string(),
            entity_type: entity_type.to_string(),
            canonical_name: normalized,
            display_name: display_name.to_string(),
            short_summary: short_summary.map(|s| s.to_string()),
            importance_score,
            first_seen_chapter,
            last_seen_chapter: first_seen_chapter,
            status: "active".to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn get_by_id(&self, entity_id: &str) -> anyhow::Result<Option<EntityRecord>> {
        let row = sqlx::query_as::<_, EntityRow>(
            "SELECT id, book_id, entity_type, canonical_name, display_name, short_summary, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at FROM entities WHERE id = ?"
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn get_by_canonical_name(
        &self,
        book_id: &str,
        canonical_name: &str,
    ) -> anyhow::Result<Option<EntityRecord>> {
        let normalized = normalize_name(canonical_name);
        let row = sqlx::query_as::<_, EntityRow>(
            "SELECT id, book_id, entity_type, canonical_name, display_name, short_summary, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at FROM entities WHERE book_id = ? AND canonical_name = ?"
        )
        .bind(book_id)
        .bind(&normalized)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn list_by_book(&self, book_id: &str) -> anyhow::Result<Vec<EntityRecord>> {
        let rows = sqlx::query_as::<_, EntityRow>(
            "SELECT id, book_id, entity_type, canonical_name, display_name, short_summary, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at FROM entities WHERE book_id = ? AND status = 'active' ORDER BY importance_score DESC, canonical_name ASC"
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// List active entities for a book, sorted by last_seen_chapter DESC (most recent first).
    pub async fn list_recent_active(&self, book_id: &str) -> anyhow::Result<Vec<EntityRecord>> {
        let rows = sqlx::query_as::<_, EntityRow>(
            "SELECT id, book_id, entity_type, canonical_name, display_name, short_summary, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at FROM entities WHERE book_id = ? AND status = 'active' ORDER BY last_seen_chapter DESC, importance_score DESC"
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn update_last_seen(
        &self,
        entity_id: &str,
        chapter_index: i64,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE entities SET last_seen_chapter = ?, updated_at = ? WHERE id = ? AND last_seen_chapter < ?"
        )
        .bind(chapter_index)
        .bind(&now)
        .bind(entity_id)
        .bind(chapter_index)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_display_name(
        &self,
        entity_id: &str,
        display_name: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE entities SET display_name = ?, updated_at = ? WHERE id = ?")
            .bind(display_name)
            .bind(&now)
            .bind(entity_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // Alias operations

    pub async fn create_alias(
        &self,
        book_id: &str,
        entity_id: &str,
        alias: &str,
        alias_type: &str,
        first_seen_chapter: i64,
        confidence: f64,
        source_claim_id: Option<&str>,
    ) -> anyhow::Result<AliasRecord> {
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT OR IGNORE INTO entity_aliases (id, book_id, entity_id, alias, alias_type, first_seen_chapter, confidence, source_claim_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(entity_id)
        .bind(alias)
        .bind(alias_type)
        .bind(first_seen_chapter)
        .bind(confidence)
        .bind(source_claim_id)
        .execute(&self.pool)
        .await?;

        Ok(AliasRecord {
            id,
            book_id: book_id.to_string(),
            entity_id: entity_id.to_string(),
            alias: alias.to_string(),
            alias_type: alias_type.to_string(),
            first_seen_chapter,
            last_seen_chapter: None,
            confidence,
            source_claim_id: source_claim_id.map(|s| s.to_string()),
        })
    }

    pub async fn get_by_alias(
        &self,
        book_id: &str,
        alias: &str,
    ) -> anyhow::Result<Vec<AliasRecord>> {
        let rows = sqlx::query_as::<_, AliasRow>(
            "SELECT id, book_id, entity_id, alias, alias_type, first_seen_chapter, last_seen_chapter, confidence, source_claim_id FROM entity_aliases WHERE book_id = ? AND alias = ?"
        )
        .bind(book_id)
        .bind(alias)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn list_aliases_by_entity(
        &self,
        entity_id: &str,
    ) -> anyhow::Result<Vec<AliasRecord>> {
        let rows = sqlx::query_as::<_, AliasRow>(
            "SELECT id, book_id, entity_id, alias, alias_type, first_seen_chapter, last_seen_chapter, confidence, source_claim_id FROM entity_aliases WHERE entity_id = ? ORDER BY first_seen_chapter ASC"
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn find_entity_by_alias(
        &self,
        book_id: &str,
        alias: &str,
    ) -> anyhow::Result<Option<EntityRecord>> {
        let row = sqlx::query_as::<_, EntityRow>(
            "SELECT e.id, e.book_id, e.entity_type, e.canonical_name, e.display_name, e.short_summary, e.importance_score, e.first_seen_chapter, e.last_seen_chapter, e.status, e.created_at, e.updated_at
             FROM entities e
             JOIN entity_aliases a ON a.entity_id = e.id
             WHERE e.book_id = ? AND a.alias = ? AND e.status = 'active'
             LIMIT 1"
        )
        .bind(book_id)
        .bind(alias)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    // Transaction-aware methods for use within reducer

    pub async fn create_entity_with_conn(
        conn: &mut sqlx::SqliteConnection,
        book_id: &str,
        entity_type: &str,
        canonical_name: &str,
        display_name: &str,
        short_summary: Option<&str>,
        importance_score: f64,
        first_seen_chapter: i64,
    ) -> anyhow::Result<EntityRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let normalized = normalize_name(canonical_name);

        sqlx::query(
            "INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, short_summary, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(entity_type)
        .bind(&normalized)
        .bind(display_name)
        .bind(short_summary)
        .bind(importance_score)
        .bind(first_seen_chapter)
        .bind(first_seen_chapter)
        .bind(&now)
        .bind(&now)
        .execute(conn)
        .await?;

        Ok(EntityRecord {
            id,
            book_id: book_id.to_string(),
            entity_type: entity_type.to_string(),
            canonical_name: normalized,
            display_name: display_name.to_string(),
            short_summary: short_summary.map(|s| s.to_string()),
            importance_score,
            first_seen_chapter,
            last_seen_chapter: first_seen_chapter,
            status: "active".to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn update_last_seen_with_conn(
        conn: &mut sqlx::SqliteConnection,
        entity_id: &str,
        chapter_index: i64,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE entities SET last_seen_chapter = ?, updated_at = ? WHERE id = ? AND last_seen_chapter < ?"
        )
        .bind(chapter_index)
        .bind(&now)
        .bind(entity_id)
        .bind(chapter_index)
        .execute(conn)
        .await?;

        Ok(())
    }

    pub async fn create_alias_with_conn(
        conn: &mut sqlx::SqliteConnection,
        book_id: &str,
        entity_id: &str,
        alias: &str,
        alias_type: &str,
        first_seen_chapter: i64,
        confidence: f64,
        source_claim_id: Option<&str>,
    ) -> anyhow::Result<AliasRecord> {
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT OR IGNORE INTO entity_aliases (id, book_id, entity_id, alias, alias_type, first_seen_chapter, confidence, source_claim_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(entity_id)
        .bind(alias)
        .bind(alias_type)
        .bind(first_seen_chapter)
        .bind(confidence)
        .bind(source_claim_id)
        .execute(conn)
        .await?;

        Ok(AliasRecord {
            id,
            book_id: book_id.to_string(),
            entity_id: entity_id.to_string(),
            alias: alias.to_string(),
            alias_type: alias_type.to_string(),
            first_seen_chapter,
            last_seen_chapter: None,
            confidence,
            source_claim_id: source_claim_id.map(|s| s.to_string()),
        })
    }

    pub async fn find_entity_by_alias_with_conn(
        conn: &mut sqlx::SqliteConnection,
        book_id: &str,
        alias: &str,
    ) -> anyhow::Result<Option<EntityRecord>> {
        let row = sqlx::query_as::<_, EntityRow>(
            "SELECT e.id, e.book_id, e.entity_type, e.canonical_name, e.display_name, e.short_summary, e.importance_score, e.first_seen_chapter, e.last_seen_chapter, e.status, e.created_at, e.updated_at
             FROM entities e
             JOIN entity_aliases a ON a.entity_id = e.id
             WHERE e.book_id = ? AND a.alias = ? AND e.status = 'active'
             LIMIT 1"
        )
        .bind(book_id)
        .bind(alias)
        .fetch_optional(conn)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn get_by_id_with_conn(
        conn: &mut sqlx::SqliteConnection,
        entity_id: &str,
    ) -> anyhow::Result<Option<EntityRecord>> {
        let row = sqlx::query_as::<_, EntityRow>(
            "SELECT id, book_id, entity_type, canonical_name, display_name, short_summary, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at FROM entities WHERE id = ?"
        )
        .bind(entity_id)
        .fetch_optional(conn)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn get_by_canonical_name_with_conn(
        conn: &mut sqlx::SqliteConnection,
        book_id: &str,
        canonical_name: &str,
    ) -> anyhow::Result<Option<EntityRecord>> {
        let normalized = normalize_name(canonical_name);
        let row = sqlx::query_as::<_, EntityRow>(
            "SELECT id, book_id, entity_type, canonical_name, display_name, short_summary, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at FROM entities WHERE book_id = ? AND canonical_name = ?"
        )
        .bind(book_id)
        .bind(&normalized)
        .fetch_optional(conn)
        .await?;

        Ok(row.map(|r| r.into()))
    }
}

/// Normalize canonical name: trim + full-width to half-width + lowercase.
pub fn normalize_name(name: &str) -> String {
    let trimmed = name.trim();
    let mut result = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        match ch {
            '\u{FF01}'..='\u{FF5E}' => {
                // Full-width ASCII to half-width
                result.push((ch as u32 - 0xFF01 + 0x21) as u8 as char);
            }
            '\u{3000}' => result.push(' '), // Ideographic space
            _ => result.push(ch),
        }
    }
    result.to_lowercase()
}

// SQLx row types for query_as

#[derive(sqlx::FromRow)]
struct EntityRow {
    id: String,
    book_id: String,
    entity_type: String,
    canonical_name: String,
    display_name: String,
    short_summary: Option<String>,
    importance_score: f64,
    first_seen_chapter: i64,
    last_seen_chapter: i64,
    status: String,
    created_at: String,
    updated_at: String,
}

impl From<EntityRow> for EntityRecord {
    fn from(r: EntityRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            entity_type: r.entity_type,
            canonical_name: r.canonical_name,
            display_name: r.display_name,
            short_summary: r.short_summary,
            importance_score: r.importance_score,
            first_seen_chapter: r.first_seen_chapter,
            last_seen_chapter: r.last_seen_chapter,
            status: r.status,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct AliasRow {
    id: String,
    book_id: String,
    entity_id: String,
    alias: String,
    alias_type: String,
    first_seen_chapter: i64,
    last_seen_chapter: Option<i64>,
    confidence: f64,
    source_claim_id: Option<String>,
}

impl From<AliasRow> for AliasRecord {
    fn from(r: AliasRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            entity_id: r.entity_id,
            alias: r.alias,
            alias_type: r.alias_type,
            first_seen_chapter: r.first_seen_chapter,
            last_seen_chapter: r.last_seen_chapter,
            confidence: r.confidence,
            source_claim_id: r.source_claim_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup() -> (SqlitePool, EntityRepo) {
        let dir = std::env::temp_dir().join(format!("reader-v4-entity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        let repo = EntityRepo::new(pool.clone());
        (pool, repo)
    }

    #[tokio::test]
    async fn create_and_get_entity() {
        let (_pool, repo) = setup().await;
        let entity = repo
            .create_entity("book1", "character", "张三", "张三", Some("主角"), 0.9, 1)
            .await
            .unwrap();

        assert_eq!(entity.entity_type, "character");
        assert_eq!(entity.canonical_name, "张三");
        assert_eq!(entity.display_name, "张三");
        assert_eq!(entity.first_seen_chapter, 1);

        let fetched = repo.get_by_id(&entity.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, entity.id);
        assert_eq!(fetched.canonical_name, "张三");
    }

    #[tokio::test]
    async fn get_by_canonical_name() {
        let (_pool, repo) = setup().await;
        repo.create_entity("book1", "character", "李四", "李四", None, 0.5, 1)
            .await
            .unwrap();

        let found = repo.get_by_canonical_name("book1", "李四").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().canonical_name, "李四");

        let not_found = repo.get_by_canonical_name("book1", "不存在").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn list_by_book_returns_sorted() {
        let (_pool, repo) = setup().await;
        repo.create_entity("book1", "character", "配角", "配角", None, 0.3, 1)
            .await
            .unwrap();
        repo.create_entity("book1", "character", "主角", "主角", None, 0.9, 1)
            .await
            .unwrap();
        repo.create_entity("book2", "character", "其他书", "其他书", None, 0.5, 1)
            .await
            .unwrap();

        let list = repo.list_by_book("book1").await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].canonical_name, "主角"); // higher importance first
        assert_eq!(list[1].canonical_name, "配角");
    }

    #[tokio::test]
    async fn update_last_seen() {
        let (_pool, repo) = setup().await;
        let entity = repo
            .create_entity("book1", "character", "测试", "测试", None, 0.5, 1)
            .await
            .unwrap();

        repo.update_last_seen(&entity.id, 5).await.unwrap();

        let updated = repo.get_by_id(&entity.id).await.unwrap().unwrap();
        assert_eq!(updated.last_seen_chapter, 5);

        // Should not decrease
        repo.update_last_seen(&entity.id, 3).await.unwrap();
        let unchanged = repo.get_by_id(&entity.id).await.unwrap().unwrap();
        assert_eq!(unchanged.last_seen_chapter, 5);
    }

    #[tokio::test]
    async fn create_and_list_aliases() {
        let (_pool, repo) = setup().await;
        let entity = repo
            .create_entity("book1", "character", "张三", "张三", None, 0.5, 1)
            .await
            .unwrap();

        repo.create_alias("book1", &entity.id, "小三", "nickname", 1, 0.8, None)
            .await
            .unwrap();
        repo.create_alias("book1", &entity.id, "张三丰", "alias", 3, 0.9, None)
            .await
            .unwrap();

        let aliases = repo.list_aliases_by_entity(&entity.id).await.unwrap();
        assert_eq!(aliases.len(), 2);
    }

    #[tokio::test]
    async fn find_entity_by_alias() {
        let (_pool, repo) = setup().await;
        let entity = repo
            .create_entity("book1", "character", "张三", "张三", None, 0.5, 1)
            .await
            .unwrap();
        repo.create_alias("book1", &entity.id, "小三", "nickname", 1, 0.8, None)
            .await
            .unwrap();

        let found = repo.find_entity_by_alias("book1", "小三").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, entity.id);

        let not_found = repo.find_entity_by_alias("book1", "不存在").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn alias_duplicate_ignored() {
        let (_pool, repo) = setup().await;
        let entity = repo
            .create_entity("book1", "character", "张三", "张三", None, 0.5, 1)
            .await
            .unwrap();

        repo.create_alias("book1", &entity.id, "小三", "nickname", 1, 0.8, None)
            .await
            .unwrap();
        // Second insert with same (book_id, entity_id, alias) should be ignored
        repo.create_alias("book1", &entity.id, "小三", "nickname", 2, 0.9, None)
            .await
            .unwrap();

        let aliases = repo.list_aliases_by_entity(&entity.id).await.unwrap();
        assert_eq!(aliases.len(), 1, "duplicate alias should be ignored");
    }

    #[tokio::test]
    async fn normalize_name_consistency() {
        assert_eq!(normalize_name("  张三  "), "张三");
        assert_eq!(normalize_name("Ｚｈａｎｇ"), "zhang");
        assert_eq!(normalize_name("ＺＨＡＮＧ"), "zhang");
        assert_eq!(normalize_name("　全角空格"), "全角空格"); // ideographic space
    }

    #[tokio::test]
    async fn duplicate_entity_rejected_by_unique_constraint() {
        let (_pool, repo) = setup().await;
        repo.create_entity("book1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        // Second insert with same (book_id, entity_type, canonical_name) should fail
        let result = repo
            .create_entity("book1", "character", "张三", "张三改", None, 0.5, 2)
            .await;
        assert!(result.is_err(), "duplicate entity should be rejected by UNIQUE constraint");
    }

    #[tokio::test]
    async fn same_name_different_entity_type_allowed() {
        let (_pool, repo) = setup().await;
        repo.create_entity("book1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        // Same canonical_name but different entity_type should succeed
        let result = repo
            .create_entity("book1", "location", "张三", "张三城", None, 0.3, 1)
            .await;
        assert!(result.is_ok(), "same name different type should be allowed");
    }
}
