use sqlx::SqlitePool;

pub struct CacheRepo {
    pool: SqlitePool,
}

impl CacheRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get cached payload_json for a specific (book_id, view_type, scope_id, max_chapter).
    pub async fn get_cached(
        &self,
        book_id: &str,
        view_type: &str,
        scope_id: &str,
        max_chapter: i64,
    ) -> anyhow::Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT payload_json FROM view_model_cache WHERE book_id = ? AND view_type = ? AND scope_id = ? AND max_chapter = ?"
        )
        .bind(book_id)
        .bind(view_type)
        .bind(scope_id)
        .bind(max_chapter)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0))
    }

    /// Set (upsert) a cache entry.
    pub async fn set_cached(
        &self,
        book_id: &str,
        view_type: &str,
        scope_id: &str,
        max_chapter: i64,
        payload_json: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO view_model_cache (book_id, view_type, scope_id, max_chapter, payload_json, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(book_id, view_type, scope_id, max_chapter) DO UPDATE SET
               payload_json = excluded.payload_json, updated_at = excluded.updated_at"
        )
        .bind(book_id)
        .bind(view_type)
        .bind(scope_id)
        .bind(max_chapter)
        .bind(payload_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Invalidate all cache entries for a specific (book_id, view_type, scope_id).
    pub async fn invalidate(
        &self,
        book_id: &str,
        view_type: &str,
        scope_id: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "DELETE FROM view_model_cache WHERE book_id = ? AND view_type = ? AND scope_id = ?",
        )
        .bind(book_id)
        .bind(view_type)
        .bind(scope_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Invalidate all cache entries for a book (all view_types, all scopes).
    pub async fn invalidate_all(&self, book_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM view_model_cache WHERE book_id = ?")
            .bind(book_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup() -> (SqlitePool, CacheRepo) {
        let dir = std::env::temp_dir().join(format!("reader-v4-cache-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        let repo = CacheRepo::new(pool.clone());
        (pool, repo)
    }

    #[tokio::test]
    async fn set_and_get_cached() {
        let (_pool, repo) = setup().await;

        repo.set_cached("b1", "character_card", "e1", 10, r#"{"name":"张三"}"#)
            .await
            .unwrap();

        let result = repo
            .get_cached("b1", "character_card", "e1", 10)
            .await
            .unwrap();
        assert_eq!(result.as_deref(), Some(r#"{"name":"张三"}"#));
    }

    #[tokio::test]
    async fn get_cached_miss_returns_none() {
        let (_pool, repo) = setup().await;

        let result = repo
            .get_cached("b1", "character_card", "nonexistent", 10)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn set_cached_upserts_on_conflict() {
        let (_pool, repo) = setup().await;

        repo.set_cached("b1", "character_card", "e1", 10, r#"{"v":1}"#)
            .await
            .unwrap();
        repo.set_cached("b1", "character_card", "e1", 10, r#"{"v":2}"#)
            .await
            .unwrap();

        let result = repo
            .get_cached("b1", "character_card", "e1", 10)
            .await
            .unwrap();
        assert_eq!(result.as_deref(), Some(r#"{"v":2}"#));
    }

    #[tokio::test]
    async fn invalidate_removes_matching_entries() {
        let (_pool, repo) = setup().await;

        repo.set_cached("b1", "character_card", "e1", 5, r#"{}"#)
            .await
            .unwrap();
        repo.set_cached("b1", "character_card", "e1", 10, r#"{}"#)
            .await
            .unwrap();
        repo.set_cached("b1", "character_card", "e2", 10, r#"{}"#)
            .await
            .unwrap();

        // Invalidate e1 only
        repo.invalidate("b1", "character_card", "e1").await.unwrap();

        assert!(repo
            .get_cached("b1", "character_card", "e1", 5)
            .await
            .unwrap()
            .is_none());
        assert!(repo
            .get_cached("b1", "character_card", "e1", 10)
            .await
            .unwrap()
            .is_none());
        // e2 should survive
        assert!(repo
            .get_cached("b1", "character_card", "e2", 10)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn invalidate_all_clears_book() {
        let (_pool, repo) = setup().await;

        repo.set_cached("b1", "character_card", "e1", 10, r#"{}"#)
            .await
            .unwrap();
        repo.set_cached("b1", "character_list", "__book__", 10, r#"{}"#)
            .await
            .unwrap();
        repo.set_cached("b2", "character_card", "e1", 10, r#"{}"#)
            .await
            .unwrap();

        repo.invalidate_all("b1").await.unwrap();

        assert!(repo
            .get_cached("b1", "character_card", "e1", 10)
            .await
            .unwrap()
            .is_none());
        assert!(repo
            .get_cached("b1", "character_list", "__book__", 10)
            .await
            .unwrap()
            .is_none());
        // b2 should survive
        assert!(repo
            .get_cached("b2", "character_card", "e1", 10)
            .await
            .unwrap()
            .is_some());
    }
}
