use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::place_repo::PlaceRepo;
use crate::util::hash::md5_hex;
use sqlx::SqlitePool;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedOrganizationView {
    pub id: String,
    pub name: String,
    pub link_type: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceDetailView {
    pub id: String,
    pub name: String,
    pub place_type: String,
    pub linked_organizations: Vec<LinkedOrganizationView>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapOverviewView {
    pub place_count: i64,
    pub active_edge_count: i64,
    pub conflict_count: i64,
    pub top_places: Vec<PlaceSummaryView>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSummaryView {
    pub id: String,
    pub name: String,
    pub place_type: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceHierarchyNode {
    pub place_id: String,
    pub name: String,
    pub place_type: String,
    pub children: Vec<PlaceHierarchyNode>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapGraphNode {
    pub place_id: String,
    pub label: String,
    pub place_type: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapGraphEdge {
    pub edge_id: String,
    pub from_place_id: String,
    pub to_place_id: String,
    pub edge_type: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapLayoutNode {
    pub place_id: String,
    pub x: f64,
    pub y: f64,
    pub label: String,
    pub place_type: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapLayoutView {
    pub nodes: Vec<MapLayoutNode>,
    pub edges: Vec<MapGraphEdge>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapGraphView {
    pub nodes: Vec<MapGraphNode>,
    pub edges: Vec<MapGraphEdge>,
    pub layout: Option<MapLayoutView>,
    pub warnings: Vec<String>,
}

pub async fn project_place_detail(
    book_id: &str,
    place_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<PlaceDetailView> {
    let entity_repo = EntityRepo::new(pool.clone());
    let place_repo = PlaceRepo::new(pool.clone());
    let place = entity_repo
        .get_by_id(place_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("place entity not found: {place_id}"))?;
    if place.book_id != book_id || place.entity_type != "place" {
        anyhow::bail!(
            "entity {} is {}, expected place in book {}",
            place.id,
            place.entity_type,
            book_id
        );
    }
    let detail = place_repo
        .get_place_detail(place_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("place detail not found: {place_id}"))?;
    let links = place_repo
        .list_entity_links_for_entity(book_id, place_id)
        .await?;
    let mut linked_organizations = Vec::new();
    for link in links {
        let other_id = if link.entity_a_id == place_id {
            &link.entity_b_id
        } else {
            &link.entity_a_id
        };
        let Some(entity) = entity_repo.get_by_id(other_id).await? else {
            continue;
        };
        if entity.book_id == book_id && entity.entity_type == "organization" {
            linked_organizations.push(LinkedOrganizationView {
                id: entity.id,
                name: entity.display_name,
                link_type: link.link_type,
            });
        }
    }
    linked_organizations.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));

    Ok(PlaceDetailView {
        id: place_id.to_string(),
        name: place.display_name,
        place_type: detail.place_type,
        linked_organizations,
    })
}

pub async fn project_map_overview(
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<MapOverviewView> {
    let place_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM place_details
         WHERE book_id = ? AND status = 'active'",
    )
    .bind(book_id)
    .fetch_one(pool)
    .await?;
    let active_edge_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM place_edges
         WHERE book_id = ? AND status = 'active'",
    )
    .bind(book_id)
    .fetch_one(pool)
    .await?;
    let conflict_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM place_edge_conflicts
         WHERE book_id = ? AND status = 'open'",
    )
    .bind(book_id)
    .fetch_one(pool)
    .await?;
    let top_places = sqlx::query_as::<_, PlaceSummaryRow>(
        "SELECT e.id, e.display_name AS name, d.place_type
         FROM place_details d
         JOIN entities e ON e.id = d.entity_id
         WHERE d.book_id = ? AND d.status = 'active' AND e.status = 'active'
         ORDER BY d.importance_score DESC, d.last_seen_chapter DESC, e.display_name ASC
         LIMIT 12",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(PlaceSummaryView::from)
    .collect();

    Ok(MapOverviewView {
        place_count: place_count.0,
        active_edge_count: active_edge_count.0,
        conflict_count: conflict_count.0,
        top_places,
    })
}

pub async fn project_all_places(
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<Vec<PlaceSummaryView>> {
    let places = sqlx::query_as::<_, PlaceSummaryRow>(
        "SELECT e.id, e.display_name AS name, d.place_type
         FROM place_details d
         JOIN entities e ON e.id = d.entity_id
         WHERE d.book_id = ? AND d.status = 'active' AND e.status = 'active'
         ORDER BY d.importance_score DESC, d.last_seen_chapter DESC, e.display_name ASC",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(PlaceSummaryView::from)
    .collect();

    Ok(places)
}

pub async fn project_place_hierarchy(
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<Vec<PlaceHierarchyNode>> {
    let rows = load_place_rows(book_id, pool).await?;
    let mut by_parent: BTreeMap<Option<String>, Vec<FlatPlaceRow>> = BTreeMap::new();
    for row in rows {
        by_parent
            .entry(row.parent_place_id.clone())
            .or_default()
            .push(row);
    }
    for siblings in by_parent.values_mut() {
        siblings.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    }

    fn build(
        parent_id: Option<&str>,
        by_parent: &BTreeMap<Option<String>, Vec<FlatPlaceRow>>,
    ) -> Vec<PlaceHierarchyNode> {
        by_parent
            .get(&parent_id.map(str::to_string))
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|row| PlaceHierarchyNode {
                place_id: row.id.clone(),
                name: row.name,
                place_type: row.place_type,
                children: build(Some(&row.id), by_parent),
            })
            .collect()
    }

    Ok(build(None, &by_parent))
}

pub async fn project_map_graph(book_id: &str, pool: &SqlitePool) -> anyhow::Result<MapGraphView> {
    let nodes = load_place_rows(book_id, pool)
        .await?
        .into_iter()
        .map(|row| MapGraphNode {
            place_id: row.id,
            label: row.name,
            place_type: row.place_type,
        })
        .collect();
    let edges = load_active_edges(book_id, pool).await?;

    Ok(MapGraphView {
        nodes,
        edges,
        layout: None,
        warnings: Vec::new(),
    })
}

pub async fn rebuild_layout_snapshot(
    book_id: &str,
    max_chapter: i64,
    pool: &SqlitePool,
) -> anyhow::Result<MapLayoutView> {
    let graph = project_map_graph(book_id, pool).await?;
    let nodes = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| MapLayoutNode {
            place_id: node.place_id.clone(),
            x: (index as f64 % 5.0) * 180.0,
            y: (index as f64 / 5.0).floor() * 140.0,
            label: node.label.clone(),
            place_type: node.place_type.clone(),
        })
        .collect::<Vec<_>>();
    let layout = MapLayoutView {
        nodes,
        edges: graph.edges,
        warnings: Vec::new(),
    };
    let mut edge_parts = layout
        .edges
        .iter()
        .map(|edge| {
            format!(
                "{}|{}|{}|{}",
                edge.edge_id, edge.from_place_id, edge.to_place_id, edge.edge_type
            )
        })
        .collect::<Vec<_>>();
    edge_parts.sort();
    let source_edge_hash = md5_hex(&edge_parts.join("\n"));
    PlaceRepo::new(pool.clone())
        .create_layout_snapshot(
            book_id,
            max_chapter,
            "deterministic-v1",
            &serde_json::to_string(&layout)?,
            &source_edge_hash,
        )
        .await?;
    Ok(layout)
}

pub async fn project_map_graph_with_layout_failure(
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<MapGraphView> {
    let mut graph = project_map_graph(book_id, pool).await?;
    graph
        .warnings
        .push("layout unavailable: simulated layout failure".to_string());
    graph.layout = None;
    Ok(graph)
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct PlaceSummaryRow {
    id: String,
    name: String,
    place_type: String,
}

impl From<PlaceSummaryRow> for PlaceSummaryView {
    fn from(row: PlaceSummaryRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            place_type: row.place_type,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct FlatPlaceRow {
    id: String,
    name: String,
    place_type: String,
    parent_place_id: Option<String>,
}

async fn load_place_rows(book_id: &str, pool: &SqlitePool) -> anyhow::Result<Vec<FlatPlaceRow>> {
    let rows = sqlx::query_as::<_, FlatPlaceRow>(
        "SELECT e.id, e.display_name AS name, d.place_type, d.parent_place_id
         FROM place_details d
         JOIN entities e ON e.id = d.entity_id
         WHERE d.book_id = ? AND d.status = 'active' AND e.status = 'active'
         ORDER BY e.display_name ASC, e.id ASC",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct EdgeRow {
    edge_id: String,
    from_place_id: String,
    to_place_id: String,
    edge_type: String,
}

async fn load_active_edges(book_id: &str, pool: &SqlitePool) -> anyhow::Result<Vec<MapGraphEdge>> {
    let rows = sqlx::query_as::<_, EdgeRow>(
        "SELECT id AS edge_id, from_place_id, to_place_id, edge_type
         FROM place_edges
         WHERE book_id = ? AND status = 'active'
         ORDER BY first_seen_chapter ASC, id ASC",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;

    let active_place_ids = load_place_rows(book_id, pool)
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();

    Ok(rows
        .into_iter()
        .filter(|row| {
            active_place_ids.contains(&row.from_place_id)
                && active_place_ids.contains(&row.to_place_id)
        })
        .map(|row| MapGraphEdge {
            edge_id: row.edge_id,
            from_place_id: row.from_place_id,
            to_place_id: row.to_place_id,
            edge_type: row.edge_type,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::place_reducer::{
        apply_place_write, PlaceWriteAction, PlaceWriteCommand, PlaceWriteOrganizationLink,
        PlaceWritePlaceAction, PlaceWriteProvenance,
    };
    use crate::storage::db;
    use crate::storage::db::v4::claim_repo::ClaimRepo;
    use crate::storage::db::v4::entity_repo::EntityRepo;
    use crate::storage::db::v4::place_repo::PlaceRepo;
    use serde_json::json;

    async fn setup_test_db() -> (SqlitePool, ClaimRepo, EntityRepo, PlaceRepo, String, String) {
        let dir = std::env::temp_dir().join(format!(
            "reader-v4-place-projection-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();

        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 1, '青云门山门属于青云门。', 'h1', datetime('now'))")
            .bind(&chapter_id)
            .execute(&pool)
            .await
            .unwrap();

        let segment_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'h1', 0, datetime('now'))")
            .bind(&segment_id)
            .bind(&chapter_id)
            .execute(&pool)
            .await
            .unwrap();

        let span_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'h1', ?, 0, 0, 20, '青云门山门属于青云门', datetime('now'))")
            .bind(&span_id)
            .bind(&chapter_id)
            .bind(&segment_id)
            .execute(&pool)
            .await
            .unwrap();

        let run_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, 'b1', ?, 'extract', 'test-model', 'v1', 1, 'inputhash', 'success', datetime('now'))")
            .bind(&run_id)
            .bind(&chapter_id)
            .execute(&pool)
            .await
            .unwrap();

        (
            pool.clone(),
            ClaimRepo::new(pool.clone()),
            EntityRepo::new(pool.clone()),
            PlaceRepo::new(pool),
            span_id,
            run_id,
        )
    }

    async fn create_place(
        entity_repo: &EntityRepo,
        place_repo: &PlaceRepo,
        name: &str,
        place_type: &str,
        parent_place_id: Option<&str>,
        importance_score: f64,
    ) -> String {
        let entity = entity_repo
            .create_entity("b1", "place", name, name, None, importance_score, 1)
            .await
            .unwrap();
        place_repo
            .upsert_place_detail(
                "b1",
                &entity.id,
                place_type,
                parent_place_id,
                0,
                importance_score,
                true,
                1,
                1,
                "active",
            )
            .await
            .unwrap();
        entity.id
    }

    async fn create_claim(claim_repo: &ClaimRepo, span_id: &str, run_id: &str) -> String {
        claim_repo
            .create_claim(
                "b1",
                1,
                "location_edge",
                Some("青云城"),
                Some("黑风谷"),
                None,
                None,
                "location_edge",
                None,
                Some(&json!({"edge_type": "route_to"}).to_string()),
                span_id,
                run_id,
                0.8,
                "medium",
            )
            .await
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn place_projection_shows_linked_organization() {
        let (pool, claim_repo, entity_repo, _place_repo, span_id, run_id) = setup_test_db().await;
        let organization = entity_repo
            .create_entity("b1", "organization", "青云门", "青云门", None, 0.8, 1)
            .await
            .unwrap();
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "location_introduction",
                Some("青云门山门"),
                None,
                None,
                None,
                "location_introduction",
                Some("青云门山门"),
                Some(
                    &json!({
                        "place_type": "sect_site",
                        "organization_link_candidate_id": organization.id,
                        "entity_link_type": "organization_place_pair"
                    })
                    .to_string(),
                ),
                &span_id,
                &run_id,
                0.88,
                "low",
            )
            .await
            .unwrap();

        let result = apply_place_write(
            PlaceWriteCommand {
                book_id: "b1".to_string(),
                chapter_index: 1,
                provenance: PlaceWriteProvenance {
                    claim_id: claim.id.clone(),
                    evidence_span_ids: vec![span_id],
                    confidence: 0.88,
                },
                action: PlaceWriteAction::UpsertPlace(PlaceWritePlaceAction {
                    resolved_place_entity_id: None,
                    display_name: "青云门山门".to_string(),
                    place_type: "sect_site".to_string(),
                    aliases: Vec::new(),
                    parent_place_id: None,
                    organization_link: Some(PlaceWriteOrganizationLink {
                        organization_entity_id: organization.id.clone(),
                        link_type: "organization_place_pair".to_string(),
                    }),
                    scale_level: 0,
                    importance_score: 0.88,
                    map_visible: true,
                }),
            },
            &pool,
        )
        .await
        .unwrap();
        assert_eq!(result.entity_links_touched.len(), 1);
        let place_id = result.places_touched.first().unwrap();

        let detail = project_place_detail("b1", place_id, &pool).await.unwrap();

        assert_eq!(detail.name, "青云门山门");
        assert_eq!(detail.linked_organizations.len(), 1);
        assert_eq!(detail.linked_organizations[0].id, organization.id);
        assert_eq!(detail.linked_organizations[0].name, "青云门");
        assert_eq!(
            detail.linked_organizations[0].link_type,
            "organization_place_pair"
        );
    }

    #[tokio::test]
    async fn map_projection_overview_counts() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let from_id = create_place(&entity_repo, &place_repo, "青云城", "city", None, 0.9).await;
        let to_id = create_place(&entity_repo, &place_repo, "黑风谷", "dungeon", None, 0.6).await;
        let claim_id = create_claim(&claim_repo, &span_id, &run_id).await;
        place_repo
            .find_or_create_edge(
                "b1",
                &from_id,
                &to_id,
                "route_to",
                None,
                Some("三日路程"),
                0.8,
                &claim_id,
                1,
            )
            .await
            .unwrap();
        place_repo
            .insert_conflict(
                "b1",
                &claim_id,
                None,
                "duplicate_conflicting_direction",
                "opposite_direction",
                None,
            )
            .await
            .unwrap();

        let overview = project_map_overview("b1", &pool).await.unwrap();

        assert_eq!(overview.place_count, 2);
        assert_eq!(overview.active_edge_count, 1);
        assert_eq!(overview.conflict_count, 1);
        assert_eq!(overview.top_places[0].name, "青云城");
    }

    #[tokio::test]
    async fn place_hierarchy_from_parent_place_id() {
        let (pool, _claim_repo, entity_repo, place_repo, _span_id, _run_id) = setup_test_db().await;
        let parent_id = create_place(&entity_repo, &place_repo, "东域", "region", None, 0.9).await;
        let child_id = create_place(
            &entity_repo,
            &place_repo,
            "青云城",
            "city",
            Some(&parent_id),
            0.7,
        )
        .await;

        let hierarchy = project_place_hierarchy("b1", &pool).await.unwrap();

        assert_eq!(hierarchy.len(), 1);
        assert_eq!(hierarchy[0].place_id, parent_id);
        assert_eq!(hierarchy[0].children.len(), 1);
        assert_eq!(hierarchy[0].children[0].place_id, child_id);
    }

    #[tokio::test]
    async fn map_graph_hides_conflict_edges() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let from_id = create_place(&entity_repo, &place_repo, "青云城", "city", None, 0.9).await;
        let to_id = create_place(&entity_repo, &place_repo, "黑风谷", "dungeon", None, 0.6).await;
        let claim_id = create_claim(&claim_repo, &span_id, &run_id).await;
        place_repo
            .insert_conflict(
                "b1",
                &claim_id,
                None,
                "duplicate_conflicting_direction",
                "opposite_direction",
                None,
            )
            .await
            .unwrap();

        let graph = project_map_graph("b1", &pool).await.unwrap();

        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.iter().any(|node| node.place_id == from_id));
        assert!(graph.nodes.iter().any(|node| node.place_id == to_id));
        assert_eq!(graph.edges.len(), 0);
    }

    #[tokio::test]
    async fn layout_snapshot_generated_from_active_edges() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let from_id = create_place(&entity_repo, &place_repo, "青云城", "city", None, 0.9).await;
        let to_id = create_place(&entity_repo, &place_repo, "黑风谷", "dungeon", None, 0.6).await;
        let claim_id = create_claim(&claim_repo, &span_id, &run_id).await;
        let edge = place_repo
            .find_or_create_edge(
                "b1", &from_id, &to_id, "route_to", None, None, 0.8, &claim_id, 1,
            )
            .await
            .unwrap();

        let layout = rebuild_layout_snapshot("b1", 1, &pool).await.unwrap();

        assert_eq!(layout.nodes.len(), 2);
        assert_eq!(layout.edges.len(), 1);
        assert_eq!(layout.edges[0].edge_id, edge.id);
        let snapshot = place_repo
            .latest_layout_snapshot("b1", "deterministic-v1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.max_chapter, 1);
    }

    #[tokio::test]
    async fn layout_failure_still_returns_graph() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let from_id = create_place(&entity_repo, &place_repo, "青云城", "city", None, 0.9).await;
        let to_id = create_place(&entity_repo, &place_repo, "黑风谷", "dungeon", None, 0.6).await;
        let claim_id = create_claim(&claim_repo, &span_id, &run_id).await;
        place_repo
            .find_or_create_edge(
                "b1", &from_id, &to_id, "route_to", None, None, 0.8, &claim_id, 1,
            )
            .await
            .unwrap();

        let graph = project_map_graph_with_layout_failure("b1", &pool)
            .await
            .unwrap();

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert!(graph.layout.is_none());
        assert!(graph
            .warnings
            .iter()
            .any(|warning| warning.contains("layout")));
    }
}
