use crate::service::v4::extractor::{VALID_PLACE_EDGE_TYPES, VALID_PLACE_TYPES};
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
use crate::storage::db::v4::entity_repo::{EntityRecord, EntityRepo};
use crate::storage::db::v4::place_repo::{PlaceDetailRecord, PlaceRepo};
use serde_json::Value;
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

pub async fn reduce_location_claims(
    claims: &[ClaimRecord],
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<PlaceReductionResult> {
    let reducible = claims
        .iter()
        .filter(|claim| {
            claim.status == "proposed"
                && (claim.risk_level == "low" || claim.risk_level == "medium")
                && matches!(
                    claim.claim_type.as_str(),
                    "location_introduction" | "location_edge"
                )
        })
        .collect::<Vec<_>>();

    let mut result = PlaceReductionResult::default();
    if reducible.is_empty() {
        return Ok(result);
    }

    let mut tx = pool.begin().await?;
    for claim in reducible {
        match claim.claim_type.as_str() {
            "location_introduction" => {
                reduce_location_introduction(claim, book_id, &mut tx, &mut result).await?;
            }
            "location_edge" => {
                reduce_location_edge(claim, book_id, &mut tx, &mut result).await?;
            }
            _ => result.claims_skipped.push(claim.id.clone()),
        }
    }

    if !result.claims_accepted.is_empty() {
        ClaimRepo::batch_update_claim_status_with_conn(
            &mut tx,
            &result.claims_accepted,
            "accepted",
        )
        .await?;
    }
    if !result.claims_rejected.is_empty() {
        ClaimRepo::batch_update_claim_status_with_conn(
            &mut tx,
            &result.claims_rejected,
            "rejected",
        )
        .await?;
    }

    invalidate_map_cache_with_conn(&mut tx, book_id).await?;
    tx.commit().await?;
    Ok(result)
}

async fn reduce_location_introduction(
    claim: &ClaimRecord,
    book_id: &str,
    conn: &mut SqliteConnection,
    result: &mut PlaceReductionResult,
) -> anyhow::Result<()> {
    let value = claim_value(claim)?;
    let place_type = str_field(&value, "place_type").unwrap_or("unknown");
    validate_place_type(place_type)?;

    let display_name = claim
        .subject_mention
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("location_introduction missing subject_mention"))?;
    let importance_score = value
        .get("importance_score")
        .and_then(Value::as_f64)
        .unwrap_or(claim.confidence.clamp(0.0, 1.0));
    let scale_level = value.get("scale_level").and_then(Value::as_i64).unwrap_or(0);
    let map_visible = value
        .get("map_visible_hint")
        .or_else(|| value.get("map_visible"))
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let place = get_or_create_place_entity(
        conn,
        book_id,
        claim.subject_entity_id.as_deref(),
        display_name,
        place_type,
        importance_score,
        claim.chapter_index,
    )
    .await?;

    let parent_place_id = str_field(&value, "parent_place_id");
    if let Some(parent_id) = parent_place_id {
        if would_create_parent_cycle(conn, &place.id, parent_id).await? {
            result.claims_rejected.push(claim.id.clone());
            return Ok(());
        }
    }

    PlaceRepo::upsert_place_detail_with_conn(
        conn,
        book_id,
        &place.id,
        place_type,
        parent_place_id,
        scale_level,
        importance_score,
        map_visible,
        claim.chapter_index,
        claim.chapter_index,
        "active",
    )
    .await?;
    push_unique(&mut result.places_touched, place.id.clone());

    if let Some(aliases) = value.get("aliases").and_then(Value::as_array) {
        for alias in aliases.iter().filter_map(Value::as_str) {
            let alias = alias.trim();
            if alias.is_empty() || alias == display_name {
                continue;
            }
            EntityRepo::create_alias_with_conn(
                conn,
                book_id,
                &place.id,
                alias,
                "name",
                claim.chapter_index,
                claim.confidence,
                Some(&claim.id),
            )
            .await?;
        }
    }

    if let Some(org_id) = str_field(&value, "organization_link_candidate_id") {
        let link = PlaceRepo::find_or_create_entity_link_with_conn(
            conn,
            book_id,
            org_id,
            &place.id,
            str_field(&value, "entity_link_type").unwrap_or("organization_place_pair"),
            &claim.id,
            claim.confidence,
        )
        .await?;
        push_unique(&mut result.entity_links_touched, link.id);
    }

    result.claims_accepted.push(claim.id.clone());
    Ok(())
}

