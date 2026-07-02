use crate::storage::db::v4::cache_repo::CacheRepo;
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::property_repo::PropertyRepo;
use sqlx::SqlitePool;
use std::collections::HashMap;

/// Character card view model for a single entity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CharacterCardView {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub summary: Option<String>,
    pub importance: f64,
    pub first_seen_chapter: i64,
    pub last_seen_chapter: i64,
    pub current_states: HashMap<String, DimensionState>,
}

/// State of a single dimension for a character.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DimensionState {
    pub label: String,
    pub value: String,
    pub updated_chapter: i64,
    pub confidence: f64,
}

/// Character list item for book-level view.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CharacterListItem {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub summary: Option<String>,
    pub importance: f64,
    pub first_seen_chapter: i64,
    pub last_seen_chapter: i64,
    pub visibility_score: f64,
}

/// Project a character card for a single entity.
///
/// If cache is valid, return from cache. Otherwise, project from DB and cache result.
pub async fn project_character_card(
    entity_id: &str,
    book_id: &str,
    max_chapter: i64,
    pool: &SqlitePool,
) -> anyhow::Result<CharacterCardView> {
    let cache_repo = CacheRepo::new(pool.clone());

    // Try cache first
    if let Some(cached) = cache_repo
        .get_cached(book_id, "character_card", entity_id, max_chapter)
        .await?
    {
        if let Ok(view) = serde_json::from_str::<CharacterCardView>(&cached) {
            return Ok(view);
        }
    }

    // Cache miss - project from DB
    let view = project_character_card_from_db(entity_id, book_id, pool).await?;

    // Cache the result
    let payload = serde_json::to_string(&view)?;
    cache_repo
        .set_cached(book_id, "character_card", entity_id, max_chapter, &payload)
        .await?;

    Ok(view)
}

