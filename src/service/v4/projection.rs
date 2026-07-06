use crate::storage::db::v4::cache_repo::CacheRepo;
use crate::storage::db::v4::entity_repo::{EntityRecord, EntityRepo};
use crate::storage::db::v4::identity_repo::IdentityRepo;
use crate::storage::db::v4::property_repo::PropertyRepo;
use sqlx::SqlitePool;
use std::collections::HashMap;

/// Character card view model for a single entity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct DimensionState {
    pub label: String,
    pub value: String,
    pub updated_chapter: i64,
    pub confidence: f64,
}

/// Character list item for book-level view.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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

const GENERIC_CHARACTER_LABELS: &[&str] = &[
    "男人",
    "女人",
    "男孩",
    "女孩",
    "少年",
    "少女",
    "老人",
    "妇人",
    "孩子",
    "青年",
    "中年男子",
    "中年女人",
];

const GENERIC_CHARACTER_SUFFIXES: &[&str] = &["男子", "女人", "男孩", "女孩", "青年", "老人", "妇人"];

fn normalize_unit_score(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    let normalized = if value > 1.0 && value.fract().abs() < f64::EPSILON {
        if value <= 10.0 {
            value / 10.0
        } else if value <= 100.0 {
            value / 100.0
        } else {
            1.0
        }
    } else {
        value
    };
    normalized.clamp(0.0, 1.0)
}

fn looks_generic_character_label(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return true;
    }
    GENERIC_CHARACTER_LABELS.contains(&trimmed)
        || GENERIC_CHARACTER_SUFFIXES
            .iter()
            .any(|suffix| trimmed.ends_with(suffix))
        || trimmed.starts_with("一位")
}

fn alias_display_rank(alias: &str) -> (i32, usize, usize) {
    let trimmed = alias.trim();
    let generic_penalty = if looks_generic_character_label(trimmed) { 1 } else { 0 };
    let honorific_penalty =
        if trimmed.ends_with("叔叔") || trimmed.ends_with("先生") || trimmed.ends_with("大人") {
            1
        } else {
            0
        };
    (generic_penalty, honorific_penalty, trimmed.chars().count())
}

fn preferred_character_name(
    display_name: &str,
    canonical_name: &str,
    aliases: &[String],
) -> String {
    if !looks_generic_character_label(display_name) {
        return display_name.to_string();
    }

    let mut candidates: Vec<&str> = aliases
        .iter()
        .map(String::as_str)
        .filter(|alias| {
            let trimmed = alias.trim();
            !trimmed.is_empty() && trimmed != display_name.trim() && trimmed != canonical_name.trim()
        })
        .collect();
    candidates.sort_by_key(|alias| alias_display_rank(alias));

    candidates
        .into_iter()
        .next()
        .unwrap_or(display_name)
        .to_string()
}