async fn reduce_location_edge(
    claim: &ClaimRecord,
    book_id: &str,
    conn: &mut SqliteConnection,
    result: &mut PlaceReductionResult,
) -> anyhow::Result<()> {
    let value = claim_value(claim)?;
    let edge_type = str_field(&value, "normalized_edge_type")
        .or_else(|| str_field(&value, "edge_type"))
        .ok_or_else(|| anyhow::anyhow!("location_edge missing edge_type"))?;
    validate_edge_type(edge_type)?;

    let from_place_id = claim
        .subject_entity_id
        .as_deref()
        .or_else(|| str_field(&value, "from_place_id"))
        .ok_or_else(|| anyhow::anyhow!("location_edge missing from_place_id"))?;
    let to_place_id = claim
        .object_entity_id
        .as_deref()
        .or_else(|| str_field(&value, "to_place_id"))
        .ok_or_else(|| anyhow::anyhow!("location_edge missing to_place_id"))?;

    let decision = str_field(&value, "judge_decision")
        .or_else(|| str_field(&value, "decision"))
        .unwrap_or("accept");
    if decision == "conflict" {
        let conflict_type =
            normalize_conflict_type(str_field(&value, "conflict_type").unwrap_or("none"));
        let reason_code = str_field(&value, "reason_code").unwrap_or("map_conflict");
        let judge_output_json = value.get("judge_output").map(Value::to_string);
        let conflict = PlaceRepo::insert_conflict_with_conn(
            conn,
            book_id,
            &claim.id,
            str_field(&value, "existing_edge_id"),
            conflict_type,
            reason_code,
            judge_output_json.as_deref(),
        )
        .await?;
        push_unique(&mut result.conflicts_recorded, conflict.id);
        result.claims_accepted.push(claim.id.clone());
        return Ok(());
    }

    if decision == "reject" {
        result.claims_rejected.push(claim.id.clone());
        return Ok(());
    }
    if decision == "uncertain" {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    }

    let edge = PlaceRepo::find_or_create_edge_with_conn(
        conn,
        book_id,
        from_place_id,
        to_place_id,
        edge_type,
        str_field(&value, "normalized_direction_hint")
            .or_else(|| str_field(&value, "direction_hint")),
        str_field(&value, "normalized_distance_hint").or_else(|| str_field(&value, "distance_hint")),
        value
            .get("judge_confidence")
            .or_else(|| value.get("confidence"))
            .and_then(Value::as_f64)
            .unwrap_or(claim.confidence),
        &claim.id,
        claim.chapter_index,
    )
    .await?;
    push_unique(&mut result.edges_touched, edge.id);
    result.claims_accepted.push(claim.id.clone());
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
            anyhow::bail!("entity {} is {}, expected place", entity.id, entity.entity_type);
        }
        EntityRepo::update_last_seen_with_conn(conn, &entity.id, chapter_index).await?;
        return Ok(entity);
    }

    if let Some(entity) = find_place_by_canonical_name_with_conn(conn, book_id, display_name).await? {
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

fn claim_value(claim: &ClaimRecord) -> anyhow::Result<Value> {
    let Some(raw) = claim.value_json.as_deref() else {
        return Ok(Value::Object(Default::default()));
    };
    serde_json::from_str(raw).map_err(|err| anyhow::anyhow!("invalid location claim value_json: {err}"))
}

fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
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
    use serde_json::json;

    async fn setup_test_db() -> (SqlitePool, ClaimRepo, EntityRepo, PlaceRepo, String, String) {
        let dir = std::env::temp_dir().join(format!(
            "reader-v4-place-reducer-{}",
            uuid::Uuid::new_v4()
        ));
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
                "b1",
                &entity.id,
                place_type,
                None,
                0,
                0.6,
                true,
                1,
                1,
                "active",
            )
            .await
            .unwrap();
        entity.id
    }

    #[tokio::test]
    async fn place_reducer_location_introduction_creates_place_entity() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let claim = location_intro_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "青云城",
            json!({
                "place_type": "city",
                "aliases": [],
                "importance_score": 0.7,
                "map_visible_hint": true
            }),
        )
        .await;

        let result = reduce_location_claims(&[claim.clone()], "b1", &pool)
            .await
            .unwrap();

        assert_eq!(result.claims_accepted, vec![claim.id.clone()]);
        let place = entity_repo
            .get_by_canonical_name_and_type("b1", "place", "青云城")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(place.entity_type, "place");
        let detail = place_repo.get_place_detail(&place.id).await.unwrap().unwrap();
        assert_eq!(detail.place_type, "city");
        assert_eq!(claim_repo.get_claim(&claim.id).await.unwrap().unwrap().status, "accepted");
    }

    #[tokio::test]
    async fn place_reducer_alias_stored_in_entity_aliases() {
        let (pool, claim_repo, entity_repo, _place_repo, span_id, run_id) = setup_test_db().await;
        let claim = location_intro_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "青云城",
            json!({
                "place_type": "city",
                "aliases": ["云城", "青城"],
                "importance_score": 0.7,
                "map_visible_hint": true
            }),
        )
        .await;

        reduce_location_claims(&[claim.clone()], "b1", &pool)
            .await
            .unwrap();

        let place = entity_repo
            .get_by_canonical_name_and_type("b1", "place", "青云城")
            .await
            .unwrap()
            .unwrap();
        let aliases = entity_repo.list_aliases_by_entity(&place.id).await.unwrap();
        let alias_values = aliases
            .iter()
            .map(|alias| alias.alias.as_str())
            .collect::<Vec<_>>();
        assert!(alias_values.contains(&"云城"));
        assert!(alias_values.contains(&"青城"));
    }

    #[tokio::test]
    async fn place_reducer_parent_cycle_rejected() {
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

        let claim = location_intro_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "东域",
            json!({
                "place_type": "region",
                "parent_place_id": child_id,
                "aliases": []
            }),
        )
        .await;

        let result = reduce_location_claims(&[claim.clone()], "b1", &pool)
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
        assert_eq!(claim_repo.get_claim(&claim.id).await.unwrap().unwrap().status, "rejected");
    }

    #[tokio::test]
    async fn place_reducer_add_edge_active() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let from_id = create_place(&entity_repo, &place_repo, "青云城", "city").await;
        let to_id = create_place(&entity_repo, &place_repo, "黑风谷", "dungeon").await;
        let claim = location_edge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            &from_id,
            &to_id,
            json!({
                "edge_type": "route_to",
                "direction_hint": "north road",
                "distance_hint": "三日路程",
                "judge_decision": "accept"
            }),
        )
        .await;

        let result = reduce_location_claims(&[claim.clone()], "b1", &pool)
            .await
            .unwrap();

        assert_eq!(result.claims_accepted, vec![claim.id.clone()]);
        assert_eq!(result.edges_touched.len(), 1);
        let edge = place_repo
            .get_edge_by_id(&result.edges_touched[0])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.status, "active");
        assert_eq!(edge.edge_type, "route_to");
        assert_eq!(edge.source_claim_id, claim.id);
    }

    #[tokio::test]
    async fn place_reducer_conflicting_edge_records_conflict_not_active() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let from_id = create_place(&entity_repo, &place_repo, "青云城", "city").await;
        let to_id = create_place(&entity_repo, &place_repo, "黑风谷", "dungeon").await;
        let claim = location_edge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            &from_id,
            &to_id,
            json!({
                "edge_type": "north_of",
                "judge_decision": "conflict",
                "conflict_type": "duplicate_conflicting_direction",
                "reason_code": "opposite_direction"
            }),
        )
        .await;

        let result = reduce_location_claims(&[claim.clone()], "b1", &pool)
            .await
            .unwrap();

        assert_eq!(result.conflicts_recorded.len(), 1);
        assert_eq!(result.edges_touched.len(), 0);
        assert_eq!(place_repo.list_conflicts("b1", Some("open")).await.unwrap().len(), 1);
        let active_edges: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM place_edges WHERE book_id = 'b1' AND status = 'active'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(active_edges.0, 0);
    }

    #[tokio::test]
    async fn organization_place_entity_link_created() {
        let (pool, claim_repo, entity_repo, place_repo, span_id, run_id) = setup_test_db().await;
        let organization = entity_repo
            .create_entity("b1", "organization", "青云门", "青云门", None, 0.8, 1)
            .await
            .unwrap();
        let claim = location_intro_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "青云门山门",
            json!({
                "place_type": "sect_site",
                "organization_link_candidate_id": organization.id,
                "entity_link_type": "organization_place_pair"
            }),
        )
        .await;

        let result = reduce_location_claims(&[claim], "b1", &pool).await.unwrap();

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
    }

    #[tokio::test]
    async fn entity_links_are_not_map_edges() {
        let (pool, claim_repo, entity_repo, _place_repo, span_id, run_id) = setup_test_db().await;
        let organization = entity_repo
            .create_entity("b1", "organization", "青云门", "青云门", None, 0.8, 1)
            .await
            .unwrap();
        let claim = location_intro_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "青云门山门",
            json!({
                "place_type": "sect_site",
                "organization_link_candidate_id": organization.id
            }),
        )
        .await;

        reduce_location_claims(&[claim], "b1", &pool).await.unwrap();

        let active_edges: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM place_edges WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(active_edges.0, 0);
    }

    #[tokio::test]
    async fn place_reducer_rolls_back_on_failure() {
        let (pool, claim_repo, _entity_repo, _place_repo, span_id, run_id) = setup_test_db().await;
        let good = location_intro_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "青云城",
            json!({
                "place_type": "city",
                "aliases": ["云城"]
            }),
        )
        .await;
        let bad = location_intro_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "坏地点",
            json!({
                "place_type": "not_a_real_place_type",
                "aliases": []
            }),
        )
        .await;

        let err = reduce_location_claims(&[good.clone(), bad], "b1", &pool)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("invalid place_type"));
        let place_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM entities WHERE book_id = 'b1' AND entity_type = 'place'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(place_count.0, 0);
        assert_eq!(claim_repo.get_claim(&good.id).await.unwrap().unwrap().status, "proposed");
    }
}
