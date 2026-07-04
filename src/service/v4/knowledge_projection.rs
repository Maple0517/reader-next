use crate::storage::db::v4::cache_repo::CacheRepo;
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::identity_repo::IdentityRepo;
use crate::storage::db::v4::knowledge_repo::{
    KnowledgeAssertionRecord, KnowledgeCardRecord, KnowledgeRepo,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeOverviewView {
    pub cards: Vec<KnowledgeCardListItem>,
    pub categories: Vec<KnowledgeCategorySummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeCategoryView {
    pub category: String,
    pub cards: Vec<KnowledgeCardListItem>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeCardDetailView {
    pub card: KnowledgeCardListItem,
    pub assertions_by_status: HashMap<String, Vec<KnowledgeAssertionView>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeCategorySummary {
    pub category: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeCardListItem {
    pub id: String,
    pub category: String,
    pub topic_key: String,
    pub topic_display: String,
    pub current_summary: Option<String>,
    pub confidence: f64,
    pub importance_score: f64,
    pub first_seen_chapter: i64,
    pub last_updated_chapter: i64,
    pub assertion_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeAssertionView {
    pub id: String,
    pub assertion_text: String,
    pub status: String,
    pub confidence: f64,
    pub importance_score: f64,
    pub chapter_index: i64,
    pub referenced_entities: Vec<KnowledgeReferencedEntityView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeReferencedEntityView {
    pub entity_id: String,
    pub display_name: String,
    pub entity_type: String,
    pub role: String,
}

pub async fn count_active_knowledge_cards(book_id: &str, pool: &SqlitePool) -> anyhow::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM knowledge_cards WHERE book_id = ? AND status = 'active'",
    )
    .bind(book_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn project_knowledge_overview(
    book_id: &str,
    max_chapter: i64,
    pool: &SqlitePool,
) -> anyhow::Result<KnowledgeOverviewView> {
    let cache_repo = CacheRepo::new(pool.clone());
    if let Some(cached) = cache_repo
        .get_cached(book_id, "knowledge", "__book__", max_chapter)
        .await?
    {
        if let Ok(view) = serde_json::from_str::<KnowledgeOverviewView>(&cached) {
            return Ok(view);
        }
    }

    let repo = KnowledgeRepo::new(pool.clone());
    let cards = repo.list_cards(book_id, None, Some("active")).await?;
    let items = build_card_list_items(pool, &cards).await?;
    let categories = build_category_summaries(&items);
    let view = KnowledgeOverviewView {
        total: items.len(),
        cards: items,
        categories,
    };
    cache_repo
        .set_cached(
            book_id,
            "knowledge",
            "__book__",
            max_chapter,
            &serde_json::to_string(&view)?,
        )
        .await?;
    Ok(view)
}

pub async fn project_knowledge_category(
    book_id: &str,
    category: &str,
    max_chapter: i64,
    pool: &SqlitePool,
) -> anyhow::Result<KnowledgeCategoryView> {
    let cache_repo = CacheRepo::new(pool.clone());
    let scope_id = format!("category:{category}");
    if let Some(cached) = cache_repo
        .get_cached(book_id, "knowledge", &scope_id, max_chapter)
        .await?
    {
        if let Ok(view) = serde_json::from_str::<KnowledgeCategoryView>(&cached) {
            return Ok(view);
        }
    }

    let repo = KnowledgeRepo::new(pool.clone());
    let cards = repo
        .list_cards(book_id, Some(category), Some("active"))
        .await?;
    let items = build_card_list_items(pool, &cards).await?;
    let view = KnowledgeCategoryView {
        category: category.to_string(),
        total: items.len(),
        cards: items,
    };
    cache_repo
        .set_cached(
            book_id,
            "knowledge",
            &scope_id,
            max_chapter,
            &serde_json::to_string(&view)?,
        )
        .await?;
    Ok(view)
}

pub async fn project_knowledge_card_detail(
    book_id: &str,
    card_id: &str,
    max_chapter: i64,
    pool: &SqlitePool,
) -> anyhow::Result<KnowledgeCardDetailView> {
    let cache_repo = CacheRepo::new(pool.clone());
    let scope_id = format!("card:{card_id}");
    if let Some(cached) = cache_repo
        .get_cached(book_id, "knowledge", &scope_id, max_chapter)
        .await?
    {
        if let Ok(view) = serde_json::from_str::<KnowledgeCardDetailView>(&cached) {
            return Ok(view);
        }
    }

    let repo = KnowledgeRepo::new(pool.clone());
    let card = repo
        .get_card_by_id(card_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("knowledge card not found: {}", card_id))?;
    if card.book_id != book_id || card.status != "active" {
        anyhow::bail!("knowledge card not active in book");
    }

    let mut card_items = build_card_list_items(pool, std::slice::from_ref(&card)).await?;
    let card_item = card_items
        .pop()
        .ok_or_else(|| anyhow::anyhow!("knowledge card projection failed"))?;
    let assertions = repo.list_assertions_for_card(card_id).await?;
    let mut assertions_by_status: HashMap<String, Vec<KnowledgeAssertionView>> = HashMap::new();
    for assertion in assertions {
        let view = build_assertion_view(pool, &repo, book_id, assertion).await?;
        assertions_by_status
            .entry(view.status.clone())
            .or_default()
            .push(view);
    }

    let view = KnowledgeCardDetailView {
        card: card_item,
        assertions_by_status,
    };
    cache_repo
        .set_cached(
            book_id,
            "knowledge",
            &scope_id,
            max_chapter,
            &serde_json::to_string(&view)?,
        )
        .await?;
    Ok(view)
}

async fn build_card_list_items(
    pool: &SqlitePool,
    cards: &[KnowledgeCardRecord],
) -> anyhow::Result<Vec<KnowledgeCardListItem>> {
    let mut items = Vec::new();
    for card in cards {
        let assertion_count = count_assertions_for_card(pool, &card.id).await?;
        items.push(KnowledgeCardListItem {
            id: card.id.clone(),
            category: card.category.clone(),
            topic_key: card.topic_key.clone(),
            topic_display: card.topic_display.clone(),
            current_summary: card.current_summary.clone(),
            confidence: card.confidence,
            importance_score: card.importance_score,
            first_seen_chapter: card.first_seen_chapter,
            last_updated_chapter: card.last_updated_chapter,
            assertion_count,
        });
    }
    Ok(items)
}

async fn count_assertions_for_card(pool: &SqlitePool, card_id: &str) -> anyhow::Result<usize> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM knowledge_assertions WHERE card_id = ?")
        .bind(card_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0 as usize)
}

fn build_category_summaries(items: &[KnowledgeCardListItem]) -> Vec<KnowledgeCategorySummary> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for item in items {
        *counts.entry(item.category.clone()).or_default() += 1;
    }
    let mut categories = counts
        .into_iter()
        .map(|(category, count)| KnowledgeCategorySummary { category, count })
        .collect::<Vec<_>>();
    categories.sort_by(|left, right| left.category.cmp(&right.category));
    categories
}

async fn build_assertion_view(
    pool: &SqlitePool,
    repo: &KnowledgeRepo,
    book_id: &str,
    assertion: KnowledgeAssertionRecord,
) -> anyhow::Result<KnowledgeAssertionView> {
    let referenced_entities = build_referenced_entities(pool, repo, book_id, &assertion.id).await?;
    Ok(KnowledgeAssertionView {
        id: assertion.id,
        assertion_text: assertion.assertion_text,
        status: assertion.status,
        confidence: assertion.confidence,
        importance_score: assertion.importance_score,
        chapter_index: assertion.chapter_index,
        referenced_entities,
    })
}

async fn build_referenced_entities(
    pool: &SqlitePool,
    repo: &KnowledgeRepo,
    book_id: &str,
    assertion_id: &str,
) -> anyhow::Result<Vec<KnowledgeReferencedEntityView>> {
    let entity_repo = EntityRepo::new(pool.clone());
    let identity_repo = IdentityRepo::new(pool.clone());
    let refs = repo.list_entities_for_assertion(assertion_id).await?;
    let mut views = Vec::new();
    for entity_ref in refs {
        let entity_id = identity_repo
            .resolve_redirect_target(book_id, &entity_ref.entity_id)
            .await?
            .unwrap_or(entity_ref.entity_id);
        if let Some(entity) = entity_repo.get_by_id(&entity_id).await? {
            views.push(KnowledgeReferencedEntityView {
                entity_id: entity.id,
                display_name: entity.display_name,
                entity_type: entity.entity_type,
                role: entity_ref.role,
            });
        }
    }
    Ok(views)
}

#[cfg(test)]
mod tests {
    use super::{
        count_active_knowledge_cards, project_knowledge_card_detail, project_knowledge_category,
        project_knowledge_overview,
    };
    use crate::storage::db;
    use crate::storage::db::v4::cache_repo::CacheRepo;
    use crate::storage::db::v4::entity_repo::EntityRepo;
    use crate::storage::db::v4::identity_repo::IdentityRepo;
    use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
    use sqlx::SqlitePool;

    async fn setup() -> (SqlitePool, KnowledgeRepo, EntityRepo, String) {
        let dir = std::env::temp_dir().join(format!(
            "reader-knowledge-projection-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('chapter1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('segment1', 'b1', 'chapter1', 'hash', 0, datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span1', 'b1', 'chapter1', 'hash', 'segment1', 0, 0, 10, 'evidence', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'chapter1', 'extract', 'test', 'v1', 1, 'input', 'completed', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('claim1', 'b1', 1, 'knowledge_assertion', 'knowledge', 'span1', 'run1', 0.9, 'high', 'accepted', datetime('now'), datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();

        (
            pool.clone(),
            KnowledgeRepo::new(pool.clone()),
            EntityRepo::new(pool),
            "claim1".to_string(),
        )
    }

    #[tokio::test]
    async fn overview_returns_cards_categories_count_and_uses_cache() {
        let (pool, repo, _entity_repo, claim_id) = setup().await;
        repo.find_or_create_card(
            "b1",
            "power_system",
            "cultivation-realms",
            "Cultivation Realms",
            Some("Realm summary"),
            0.9,
            0.8,
            1,
        )
        .await
        .unwrap();
        repo.find_or_create_card(
            "b1",
            "history",
            "old-war",
            "Old War",
            Some("War summary"),
            0.8,
            0.7,
            1,
        )
        .await
        .unwrap();

        let overview = project_knowledge_overview("b1", 10, &pool).await.unwrap();

        assert_eq!(overview.total, 2);
        assert_eq!(overview.categories.len(), 2);
        assert_eq!(count_active_knowledge_cards("b1", &pool).await.unwrap(), 2);
        assert!(overview
            .cards
            .iter()
            .any(|card| card.topic_display == "Cultivation Realms"));
        assert!(overview
            .cards
            .iter()
            .any(|card| card.topic_display == "Old War"));

        let cached = CacheRepo::new(pool)
            .get_cached("b1", "knowledge", "__book__", 10)
            .await
            .unwrap();
        assert!(cached.is_some(), "overview should populate knowledge cache");

        drop(claim_id);
    }

    #[tokio::test]
    async fn category_projection_filters_cards() {
        let (pool, repo, _entity_repo, _claim_id) = setup().await;
        repo.find_or_create_card(
            "b1",
            "power_system",
            "cultivation-realms",
            "Cultivation Realms",
            Some("Realm summary"),
            0.9,
            0.8,
            1,
        )
        .await
        .unwrap();
        repo.find_or_create_card(
            "b1",
            "history",
            "old-war",
            "Old War",
            Some("War summary"),
            0.8,
            0.7,
            1,
        )
        .await
        .unwrap();

        let view = project_knowledge_category("b1", "history", 10, &pool)
            .await
            .unwrap();

        assert_eq!(view.category, "history");
        assert_eq!(view.total, 1);
        assert_eq!(view.cards[0].topic_display, "Old War");
    }

    #[tokio::test]
    async fn detail_groups_assertions_and_resolves_referenced_entity_redirects() {
        let (pool, repo, entity_repo, claim_id) = setup().await;
        let victim = entity_repo
            .create_entity("b1", "concept", "masked", "Masked Concept", None, 0.4, 1)
            .await
            .unwrap();
        let survivor = entity_repo
            .create_entity("b1", "concept", "truth", "Truth Concept", None, 0.9, 1)
            .await
            .unwrap();
        IdentityRepo::new(pool.clone())
            .create_identity_link(
                "b1",
                &victim.id,
                &survivor.id,
                "redirect",
                0.9,
                &claim_id,
                "active",
            )
            .await
            .unwrap();
        let card = repo
            .find_or_create_card(
                "b1",
                "secret",
                "hidden-truth",
                "Hidden Truth",
                Some("Secret summary"),
                0.9,
                0.8,
                1,
            )
            .await
            .unwrap();
        let active = repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                &claim_id,
                "The truth exists.",
                "active",
                0.9,
                0.8,
                1,
            )
            .await
            .unwrap();
        repo.find_or_create_assertion(
            "b1",
            &card.id,
            &claim_id,
            "Old rumor.",
            "rumor",
            0.6,
            0.4,
            1,
        )
        .await
        .unwrap();
        repo.insert_assertion_entity("b1", &active.id, &victim.id, "related")
            .await
            .unwrap();

        let detail = project_knowledge_card_detail("b1", &card.id, 10, &pool)
            .await
            .unwrap();

        assert_eq!(detail.card.topic_display, "Hidden Truth");
        assert_eq!(detail.assertions_by_status["active"].len(), 1);
        assert_eq!(detail.assertions_by_status["rumor"].len(), 1);
        let referenced = &detail.assertions_by_status["active"][0].referenced_entities[0];
        assert_eq!(referenced.entity_id, survivor.id);
        assert_eq!(referenced.display_name, "Truth Concept");
    }
}
