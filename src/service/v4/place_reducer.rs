use crate::service::v4::extractor::{VALID_PLACE_EDGE_TYPES, VALID_PLACE_TYPES};
use crate::storage::db::v4::entity_repo::{EntityRecord, EntityRepo};
use crate::storage::db::v4::place_repo::{PlaceDetailRecord, PlaceRepo};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlaceReductionResult {
    pub claims_accepted: Vec<String>,
    pub claims_rejected: Vec<String>,
    pub claims_skipped: Vec<String>,
    pub conflicts_recorded: Vec<String>,
    pub places_touched: Vec<String>,
    pub edges_touched: Vec<String>,
    pub entity_links_touched: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaceWriteProvenance {
    pub claim_id: String,
    pub evidence_span_ids: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceWriteOrganizationLink {
    pub organization_entity_id: String,
    pub link_type: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaceWritePlaceAction {
    pub resolved_place_entity_id: Option<String>,
    pub display_name: String,
    pub place_type: String,
    pub aliases: Vec<String>,
    pub parent_place_id: Option<String>,
    pub organization_link: Option<PlaceWriteOrganizationLink>,
    pub scale_level: i64,
    pub importance_score: f64,
    pub map_visible: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaceWriteEdgeAction {
    pub from_place_id: String,
    pub to_place_id: String,
    pub edge_type: String,
    pub direction_hint: Option<String>,
    pub distance_hint: Option<String>,
    pub confidence: f64,
    pub allow_parent_change: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaceWriteConflictAction {
    pub existing_edge_id: Option<String>,
    pub conflict_type: String,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaceWriteAction {
    UpsertPlace(PlaceWritePlaceAction),
    UpsertEdge(PlaceWriteEdgeAction),
    RecordConflict(PlaceWriteConflictAction),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaceWriteCommand {
    pub book_id: String,
    pub chapter_index: i64,
    pub provenance: PlaceWriteProvenance,
    pub action: PlaceWriteAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceEdgeStatusWriteCommand {
    pub book_id: String,
    pub edge_id: String,
    pub status: String,
}

pub async fn apply_place_write(
    command: PlaceWriteCommand,
    pool: &SqlitePool,
) -> anyhow::Result<PlaceReductionResult> {
    let mut result = PlaceReductionResult::default();
    let mut tx = pool.begin().await?;

    match &command.action {
        PlaceWriteAction::UpsertPlace(action) => {
            apply_place_upsert(&command, action, &mut tx, &mut result).await?;
        }
        PlaceWriteAction::UpsertEdge(action) => {
            apply_place_edge(&command, action, &mut tx, &mut result).await?;
        }
        PlaceWriteAction::RecordConflict(action) => {
            apply_place_conflict(&command, action, &mut tx, &mut result).await?;
        }
    }

    invalidate_map_cache_with_conn(&mut tx, &command.book_id).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn apply_place_edge_status_write(
    command: PlaceEdgeStatusWriteCommand,
    pool: &SqlitePool,
) -> anyhow::Result<PlaceReductionResult> {
    let mut result = PlaceReductionResult::default();
    let mut tx = pool.begin().await?;
    apply_place_edge_status_write_with_conn(&command, &mut *tx, &mut result).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn apply_place_edge_status_write_with_conn(
    command: &PlaceEdgeStatusWriteCommand,
    conn: &mut SqliteConnection,
    result: &mut PlaceReductionResult,
) -> anyhow::Result<()> {
    if command.status != "deprecated" {
        anyhow::bail!(
            "place edge status write only supports deprecated, got {}",
            command.status
        );
    }

    let update_result = sqlx::query(
        "UPDATE place_edges SET status = 'deprecated', updated_at = datetime('now') WHERE book_id = ? AND id = ?",
    )
    .bind(&command.book_id)
    .bind(&command.edge_id)
    .execute(&mut *conn)
    .await?;
    if update_result.rows_affected() == 0 {
        anyhow::bail!("place edge not found for status write");
    }
    invalidate_map_cache_with_conn(conn, &command.book_id).await?;
    result.edges_touched.push(command.edge_id.clone());
    Ok(())
}

async fn apply_place_upsert(
    command: &PlaceWriteCommand,
    action: &PlaceWritePlaceAction,
    conn: &mut SqliteConnection,
    result: &mut PlaceReductionResult,
) -> anyhow::Result<()> {
    validate_place_type(&action.place_type)?;
    let place = get_or_create_place_entity(
        conn,
        &command.book_id,
        action.resolved_place_entity_id.as_deref(),
        &action.display_name,
        &action.place_type,
        action.importance_score,
        command.chapter_index,
    )
    .await?;

    if let Some(parent_id) = action.parent_place_id.as_deref() {
        if would_create_parent_cycle(conn, &place.id, parent_id).await? {
            result
                .claims_rejected
                .push(command.provenance.claim_id.clone());
            return Ok(());
        }
        let existing_detail = PlaceRepo::get_place_detail_with_conn(conn, &place.id).await?;
        if has_conflicting_parent(existing_detail.as_ref(), parent_id) {
            result
                .claims_skipped
                .push(command.provenance.claim_id.clone());
            return Ok(());
        }
    }

    PlaceRepo::upsert_place_detail_with_conn(
        conn,
        &command.book_id,
        &place.id,
        &action.place_type,
        action.parent_place_id.as_deref(),
        action.scale_level,
        action.importance_score,
        action.map_visible,
        command.chapter_index,
        command.chapter_index,
        "active",
    )
    .await?;
    push_unique(&mut result.places_touched, place.id.clone());

    for alias in &action.aliases {
        let alias = alias.trim();
        if alias.is_empty() || alias == action.display_name {
            continue;
        }
        EntityRepo::create_alias_with_conn(
            conn,
            &command.book_id,
            &place.id,
            alias,
            "name",
            command.chapter_index,
            command.provenance.confidence,
            Some(&command.provenance.claim_id),
        )
        .await?;
    }

    if let Some(link) = &action.organization_link {
        let record = PlaceRepo::find_or_create_entity_link_with_conn(
            conn,
            &command.book_id,
            &link.organization_entity_id,
            &place.id,
            &link.link_type,
            &command.provenance.claim_id,
            command.provenance.confidence,
        )
        .await?;
        push_unique(&mut result.entity_links_touched, record.id);
    }

    result
        .claims_accepted
        .push(command.provenance.claim_id.clone());
    Ok(())
}

async fn apply_place_edge(
    command: &PlaceWriteCommand,
    action: &PlaceWriteEdgeAction,
    conn: &mut SqliteConnection,
    result: &mut PlaceReductionResult,
) -> anyhow::Result<()> {
    validate_edge_type(&action.edge_type)?;
    let (normalized_from_place_id, normalized_to_place_id, normalized_edge_type) =
        PlaceRepo::normalize_edge_identity(
            &action.from_place_id,
            &action.to_place_id,
            &action.edge_type,
        );

    if normalized_edge_type == "contains" {
        if would_create_parent_cycle(conn, &normalized_to_place_id, &normalized_from_place_id)
            .await?
        {
            result
                .claims_rejected
                .push(command.provenance.claim_id.clone());
            return Ok(());
        }
        let child_detail =
            PlaceRepo::get_place_detail_with_conn(conn, &normalized_to_place_id).await?;
        if has_conflicting_parent(child_detail.as_ref(), &normalized_from_place_id)
            && !action.allow_parent_change
        {
            result
                .claims_skipped
                .push(command.provenance.claim_id.clone());
            return Ok(());
        }
    }

    let edge = PlaceRepo::find_or_create_edge_with_conn(
        conn,
        &command.book_id,
        &normalized_from_place_id,
        &normalized_to_place_id,
        &normalized_edge_type,
        action.direction_hint.as_deref(),
        action.distance_hint.as_deref(),
        action.confidence,
        &command.provenance.claim_id,
        command.chapter_index,
    )
    .await?;

    if normalized_edge_type == "contains" {
        upsert_contains_parent_detail_from_command(
            conn,
            &command.book_id,
            &normalized_from_place_id,
            &normalized_to_place_id,
            command,
        )
        .await?;
    }

    push_unique(&mut result.edges_touched, edge.id);
    result
        .claims_accepted
        .push(command.provenance.claim_id.clone());
    Ok(())
}

async fn apply_place_conflict(
    command: &PlaceWriteCommand,
    action: &PlaceWriteConflictAction,
    conn: &mut SqliteConnection,
    result: &mut PlaceReductionResult,
) -> anyhow::Result<()> {
    let conflict_type = normalize_conflict_type(&action.conflict_type);
    let conflict = PlaceRepo::insert_conflict_with_conn(
        conn,
        &command.book_id,
        &command.provenance.claim_id,
        action.existing_edge_id.as_deref(),
        conflict_type,
        &action.reason_code,
        None,
    )
    .await?;
    push_unique(&mut result.conflicts_recorded, conflict.id);
    result
        .claims_accepted
        .push(command.provenance.claim_id.clone());
    Ok(())
}

async fn get_or_create_place_entity(
    conn: &mut SqliteConnection,
    book_id: &str,
    entity_id: Option<&str>,
    display_name: &str,
    place_type: &str,
    importance_score: f64,
    chapter_index: i64,
) -> anyhow::Result<EntityRecord> {
    if let Some(entity_id) = entity_id {
        let entity = EntityRepo::get_by_id_with_conn(conn, entity_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("place entity not found: {entity_id}"))?;
        if entity.entity_type != "place" {
            anyhow::bail!(
                "entity {} is {}, expected place",
                entity.id,
                entity.entity_type
            );
        }
        EntityRepo::update_last_seen_with_conn(conn, &entity.id, chapter_index).await?;
        return Ok(entity);
    }

    if let Some(entity) =
        find_place_by_canonical_name_with_conn(conn, book_id, display_name).await?
    {
        EntityRepo::update_last_seen_with_conn(conn, &entity.id, chapter_index).await?;
        return Ok(entity);
    }

    EntityRepo::create_entity_with_conn(
        conn,
        book_id,
        "place",
        display_name,
        display_name,
        None,
        importance_score,
        chapter_index,
    )
    .await
    .map_err(|err| anyhow::anyhow!("failed to create {place_type} place entity: {err}"))
}

async fn find_place_by_canonical_name_with_conn(
    conn: &mut SqliteConnection,
    book_id: &str,
    name: &str,
) -> anyhow::Result<Option<EntityRecord>> {
    let Some(entity) = EntityRepo::get_by_canonical_name_with_conn(conn, book_id, name).await?
    else {
        return Ok(None);
    };
    if entity.entity_type == "place" {
        Ok(Some(entity))
    } else {
        Ok(None)
    }
}

async fn would_create_parent_cycle(
    conn: &mut SqliteConnection,
    child_id: &str,
    parent_id: &str,
) -> anyhow::Result<bool> {
    if child_id == parent_id {
        return Ok(true);
    }

    let mut current = Some(parent_id.to_string());
    let mut guard = 0;
    while let Some(entity_id) = current {
        if entity_id == child_id {
            return Ok(true);
        }
        guard += 1;
        if guard > 128 {
            anyhow::bail!("place parent chain too deep");
        }
        current = PlaceRepo::get_place_detail_with_conn(conn, &entity_id)
            .await?
            .and_then(|detail: PlaceDetailRecord| detail.parent_place_id);
    }
    Ok(false)
}

fn validate_place_type(place_type: &str) -> anyhow::Result<()> {
    if VALID_PLACE_TYPES.contains(&place_type) {
        Ok(())
    } else {
        anyhow::bail!("invalid place_type: {place_type}")
    }
}

fn validate_edge_type(edge_type: &str) -> anyhow::Result<()> {
    if VALID_PLACE_EDGE_TYPES.contains(&edge_type) {
        Ok(())
    } else {
        anyhow::bail!("invalid edge_type: {edge_type}")
    }
}

fn normalize_conflict_type(conflict_type: &str) -> &'static str {
    match conflict_type {
        "opposite_direction" | "direction_conflict" => "duplicate_conflicting_direction",
        "hierarchy_cycle" | "cycle_risk" | "parent_conflict" => "hierarchy_cycle",
        "unclear_reference" | "ambiguous_place" => "unclear_reference",
        "low_evidence" => "low_evidence",
        "impossible_containment" => "impossible_containment",
        "duplicate_conflicting_direction" => "duplicate_conflicting_direction",
        _ => "none",
    }
}

fn has_conflicting_parent(existing_detail: Option<&PlaceDetailRecord>, parent_id: &str) -> bool {
    existing_detail
        .and_then(|detail| detail.parent_place_id.as_deref())
        .is_some_and(|existing_parent| existing_parent != parent_id)
}

async fn upsert_contains_parent_detail_from_command(
    conn: &mut SqliteConnection,
    book_id: &str,
    parent_place_id: &str,
    child_place_id: &str,
    command: &PlaceWriteCommand,
) -> anyhow::Result<()> {
    let existing = PlaceRepo::get_place_detail_with_conn(conn, child_place_id).await?;
    let place_type = existing
        .as_ref()
        .map(|detail| detail.place_type.as_str())
        .unwrap_or("unknown");
    let scale_level = existing
        .as_ref()
        .map(|detail| detail.scale_level)
        .unwrap_or(0);
    let importance_score = existing
        .as_ref()
        .map(|detail| detail.importance_score)
        .unwrap_or(command.provenance.confidence.clamp(0.0, 1.0));
    let map_visible = existing
        .as_ref()
        .map(|detail| detail.map_visible != 0)
        .unwrap_or(true);
    let first_seen_chapter = existing
        .as_ref()
        .map(|detail| detail.first_seen_chapter)
        .unwrap_or(command.chapter_index);
    let status = existing
        .as_ref()
        .map(|detail| detail.status.as_str())
        .unwrap_or("active");

    PlaceRepo::upsert_place_detail_with_conn(
        conn,
        book_id,
        child_place_id,
        place_type,
        Some(parent_place_id),
        scale_level,
        importance_score,
        map_visible,
        first_seen_chapter,
        command.chapter_index,
        status,
    )
    .await?;
    Ok(())
}

async fn invalidate_map_cache_with_conn(
    conn: &mut SqliteConnection,
    book_id: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM view_model_cache
         WHERE book_id = ? AND view_type IN (
            'map_overview',
            'map_places',
            'place_detail',
            'map_graph',
            'map_layout',
            'map_conflicts'
         )",
    )
    .bind(book_id)
    .execute(conn)
    .await?;
    Ok(())
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
    use serde_json::json;

    async fn setup_test_db() -> (SqlitePool, ClaimRepo, EntityRepo, PlaceRepo, String, String) {
        let dir =
            std::env::temp_dir().join(format!("reader-v4-place-reducer-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();

        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 1, '青云城在东域，云城是它的别称。', 'h1', datetime('now'))")
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
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'h1', ?, 0, 0, 20, '青云城在东域', datetime('now'))")
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

    async fn location_intro_claim(
        claim_repo: &ClaimRepo,
        span_id: &str,
        run_id: &str,
        place_name: &str,
        value_json: serde_json::Value,
    ) -> ClaimRecord {
        claim_repo
            .create_claim(
                "b1",
                1,
                "location_introduction",
                Some(place_name),
                None,
                None,
                None,
                "location_introduction",
                Some(place_name),
                Some(&value_json.to_string()),
                span_id,
                run_id,
                0.86,
                "low",
            )
            .await
            .unwrap()
    }

    async fn location_edge_claim(
        claim_repo: &ClaimRepo,
        span_id: &str,
        run_id: &str,
        from_place_id: &str,
        to_place_id: &str,
        value_json: serde_json::Value,
    ) -> ClaimRecord {
        claim_repo
            .create_claim(
                "b1",
                1,
                "location_edge",
                Some("青云城"),
                Some("黑风谷"),
                Some(from_place_id),
                Some(to_place_id),
                "location_edge",
                None,
                Some(&value_json.to_string()),
                span_id,
                run_id,
                0.88,
                "medium",
            )
            .await
            .unwrap()
    }

    async fn create_place(
        entity_repo: &EntityRepo,
        place_repo: &PlaceRepo,
        name: &str,
        place_type: &str,
    ) -> String {
        let entity = entity_repo
            .create_entity("b1", "place", name, name, None, 0.6, 1)
            .await
            .unwrap();
        place_repo
            .upsert_place_detail(
                "b1", &entity.id, place_type, None, 0, 0.6, true, 1, 1, "active",
            )
            .await
            .unwrap();
        entity.id
    }

    #[tokio::test]
    async fn place_reducer_accepts_typed_place_command_without_claim_value_json() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "location_introduction",
                Some("青云城"),
                None,
                None,
                None,
                "location_introduction",
                None,
                None,
                &span_id,
                &run_id,
                0.86,
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
                    evidence_span_ids: vec![span_id.clone()],
                    confidence: 0.86,
                },
                action: PlaceWriteAction::UpsertPlace(PlaceWritePlaceAction {
                    resolved_place_entity_id: None,
                    display_name: "青云城".to_string(),
                    place_type: "city".to_string(),
                    aliases: vec!["云城".to_string()],
                    parent_place_id: None,
                    organization_link: None,
                    scale_level: 0,
                    importance_score: 0.72,
                    map_visible: true,
                }),
            },
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(result.claims_accepted, vec![claim.id.clone()]);
        let place = entity_repo
            .get_by_canonical_name_and_type("b1", "place", "青云城")
            .await
            .unwrap()
            .unwrap();
        let detail = place_repo
            .get_place_detail(&place.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.place_type, "city");
        let aliases = entity_repo.list_aliases_by_entity(&place.id).await.unwrap();
        assert!(aliases.iter().any(|alias| alias.alias == "云城"));
        assert_eq!(
            claim_repo
                .get_claim(&claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "proposed"
        );
    }

    #[tokio::test]
    async fn place_reducer_reports_accepted_claim_without_mutating_ledger_status() {
        let (pool, claim_repo, _entity_repo, _place_repo, span_id, run_id) = setup_test_db().await;
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "location_introduction",
                Some("青云城"),
                None,
                None,
                None,
                "location_introduction",
                None,
                None,
                &span_id,
                &run_id,
                0.86,
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
                    evidence_span_ids: vec![span_id.clone()],
                    confidence: 0.86,
                },
                action: PlaceWriteAction::UpsertPlace(PlaceWritePlaceAction {
                    resolved_place_entity_id: None,
                    display_name: "青云城".to_string(),
                    place_type: "city".to_string(),
                    aliases: Vec::new(),
                    parent_place_id: None,
                    organization_link: None,
                    scale_level: 0,
                    importance_score: 0.72,
                    map_visible: true,
                }),
            },
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(result.claims_accepted, vec![claim.id.clone()]);
        let stored = claim_repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(stored.status, "proposed");
    }

    #[tokio::test]
    async fn place_reducer_accepts_typed_edge_command_without_claim_value_json() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let from_id = create_place(&entity_repo, &place_repo, "青云城", "city").await;
        let to_id = create_place(&entity_repo, &place_repo, "黑风谷", "dungeon").await;
        let claim = claim_repo
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
                None,
                &span_id,
                &run_id,
                0.88,
                "medium",
            )
            .await
            .unwrap();

        let result = apply_place_write(
            PlaceWriteCommand {
                book_id: "b1".to_string(),
                chapter_index: 1,
                provenance: PlaceWriteProvenance {
                    claim_id: claim.id.clone(),
                    evidence_span_ids: vec![span_id.clone()],
                    confidence: 0.88,
                },
                action: PlaceWriteAction::UpsertEdge(PlaceWriteEdgeAction {
                    from_place_id: from_id.clone(),
                    to_place_id: to_id.clone(),
                    edge_type: "route_to".to_string(),
                    direction_hint: Some("north road".to_string()),
                    distance_hint: Some("三日路程".to_string()),
                    confidence: 0.91,
                    allow_parent_change: false,
                }),
            },
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(result.claims_accepted, vec![claim.id.clone()]);
        assert_eq!(result.edges_touched.len(), 1);
        let edge = place_repo
            .get_edge_by_id(&result.edges_touched[0])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.edge_type, "route_to");
        assert_eq!(edge.from_place_id, from_id);
        assert_eq!(edge.to_place_id, to_id);
        assert_eq!(edge.source_claim_id, claim.id);
        assert_eq!(
            claim_repo
                .get_claim(&claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "proposed"
        );
    }

    #[tokio::test]
    async fn place_reducer_records_typed_conflict_without_claim_value_json() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let _from_id = create_place(&entity_repo, &place_repo, "青云城", "city").await;
        let _to_id = create_place(&entity_repo, &place_repo, "黑风谷", "dungeon").await;
        let claim = claim_repo
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
                None,
                &span_id,
                &run_id,
                0.88,
                "medium",
            )
            .await
            .unwrap();

        let result = apply_place_write(
            PlaceWriteCommand {
                book_id: "b1".to_string(),
                chapter_index: 1,
                provenance: PlaceWriteProvenance {
                    claim_id: claim.id.clone(),
                    evidence_span_ids: vec![span_id.clone()],
                    confidence: 0.88,
                },
                action: PlaceWriteAction::RecordConflict(PlaceWriteConflictAction {
                    existing_edge_id: None,
                    conflict_type: "duplicate_conflicting_direction".to_string(),
                    reason_code: "opposite_direction".to_string(),
                }),
            },
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(result.claims_accepted, vec![claim.id.clone()]);
        assert_eq!(result.conflicts_recorded.len(), 1);
        assert!(result.edges_touched.is_empty());
        assert_eq!(
            place_repo
                .list_conflicts("b1", Some("open"))
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            claim_repo
                .get_claim(&claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "proposed"
        );
    }

    #[tokio::test]
    async fn typed_place_command_rejects_parent_cycle() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let parent_id = create_place(&entity_repo, &place_repo, "东域", "region").await;
        let child_id = create_place(&entity_repo, &place_repo, "青云城", "city").await;
        place_repo
            .upsert_place_detail(
                "b1",
                &child_id,
                "city",
                Some(&parent_id),
                0,
                0.6,
                true,
                1,
                1,
                "active",
            )
            .await
            .unwrap();
        let claim = location_intro_claim(&claim_repo, &span_id, &run_id, "东域", json!({})).await;
        let result = apply_place_write(
            PlaceWriteCommand {
                book_id: "b1".to_string(),
                chapter_index: 1,
                provenance: PlaceWriteProvenance {
                    claim_id: claim.id.clone(),
                    evidence_span_ids: vec![span_id],
                    confidence: 0.86,
                },
                action: PlaceWriteAction::UpsertPlace(PlaceWritePlaceAction {
                    resolved_place_entity_id: Some(parent_id.clone()),
                    display_name: "东域".to_string(),
                    place_type: "region".to_string(),
                    aliases: Vec::new(),
                    parent_place_id: Some(child_id),
                    organization_link: None,
                    scale_level: 0,
                    importance_score: 0.7,
                    map_visible: true,
                }),
            },
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(result.claims_rejected, vec![claim.id.clone()]);
        assert_eq!(
            place_repo
                .get_place_detail(&parent_id)
                .await
                .unwrap()
                .unwrap()
                .parent_place_id,
            None
        );
        assert_eq!(
            claim_repo
                .get_claim(&claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "proposed"
        );
    }

    #[tokio::test]
    async fn typed_contains_edges_update_same_parent_and_dedupe_sources() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let parent_id = create_place(&entity_repo, &place_repo, "青云门", "sect_site").await;
        let child_id = create_place(&entity_repo, &place_repo, "丹房", "building").await;
        let inside_claim = location_edge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            &child_id,
            &parent_id,
            json!({}),
        )
        .await;
        let part_of_claim = location_edge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            &child_id,
            &parent_id,
            json!({}),
        )
        .await;
        let first = apply_place_write(
            PlaceWriteCommand {
                book_id: "b1".to_string(),
                chapter_index: 1,
                provenance: PlaceWriteProvenance {
                    claim_id: inside_claim.id.clone(),
                    evidence_span_ids: vec![span_id.clone()],
                    confidence: 0.88,
                },
                action: PlaceWriteAction::UpsertEdge(PlaceWriteEdgeAction {
                    from_place_id: child_id.clone(),
                    to_place_id: parent_id.clone(),
                    edge_type: "inside".to_string(),
                    direction_hint: None,
                    distance_hint: None,
                    confidence: 0.88,
                    allow_parent_change: true,
                }),
            },
            &pool,
        )
        .await
        .unwrap();
        let second = apply_place_write(
            PlaceWriteCommand {
                book_id: "b1".to_string(),
                chapter_index: 1,
                provenance: PlaceWriteProvenance {
                    claim_id: part_of_claim.id.clone(),
                    evidence_span_ids: vec![span_id],
                    confidence: 0.88,
                },
                action: PlaceWriteAction::UpsertEdge(PlaceWriteEdgeAction {
                    from_place_id: child_id.clone(),
                    to_place_id: parent_id.clone(),
                    edge_type: "part_of".to_string(),
                    direction_hint: None,
                    distance_hint: None,
                    confidence: 0.88,
                    allow_parent_change: true,
                }),
            },
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(first.claims_accepted.len(), 1);
        assert_eq!(second.claims_accepted.len(), 1);
        assert_eq!(first.edges_touched, second.edges_touched);
        let detail = place_repo
            .get_place_detail(&child_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.parent_place_id.as_deref(), Some(parent_id.as_str()));
        let edge = place_repo
            .get_edge_by_id(&first.edges_touched[0])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.edge_type, "contains");
        assert_eq!(edge.from_place_id, parent_id);
        assert_eq!(edge.to_place_id, child_id);
        let sources = place_repo.list_edge_sources(&edge.id).await.unwrap();
        assert_eq!(sources.len(), 2);
    }

    #[tokio::test]
    async fn typed_place_parent_change_without_judge_does_not_overwrite_existing_parent() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let original_parent_id = create_place(&entity_repo, &place_repo, "东域", "region").await;
        let new_parent_id = create_place(&entity_repo, &place_repo, "西域", "region").await;
        let child_id = create_place(&entity_repo, &place_repo, "青云城", "city").await;
        place_repo
            .upsert_place_detail(
                "b1",
                &child_id,
                "city",
                Some(&original_parent_id),
                0,
                0.6,
                true,
                1,
                1,
                "active",
            )
            .await
            .unwrap();
        let claim = location_intro_claim(&claim_repo, &span_id, &run_id, "青云城", json!({})).await;
        let result = apply_place_write(
            PlaceWriteCommand {
                book_id: "b1".to_string(),
                chapter_index: 1,
                provenance: PlaceWriteProvenance {
                    claim_id: claim.id.clone(),
                    evidence_span_ids: vec![span_id],
                    confidence: 0.86,
                },
                action: PlaceWriteAction::UpsertPlace(PlaceWritePlaceAction {
                    resolved_place_entity_id: Some(child_id.clone()),
                    display_name: "青云城".to_string(),
                    place_type: "city".to_string(),
                    aliases: Vec::new(),
                    parent_place_id: Some(new_parent_id),
                    organization_link: None,
                    scale_level: 0,
                    importance_score: 0.7,
                    map_visible: true,
                }),
            },
            &pool,
        )
        .await
        .unwrap();

        assert_eq!(result.claims_skipped, vec![claim.id.clone()]);
        let detail = place_repo
            .get_place_detail(&child_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            detail.parent_place_id.as_deref(),
            Some(original_parent_id.as_str())
        );
        assert_eq!(
            claim_repo
                .get_claim(&claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "proposed"
        );
    }

    #[tokio::test]
    async fn typed_organization_place_entity_link_created_without_map_edge() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let organization = entity_repo
            .create_entity("b1", "organization", "青云门", "青云门", None, 0.8, 1)
            .await
            .unwrap();
        let claim =
            location_intro_claim(&claim_repo, &span_id, &run_id, "青云门山门", json!({})).await;
        let result = apply_place_write(
            PlaceWriteCommand {
                book_id: "b1".to_string(),
                chapter_index: 1,
                provenance: PlaceWriteProvenance {
                    claim_id: claim.id,
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
        let links = place_repo
            .list_entity_links_for_entity("b1", place_id)
            .await
            .unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].link_type, "organization_place_pair");
        assert_eq!(links[0].entity_a_id, organization.id);
        assert_eq!(links[0].entity_b_id, *place_id);
        let active_edges: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM place_edges WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(active_edges.0, 0);
    }
}
