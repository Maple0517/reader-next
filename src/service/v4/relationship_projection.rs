use crate::storage::db::v4::cache_repo::CacheRepo;
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::relationship_repo::{
    RelationshipEventRepo, RelationshipRecord, RelationshipRepo,
};
use sqlx::SqlitePool;
use std::collections::HashSet;

const RELATIONSHIP_GRAPH_CACHE_TYPE: &str = "relationship_graph";
const RELATIONSHIP_LIST_CACHE_TYPE: &str = "relationship_list";
const BOOK_SCOPE_ID: &str = "__book__";

// --- View Models ---

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipGraphView {
    pub nodes: Vec<RelationshipNode>,
    pub edges: Vec<RelationshipEdge>,
    pub groups: Vec<String>,
    pub total: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipNode {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub importance: f64,
    pub first_seen_chapter: i64,
    pub last_seen_chapter: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub group: String,
    pub label: String,
    pub directionality: String,
    pub current_state: Option<String>,
    pub strength: f64,
    pub polarity: String,
    pub confidence: f64,
    pub importance_score: f64,
    pub first_seen_chapter: i64,
    pub last_changed_chapter: i64,
    pub last_seen_chapter: i64,
    pub event_count: usize,
    pub latest_source_claim_id: String,
    pub evidence_available: bool,
}

// --- Projection Functions ---

/// Project relationship graph view with nodes, edges, groups.
/// Only includes status='active' relationships.
pub async fn project_relationship_graph(
    book_id: &str,
    max_chapter: i64,
    pool: &SqlitePool,
) -> anyhow::Result<RelationshipGraphView> {
    let cache_repo = CacheRepo::new(pool.clone());

    // Try cache first
    if let Some(cached) = cache_repo
        .get_cached(
            book_id,
            RELATIONSHIP_GRAPH_CACHE_TYPE,
            BOOK_SCOPE_ID,
            max_chapter,
        )
        .await?
    {
        if let Ok(view) = serde_json::from_str::<RelationshipGraphView>(&cached) {
            return Ok(view);
        }
    }

    // Cache miss - project from DB
    let view = project_relationship_graph_from_db(book_id, pool).await?;

    // Cache the result
    let payload = serde_json::to_string(&view)?;
    cache_repo
        .set_cached(
            book_id,
            RELATIONSHIP_GRAPH_CACHE_TYPE,
            BOOK_SCOPE_ID,
            max_chapter,
            &payload,
        )
        .await?;

    Ok(view)
}

/// Project relationship list view (edges only, for list display).
pub async fn project_relationship_list(
    book_id: &str,
    max_chapter: i64,
    pool: &SqlitePool,
) -> anyhow::Result<Vec<RelationshipEdge>> {
    let cache_repo = CacheRepo::new(pool.clone());

    // Try cache first
    if let Some(cached) = cache_repo
        .get_cached(
            book_id,
            RELATIONSHIP_LIST_CACHE_TYPE,
            BOOK_SCOPE_ID,
            max_chapter,
        )
        .await?
    {
        if let Ok(list) = serde_json::from_str::<Vec<RelationshipEdge>>(&cached) {
            return Ok(list);
        }
    }

    // Cache miss - project from DB
    let view = project_relationship_graph_from_db(book_id, pool).await?;

    // Cache just the edges for list view
    let payload = serde_json::to_string(&view.edges)?;
    cache_repo
        .set_cached(
            book_id,
            RELATIONSHIP_LIST_CACHE_TYPE,
            BOOK_SCOPE_ID,
            max_chapter,
            &payload,
        )
        .await?;

    Ok(view.edges)
}

/// Build `RelationshipEdge` for a relationship record, fetching event count and latest claim ID.
pub async fn build_edge_for_relationship(
    rel: &RelationshipRecord,
    event_repo: &RelationshipEventRepo,
) -> anyhow::Result<RelationshipEdge> {
    let events = event_repo.list_by_relationship(&rel.id).await?;
    let event_count = events.len();
    let latest_source_claim_id = events
        .last()
        .map(|e| e.source_claim_id.clone())
        .unwrap_or_default();

    Ok(RelationshipEdge {
        id: rel.id.clone(),
        source_id: rel.subject_character_id.clone(),
        target_id: rel.object_character_id.clone(),
        group: rel.relation_group.clone(),
        label: rel.relation_label.clone(),
        directionality: rel.directionality.clone(),
        current_state: rel.current_state.clone(),
        strength: rel.strength,
        polarity: rel.polarity.clone(),
        confidence: rel.confidence,
        importance_score: rel.importance_score,
        first_seen_chapter: rel.first_seen_chapter,
        last_changed_chapter: rel.last_changed_chapter,
        last_seen_chapter: rel.last_seen_chapter,
        event_count,
        latest_source_claim_id,
        evidence_available: event_count > 0,
    })
}

/// Invalidate relationship cache entries for a book.
pub async fn invalidate_relationship_cache(
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<()> {
    let cache_repo = CacheRepo::new(pool.clone());
    cache_repo
        .invalidate(book_id, RELATIONSHIP_GRAPH_CACHE_TYPE, BOOK_SCOPE_ID)
        .await?;
    cache_repo
        .invalidate(book_id, RELATIONSHIP_LIST_CACHE_TYPE, BOOK_SCOPE_ID)
        .await?;
    Ok(())
}

// --- Internal ---

async fn project_relationship_graph_from_db(
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<RelationshipGraphView> {
    let rel_repo = RelationshipRepo::new(pool.clone());
    let entity_repo = EntityRepo::new(pool.clone());
    let event_repo = RelationshipEventRepo::new(pool.clone());

    // Only active relationships
    let relationships = rel_repo.list_active_by_book(book_id).await?;

    // Collect unique entity IDs
    let mut entity_ids: HashSet<String> = HashSet::new();
    for rel in &relationships {
        entity_ids.insert(rel.subject_character_id.clone());
        entity_ids.insert(rel.object_character_id.clone());
    }

    // Fetch entity info + aliases
    let mut nodes = Vec::new();
    for eid in &entity_ids {
        if let Some(entity) = entity_repo.get_by_id(eid).await? {
            let aliases = entity_repo.list_aliases_by_entity(eid).await?;
            let alias_names: Vec<String> = aliases.iter().map(|a| a.alias.clone()).collect();
            nodes.push(RelationshipNode {
                id: entity.id,
                name: entity.display_name,
                aliases: alias_names,
                importance: entity.importance_score,
                first_seen_chapter: entity.first_seen_chapter,
                last_seen_chapter: entity.last_seen_chapter,
            });
        }
    }

    // Build edges with event info
    let mut edges = Vec::new();
    let mut groups: HashSet<String> = HashSet::new();
    for rel in &relationships {
        groups.insert(rel.relation_group.clone());
        let edge = build_edge_for_relationship(rel, &event_repo).await?;
        edges.push(edge);
    }

    let mut groups_vec: Vec<String> = groups.into_iter().collect();
    groups_vec.sort();

    Ok(RelationshipGraphView {
        nodes,
        edges,
        groups: groups_vec,
        total: relationships.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup() -> SqlitePool {
        let dir =
            std::env::temp_dir().join(format!("reader-rel-proj-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        db::v4::init_v4(&pool).await.unwrap();
        pool
    }

    /// Seed entities, chapter/span/ai_run/claim chain, and return (e1, e2, claim_id)
    async fn seed_full(pool: &SqlitePool, chapter: i64) -> (String, String, String) {
        let entity_repo = EntityRepo::new(pool.clone());
        let e1 = entity_repo
            .create_entity("b1", "character", "Alice", "Alice", Some("protagonist"), 0.9, 1)
            .await
            .unwrap();
        entity_repo
            .create_alias("b1", &e1.id, "Ali", "nickname", 1, 0.8, None)
            .await
            .unwrap();
        let e2 = entity_repo
            .create_entity("b1", "character", "Bob", "Bob", None, 0.7, 1)
            .await
            .unwrap();

        // FK chain: chapter -> segment -> span -> ai_run -> claim
        let ch_id = uuid::Uuid::new_v4().to_string();
        let seg_id = uuid::Uuid::new_v4().to_string();
        let span_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let claim_id = uuid::Uuid::new_v4().to_string();

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', ?, 'text', 'hash', datetime('now'))")
            .bind(&ch_id).bind(chapter).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'hash', 0, datetime('now'))")
            .bind(&seg_id).bind(&ch_id).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 0, 0, 10, 'text', datetime('now'))")
            .bind(&span_id).bind(&ch_id).bind(&seg_id).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, 'b1', ?, 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
            .bind(&run_id).bind(&ch_id).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', ?, 'relationship_update', 'test', ?, ?, 0.9, 'low', 'accepted', datetime('now'), datetime('now'))")
            .bind(&claim_id).bind(chapter).bind(&span_id).bind(&run_id).execute(pool).await.unwrap();

        (e1.id, e2.id, claim_id)
    }

    #[tokio::test]
    async fn project_relationship_graph_returns_correct_nodes_and_edges() {
        let pool = setup().await;
        let (e1, e2, claim_id) = seed_full(&pool, 1).await;

        let rel_repo = RelationshipRepo::new(pool.clone());
        let ev_repo = RelationshipEventRepo::new(pool.clone());

        // Create relationship
        let rel = rel_repo
            .create_relationship(
                "b1", &e1, &e2, "friendship", "friends", "undirected",
                None, 0.5, "positive", 0.8, 0.7, 1,
            )
            .await
            .unwrap();

        // Create event
        ev_repo
            .create_event(
                "b1", &rel.id, "established", "friendship", "friends",
                None, Some(0.8), Some("positive"), 1, &claim_id, 0.9,
            )
            .await
            .unwrap();

        let graph = project_relationship_graph_from_db("b1", &pool).await.unwrap();

        // Should have 2 nodes (Alice and Bob)
        assert_eq!(graph.nodes.len(), 2);
        let node_names: Vec<&str> = graph.nodes.iter().map(|n| n.name.as_str()).collect();
        assert!(node_names.contains(&"Alice"));
        assert!(node_names.contains(&"Bob"));

        // Alice should have aliases
        let alice_node = graph.nodes.iter().find(|n| n.name == "Alice").unwrap();
        assert_eq!(alice_node.aliases, vec!["Ali"]);

        // Should have 1 edge
        assert_eq!(graph.edges.len(), 1);
        let edge = &graph.edges[0];
        assert_eq!(edge.group, "friendship");
        assert_eq!(edge.label, "friends");
        assert_eq!(edge.event_count, 1);
        assert_eq!(edge.latest_source_claim_id, claim_id);
        assert!(edge.evidence_available);

        // Should have 1 group
        assert_eq!(graph.groups, vec!["friendship"]);
        assert_eq!(graph.total, 1);
    }

    #[tokio::test]
    async fn project_relationship_list_returns_edges_only() {
        let pool = setup().await;
        let (e1, e2, claim_id) = seed_full(&pool, 1).await;

        let rel_repo = RelationshipRepo::new(pool.clone());
        let ev_repo = RelationshipEventRepo::new(pool.clone());

        let rel = rel_repo
            .create_relationship(
                "b1", &e1, &e2, "mentorship", "mentor-student", "directed",
                Some("teaching"), 0.9, "positive", 0.9, 0.8, 1,
            )
            .await
            .unwrap();
        ev_repo
            .create_event(
                "b1", &rel.id, "established", "mentorship", "mentor-student",
                Some("teaching"), Some(0.9), Some("positive"), 1, &claim_id, 0.95,
            )
            .await
            .unwrap();

        let edges = project_relationship_graph_from_db("b1", &pool).await.unwrap().edges;
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].group, "mentorship");
    }

    #[tokio::test]
    async fn inactive_relationships_excluded() {
        let pool = setup().await;
        let (e1, e2, _claim_id) = seed_full(&pool, 1).await;

        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo
            .create_relationship(
                "b1", &e1, &e2, "rivalry", "rivals", "undirected",
                None, 0.5, "negative", 0.7, 0.5, 1,
            )
            .await
            .unwrap();

        // Initially active
        let graph = project_relationship_graph_from_db("b1", &pool).await.unwrap();
        assert_eq!(graph.total, 1);

        // Mark inactive
        rel_repo
            .update_relationship(&rel.id, "former rivals", None, 0.5, "neutral", 0.5, 0.5, 1, 1, "inactive")
            .await
            .unwrap();

        let graph = project_relationship_graph_from_db("b1", &pool).await.unwrap();
        assert_eq!(graph.total, 0);
        assert!(graph.edges.is_empty());
    }

    #[tokio::test]
    async fn multiple_groups_extracted_correctly() {
        let pool = setup().await;
        let entity_repo = EntityRepo::new(pool.clone());
        let rel_repo = RelationshipRepo::new(pool.clone());

        let e1 = entity_repo
            .create_entity("b1", "character", "A", "A", None, 0.5, 1)
            .await
            .unwrap();
        let e2 = entity_repo
            .create_entity("b1", "character", "B", "B", None, 0.5, 1)
            .await
            .unwrap();
        let e3 = entity_repo
            .create_entity("b1", "character", "C", "C", None, 0.5, 1)
            .await
            .unwrap();

        rel_repo
            .create_relationship("b1", &e1.id, &e2.id, "friendship", "friends", "undirected", None, 0.5, "positive", 0.8, 0.7, 1)
            .await
            .unwrap();
        rel_repo
            .create_relationship("b1", &e1.id, &e3.id, "rivalry", "rivals", "directed", None, 0.5, "negative", 0.7, 0.6, 1)
            .await
            .unwrap();

        let graph = project_relationship_graph_from_db("b1", &pool).await.unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.groups.len(), 2);
        assert!(graph.groups.contains(&"friendship".to_string()));
        assert!(graph.groups.contains(&"rivalry".to_string()));
        assert_eq!(graph.total, 2);
    }

    #[tokio::test]
    async fn cache_round_trip_graph() {
        let pool = setup().await;
        let (e1, e2, _) = seed_full(&pool, 1).await;

        let rel_repo = RelationshipRepo::new(pool.clone());
        rel_repo
            .create_relationship("b1", &e1, &e2, "family", "siblings", "undirected", None, 0.5, "positive", 0.9, 0.8, 1)
            .await
            .unwrap();

        // First call - cache miss
        let graph1 = project_relationship_graph("b1", 10, &pool).await.unwrap();
        assert_eq!(graph1.total, 1);

        // Second call - should hit cache
        let graph2 = project_relationship_graph("b1", 10, &pool).await.unwrap();
        assert_eq!(graph2.total, 1);

        // Verify cache exists
        let cache_repo = CacheRepo::new(pool.clone());
        let cached = cache_repo
            .get_cached("b1", RELATIONSHIP_GRAPH_CACHE_TYPE, BOOK_SCOPE_ID, 10)
            .await
            .unwrap();
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn cache_round_trip_list() {
        let pool = setup().await;
        let (e1, e2, _) = seed_full(&pool, 1).await;

        let rel_repo = RelationshipRepo::new(pool.clone());
        rel_repo
            .create_relationship("b1", &e1, &e2, "alliance", "allies", "undirected", None, 0.5, "positive", 0.7, 0.6, 1)
            .await
            .unwrap();

        // First call
        let list1 = project_relationship_list("b1", 10, &pool).await.unwrap();
        assert_eq!(list1.len(), 1);

        // Second call - cache hit
        let list2 = project_relationship_list("b1", 10, &pool).await.unwrap();
        assert_eq!(list2.len(), 1);

        let cache_repo = CacheRepo::new(pool.clone());
        let cached = cache_repo
            .get_cached("b1", RELATIONSHIP_LIST_CACHE_TYPE, BOOK_SCOPE_ID, 10)
            .await
            .unwrap();
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn invalidate_relationship_cache_clears_both_types() {
        let pool = setup().await;
        let (e1, e2, _) = seed_full(&pool, 1).await;

        let rel_repo = RelationshipRepo::new(pool.clone());
        rel_repo
            .create_relationship("b1", &e1, &e2, "friendship", "friends", "undirected", None, 0.5, "positive", 0.8, 0.7, 1)
            .await
            .unwrap();

        // Populate caches
        project_relationship_graph("b1", 10, &pool).await.unwrap();
        project_relationship_list("b1", 10, &pool).await.unwrap();

        let cache_repo = CacheRepo::new(pool.clone());
        assert!(cache_repo
            .get_cached("b1", RELATIONSHIP_GRAPH_CACHE_TYPE, BOOK_SCOPE_ID, 10)
            .await
            .unwrap()
            .is_some());
        assert!(cache_repo
            .get_cached("b1", RELATIONSHIP_LIST_CACHE_TYPE, BOOK_SCOPE_ID, 10)
            .await
            .unwrap()
            .is_some());

        // Invalidate
        invalidate_relationship_cache("b1", &pool).await.unwrap();

        // Both should be gone
        assert!(cache_repo
            .get_cached("b1", RELATIONSHIP_GRAPH_CACHE_TYPE, BOOK_SCOPE_ID, 10)
            .await
            .unwrap()
            .is_none());
        assert!(cache_repo
            .get_cached("b1", RELATIONSHIP_LIST_CACHE_TYPE, BOOK_SCOPE_ID, 10)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn empty_book_returns_empty_graph() {
        let pool = setup().await;

        let graph = project_relationship_graph_from_db("nonexistent-book", &pool)
            .await
            .unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
        assert!(graph.groups.is_empty());
        assert_eq!(graph.total, 0);
    }

    #[tokio::test]
    async fn edge_has_correct_event_count_and_latest_claim() {
        let pool = setup().await;
        let (e1, e2, claim_id1) = seed_full(&pool, 1).await;

        let rel_repo = RelationshipRepo::new(pool.clone());
        let ev_repo = RelationshipEventRepo::new(pool.clone());

        let rel = rel_repo
            .create_relationship(
                "b1", &e1, &e2, "friendship", "friends", "undirected",
                None, 0.5, "positive", 0.8, 0.7, 1,
            )
            .await
            .unwrap();

        // First event
        ev_repo
            .create_event(
                "b1", &rel.id, "established", "friendship", "friends",
                None, Some(0.8), Some("positive"), 1, &claim_id1, 0.9,
            )
            .await
            .unwrap();

        // Second event at a later chapter (needs new claim chain)
        let claim_id2 = {
            let ch_id = uuid::Uuid::new_v4().to_string();
            let seg_id = uuid::Uuid::new_v4().to_string();
            let span_id = uuid::Uuid::new_v4().to_string();
            let run_id = uuid::Uuid::new_v4().to_string();
            let cid = uuid::Uuid::new_v4().to_string();

            sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 2, 'text', 'hash', datetime('now'))")
                .bind(&ch_id).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'hash', 0, datetime('now'))")
                .bind(&seg_id).bind(&ch_id).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 0, 0, 10, 'text', datetime('now'))")
                .bind(&span_id).bind(&ch_id).bind(&seg_id).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, 'b1', ?, 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
                .bind(&run_id).bind(&ch_id).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', 2, 'relationship_update', 'test', ?, ?, 0.9, 'low', 'accepted', datetime('now'), datetime('now'))")
                .bind(&cid).bind(&span_id).bind(&run_id).execute(&pool).await.unwrap();
            cid
        };

        ev_repo
            .create_event(
                "b1", &rel.id, "update", "friendship", "close friends",
                Some("close"), Some(0.9), Some("positive"), 2, &claim_id2, 0.95,
            )
            .await
            .unwrap();

        let graph = project_relationship_graph_from_db("b1", &pool).await.unwrap();
        assert_eq!(graph.edges.len(), 1);

        let edge = &graph.edges[0];
        assert_eq!(edge.event_count, 2);
        // Latest claim should be the second one (ordered by chapter_index ASC)
        assert_eq!(edge.latest_source_claim_id, claim_id2);
        assert!(edge.evidence_available);
    }
}