fn compute_visibility_score(entity: &EntityRecord, max_last_seen_chapter: i64) -> f64 {
    let normalized_importance = normalize_unit_score(entity.importance_score);
    let chapter_window = (max_last_seen_chapter.max(entity.last_seen_chapter) + 1).max(1) as f64;
    let span = (entity.last_seen_chapter - entity.first_seen_chapter + 1).max(1) as f64;
    let span_score = (span + 1.0).ln() / (chapter_window + 1.0).ln();
    let recency_score = if chapter_window <= 1.0 {
        1.0
    } else {
        (entity.last_seen_chapter.max(0) as f64 / (chapter_window - 1.0)).clamp(0.0, 1.0)
    };

    (0.6 * span_score + 0.25 * recency_score + 0.15 * normalized_importance).clamp(0.0, 1.0)
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
    let identity_repo = IdentityRepo::new(pool.clone());
    let resolved_entity_id = identity_repo
        .resolve_redirect_target(book_id, entity_id)
        .await?
        .unwrap_or_else(|| entity_id.to_string());

    // Try cache first
    if let Some(cached) = cache_repo
        .get_cached(book_id, "character_card", &resolved_entity_id, max_chapter)
        .await?
    {
        if let Ok(view) = serde_json::from_str::<CharacterCardView>(&cached) {
            return Ok(view);
        }
    }

    // Cache miss - project from DB
    let view = project_character_card_from_db(&resolved_entity_id, book_id, pool).await?;

    // Cache the result
    let payload = serde_json::to_string(&view)?;
    cache_repo
        .set_cached(
            book_id,
            "character_card",
            &resolved_entity_id,
            max_chapter,
            &payload,
        )
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
    let display_name =
        preferred_character_name(&entity.display_name, &entity.canonical_name, &alias_names);

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
        name: display_name,
        aliases: alias_names,
        summary: entity.short_summary,
        importance: normalize_unit_score(entity.importance_score),
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
    let identity_repo = IdentityRepo::new(pool.clone());

    let entities = entity_repo.list_by_book(book_id).await?;
    let mut items = Vec::new();
    let max_last_seen_chapter = entities
        .iter()
        .filter(|entity| entity.entity_type == "character")
        .map(|entity| entity.last_seen_chapter)
        .max()
        .unwrap_or(0);

    for entity in entities {
        if entity.entity_type != "character" {
            continue;
        }
        if identity_repo
            .resolve_redirect_target(book_id, &entity.id)
            .await?
            .is_some()
        {
            continue;
        }
        let aliases = entity_repo.list_aliases_by_entity(&entity.id).await?;
        let alias_names: Vec<String> = aliases.iter().map(|a| a.alias.clone()).collect();
        let display_name =
            preferred_character_name(&entity.display_name, &entity.canonical_name, &alias_names);
        let visibility_score = compute_visibility_score(&entity, max_last_seen_chapter);

        items.push(CharacterListItem {
            id: entity.id,
            name: display_name,
            aliases: alias_names,
            summary: entity.short_summary,
            importance: normalize_unit_score(entity.importance_score),
            first_seen_chapter: entity.first_seen_chapter,
            last_seen_chapter: entity.last_seen_chapter,
            visibility_score,
        });
    }

    items.sort_by(|left, right| {
        right
            .visibility_score
            .partial_cmp(&left.visibility_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.last_seen_chapter.cmp(&left.last_seen_chapter))
            .then_with(|| left.first_seen_chapter.cmp(&right.first_seen_chapter))
            .then_with(|| left.name.cmp(&right.name))
    });

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

    #[test]
    fn character_projection_serializes_public_api_fields_as_camel_case() {
        let mut current_states = HashMap::new();
        current_states.insert(
            "realm".to_string(),
            DimensionState {
                label: "境界".to_string(),
                value: "筑基期".to_string(),
                updated_chapter: 3,
                confidence: 0.91,
            },
        );
        let card = CharacterCardView {
            id: "char-1".to_string(),
            name: "张三".to_string(),
            aliases: vec!["小张".to_string()],
            summary: Some("主角".to_string()),
            importance: 0.9,
            first_seen_chapter: 1,
            last_seen_chapter: 5,
            current_states,
        };
        let value = serde_json::to_value(card).unwrap();

        assert!(value.get("firstSeenChapter").is_some());
        assert!(value.get("lastSeenChapter").is_some());
        assert!(value.get("currentStates").is_some());
        assert!(value.get("first_seen_chapter").is_none());
        assert!(value.get("current_states").is_none());
        assert!(value["currentStates"]["realm"].get("updatedChapter").is_some());
        assert!(value["currentStates"]["realm"].get("updated_chapter").is_none());

        let item = CharacterListItem {
            id: "char-1".to_string(),
            name: "张三".to_string(),
            aliases: vec![],
            summary: None,
            importance: 0.9,
            first_seen_chapter: 1,
            last_seen_chapter: 5,
            visibility_score: 0.9,
        };
        let item_value = serde_json::to_value(item).unwrap();
        assert!(item_value.get("firstSeenChapter").is_some());
        assert!(item_value.get("lastSeenChapter").is_some());
        assert!(item_value.get("visibilityScore").is_some());
        assert!(item_value.get("first_seen_chapter").is_none());
    }

    #[tokio::test]
    async fn project_character_list_basic() {
        let (pool, entity_repo, _property_repo) = setup().await;

        // Create entities
        let side = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.99, 8)
            .await
            .unwrap();
        let lead = entity_repo
            .create_entity("b1", "character", "李四", "李四", None, 0.7, 1)
            .await
            .unwrap();
        entity_repo.update_last_seen(&lead.id, 18).await.unwrap();
        entity_repo.update_last_seen(&side.id, 8).await.unwrap();
        entity_repo
            .create_entity("b1", "place", "阿尔托", "阿尔托", None, 0.95, 1)
            .await
            .unwrap();
        entity_repo
            .create_entity("b1", "organization", "神圣海尔兹帝国", "神圣海尔兹帝国", None, 0.8, 1)
            .await
            .unwrap();

        let list = project_character_list_from_db("b1", &pool).await.unwrap();

        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "李四");
        assert_eq!(list[1].name, "张三");
        assert!(list[0].visibility_score > list[1].visibility_score);
    }

    #[tokio::test]
    async fn project_character_list_prefers_specific_alias_for_generic_display_name() {
        let (pool, entity_repo, _property_repo) = setup().await;

        let entity = entity_repo
            .create_entity("b1", "character", "中年男子", "中年男子", None, 0.91, 1)
            .await
            .unwrap();
        entity_repo
            .create_alias("b1", &entity.id, "乔尔", "ai_extracted", 1, 0.91, None)
            .await
            .unwrap();
        entity_repo
            .create_alias("b1", &entity.id, "乔尔叔叔", "ai_extracted", 7, 0.96, None)
            .await
            .unwrap();

        let list = project_character_list_from_db("b1", &pool).await.unwrap();
        assert_eq!(list[0].name, "乔尔");

        let card = project_character_card_from_db(&entity.id, "b1", &pool)
            .await
            .unwrap();
        assert_eq!(card.name, "乔尔");
    }

    #[tokio::test]
    async fn project_character_card_resolves_active_redirect_to_survivor() {
        let (pool, entity_repo, _property_repo) = setup().await;

        let victim = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.6, 2)
            .await
            .unwrap();
        let survivor = entity_repo
            .create_entity("b1", "character", "张三", "张三", Some("真身"), 0.9, 1)
            .await
            .unwrap();
        sqlx::query("UPDATE entities SET status = 'merged' WHERE id = ?")
            .bind(&victim.id)
            .execute(&pool)
            .await
            .unwrap();
        let source_claim_id = uuid::Uuid::new_v4().to_string();
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
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', 2, 'identity_reveal', '黑衣人 is 张三', ?, ?, 0.96, 'high', 'accepted', datetime('now'), datetime('now'))")
            .bind(&source_claim_id)
            .bind(&span_id.0)
            .bind(&run_id.0)
            .execute(&pool)
            .await
            .unwrap();
        crate::storage::db::v4::identity_repo::IdentityRepo::new(pool.clone())
            .create_identity_link(
                "b1",
                &victim.id,
                &survivor.id,
                "redirect",
                0.96,
                &source_claim_id,
                "active",
            )
            .await
            .unwrap();

        let card = project_character_card(&victim.id, "b1", 10, &pool)
            .await
            .unwrap();

        assert_eq!(card.id, survivor.id);
        assert_eq!(card.name, "张三");
        assert_eq!(card.summary.as_deref(), Some("真身"));
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