/// Project a character card directly from DB (no cache).
async fn project_character_card_from_db(
    entity_id: &str,
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<CharacterCardView> {
    let entity_repo = EntityRepo::new(pool.clone());
    let property_repo = PropertyRepo::new(pool.clone());

    // Get entity
    let entity = entity_repo
        .get_by_id(entity_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Entity not found: {}", entity_id))?;

    // Get aliases
    let aliases = entity_repo.list_aliases_by_entity(entity_id).await?;
    let alias_names: Vec<String> = aliases.iter().map(|a| a.alias.clone()).collect();

    // Get current properties
    let properties = property_repo
        .list_current_properties(book_id, entity_id)
        .await?;
    let mut current_states = HashMap::new();
    for prop in properties {
        // Resolve dimension display_name
        let dimension = property_repo
            .get_dimension(book_id, "character", &prop.dimension_key)
            .await?;
        let label = dimension
            .as_ref()
            .map(|d| d.display_name.clone())
            .unwrap_or_else(|| prop.dimension_key.clone());

        current_states.insert(
            prop.dimension_key.clone(),
            DimensionState {
                label,
                value: prop.value_text.unwrap_or_default(),
                updated_chapter: prop.updated_chapter,
                confidence: prop.confidence,
            },
        );
    }

    Ok(CharacterCardView {
        id: entity.id,
        name: entity.display_name,
        aliases: alias_names,
        summary: entity.short_summary,
        importance: entity.importance_score,
        first_seen_chapter: entity.first_seen_chapter,
        last_seen_chapter: entity.last_seen_chapter,
        current_states,
    })
}

/// Project character list for a book.
///
/// Returns all active characters sorted by importance.
pub async fn project_character_list(
    book_id: &str,
    max_chapter: i64,
    pool: &SqlitePool,
) -> anyhow::Result<Vec<CharacterListItem>> {
    let cache_repo = CacheRepo::new(pool.clone());

    // Try cache first
    if let Some(cached) = cache_repo
        .get_cached(book_id, "character_list", "__book__", max_chapter)
        .await?
    {
        if let Ok(list) = serde_json::from_str::<Vec<CharacterListItem>>(&cached) {
            return Ok(list);
        }
    }

    // Cache miss - project from DB
    let list = project_character_list_from_db(book_id, pool).await?;

    // Cache the result
    let payload = serde_json::to_string(&list)?;
    cache_repo
        .set_cached(book_id, "character_list", "__book__", max_chapter, &payload)
        .await?;

    Ok(list)
}

/// Project character list directly from DB (no cache).
async fn project_character_list_from_db(
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<Vec<CharacterListItem>> {
    let entity_repo = EntityRepo::new(pool.clone());

    let entities = entity_repo.list_by_book(book_id).await?;
    let mut items = Vec::new();

    for entity in entities {
        let aliases = entity_repo.list_aliases_by_entity(&entity.id).await?;
        let alias_names: Vec<String> = aliases.iter().map(|a| a.alias.clone()).collect();

        items.push(CharacterListItem {
            id: entity.id,
            name: entity.display_name,
            aliases: alias_names,
            summary: entity.short_summary,
            importance: entity.importance_score,
            first_seen_chapter: entity.first_seen_chapter,
            last_seen_chapter: entity.last_seen_chapter,
            visibility_score: entity.importance_score, // Simple heuristic for now
        });
    }

    Ok(items)
}

/// Invalidate cache for a specific entity.
pub async fn invalidate_entity_cache(
    entity_id: &str,
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<()> {
    let cache_repo = CacheRepo::new(pool.clone());
    cache_repo
        .invalidate(book_id, "character_card", entity_id)
        .await?;
    // Also invalidate book-level list since entity data changed
    cache_repo
        .invalidate(book_id, "character_list", "__book__")
        .await?;
    Ok(())
}

/// Invalidate all cache for a book (e.g., after reset).
pub async fn invalidate_book_cache(book_id: &str, pool: &SqlitePool) -> anyhow::Result<()> {
    let cache_repo = CacheRepo::new(pool.clone());
    cache_repo.invalidate_all(book_id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup() -> (SqlitePool, EntityRepo, PropertyRepo) {
        let dir =
            std::env::temp_dir().join(format!("reader-v4-projection-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();

        // Insert chapter for FK
        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 1, 'text', 'hash', datetime('now'))")
            .bind(&chapter_id).execute(&pool).await.unwrap();

        // Insert source span for FK
        let span_id = uuid::Uuid::new_v4().to_string();
        let seg_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'hash', 0, datetime('now'))")
            .bind(&seg_id).bind(&chapter_id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 0, 0, 10, 'test', datetime('now'))")
            .bind(&span_id).bind(&chapter_id).bind(&seg_id).execute(&pool).await.unwrap();

        // Insert ai_run for FK
        let run_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, 'b1', ?, 'extract', 'gpt-4', 'v1', 1, 'hash', 'running', datetime('now'))")
            .bind(&run_id).bind(&chapter_id).execute(&pool).await.unwrap();

        let entity_repo = EntityRepo::new(pool.clone());
        let property_repo = PropertyRepo::new(pool.clone());

        (pool, entity_repo, property_repo)
    }

    #[tokio::test]
    async fn project_character_card_basic() {
        let (pool, entity_repo, property_repo) = setup().await;

        // Create entity with alias
        let entity = entity_repo
            .create_entity("b1", "character", "张三", "张三", Some("主角"), 0.9, 1)
            .await
            .unwrap();
        entity_repo
            .create_alias("b1", &entity.id, "小张", "nickname", 1, 0.8, None)
            .await
            .unwrap();

        // Create property
        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let claim_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', 1, 'property_update', 'test', ?, ?, 0.9, 'low', 'accepted', datetime('now'), datetime('now'))")
            .bind(&claim_id).bind(&span_id.0).bind(&run_id.0).execute(&pool).await.unwrap();

        property_repo
            .apply_replace(
                "b1",
                &entity.id,
                "realm",
                Some("筑基期"),
                None,
                1,
                &claim_id,
                0.9,
            )
            .await
            .unwrap();

        // Project
        let card = project_character_card_from_db(&entity.id, "b1", &pool)
            .await
            .unwrap();

        assert_eq!(card.name, "张三");
        assert_eq!(card.aliases, vec!["小张"]);
        assert_eq!(card.summary.as_deref(), Some("主角"));
        assert!(card.current_states.contains_key("realm"));
        assert_eq!(card.current_states["realm"].value, "筑基期");
    }

    #[tokio::test]
    async fn project_character_list_basic() {
        let (pool, entity_repo, _property_repo) = setup().await;

        // Create entities
        entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        entity_repo
            .create_entity("b1", "character", "李四", "李四", None, 0.7, 2)
            .await
            .unwrap();

        let list = project_character_list_from_db("b1", &pool).await.unwrap();

        assert_eq!(list.len(), 2);
        // Sorted by importance DESC
        assert_eq!(list[0].name, "张三");
        assert_eq!(list[1].name, "李四");
    }

    #[tokio::test]
    async fn cache_round_trip() {
        let (pool, entity_repo, _property_repo) = setup().await;

        // Create entity
        let entity = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        // First call - cache miss
        let card1 = project_character_card(&entity.id, "b1", 10, &pool)
            .await
            .unwrap();
        assert_eq!(card1.name, "张三");

        // Second call - should hit cache
        let card2 = project_character_card(&entity.id, "b1", 10, &pool)
            .await
            .unwrap();
        assert_eq!(card2.name, "张三");

        // Verify cache exists
        let cache_repo = CacheRepo::new(pool.clone());
        let cached = cache_repo
            .get_cached("b1", "character_card", &entity.id, 10)
            .await
            .unwrap();
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn invalidate_entity_cache() {
        let (pool, entity_repo, _property_repo) = setup().await;

        // Create entity
        let entity = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        // Populate cache
        project_character_card(&entity.id, "b1", 10, &pool)
            .await
            .unwrap();

        // Verify cached
        let cache_repo = CacheRepo::new(pool.clone());
        assert!(cache_repo
            .get_cached("b1", "character_card", &entity.id, 10)
            .await
            .unwrap()
            .is_some());

        // Invalidate
        super::invalidate_entity_cache(&entity.id, "b1", &pool)
            .await
            .unwrap();

        // Verify invalidated
        assert!(cache_repo
            .get_cached("b1", "character_card", &entity.id, 10)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn invalidate_book_cache() {
        let (pool, entity_repo, _property_repo) = setup().await;

        // Create entities and cache
        let e1 = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        let e2 = entity_repo
            .create_entity("b1", "character", "李四", "李四", None, 0.7, 2)
            .await
            .unwrap();

        project_character_card(&e1.id, "b1", 10, &pool)
            .await
            .unwrap();
        project_character_card(&e2.id, "b1", 10, &pool)
            .await
            .unwrap();
        project_character_list("b1", 10, &pool).await.unwrap();

        // Verify cached
        let cache_repo = CacheRepo::new(pool.clone());
        assert!(cache_repo
            .get_cached("b1", "character_card", &e1.id, 10)
            .await
            .unwrap()
            .is_some());

        // Invalidate all
        super::invalidate_book_cache("b1", &pool).await.unwrap();

        // Verify all invalidated
        assert!(cache_repo
            .get_cached("b1", "character_card", &e1.id, 10)
            .await
            .unwrap()
            .is_none());
        assert!(cache_repo
            .get_cached("b1", "character_card", &e2.id, 10)
            .await
            .unwrap()
            .is_none());
        assert!(cache_repo
            .get_cached("b1", "character_list", "__book__", 10)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn cache_miss_fallback() {
        let (pool, entity_repo, _property_repo) = setup().await;

        // Create entity but don't cache
        let entity = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        // Direct projection (simulating cache miss fallback)
        let card = project_character_card_from_db(&entity.id, "b1", &pool)
            .await
            .unwrap();
        assert_eq!(card.name, "张三");
    }
}
