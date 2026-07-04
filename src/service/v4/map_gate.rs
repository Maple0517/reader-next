use crate::service::v4::extractor::VALID_PLACE_EDGE_TYPES;
use crate::storage::db::v4::claim_repo::ClaimRecord;
use crate::storage::db::v4::entity_repo::EntityRecord;

const EXTREMELY_LOW_CONFIDENCE_THRESHOLD: f64 = 0.25;
const WEAK_CONFIDENCE_THRESHOLD: f64 = 0.5;

#[derive(Debug, Clone, PartialEq)]
pub enum MapGateResult {
    Pass,
    Reject(String),
    Uncertain(String),
}

#[derive(Debug, Clone)]
pub struct MapGateContext<'a> {
    pub claim: &'a ClaimRecord,
    pub from_place: Option<&'a EntityRecord>,
    pub to_place: Option<&'a EntityRecord>,
    pub existing_parent_links: &'a [(String, String)],
}

pub fn structural_gate(context: MapGateContext<'_>) -> MapGateResult {
    let claim = context.claim;
    let value_json = match claim
        .value_json
        .as_ref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
    {
        Some(value) => value,
        None => return MapGateResult::Reject("missing map claim value_json".to_string()),
    };

    if claim.primary_source_span_id.trim().is_empty() || !has_evidence_span_ids(&value_json) {
        return MapGateResult::Reject("missing evidence spans".to_string());
    }

    if claim.confidence < EXTREMELY_LOW_CONFIDENCE_THRESHOLD {
        return MapGateResult::Reject(format!(
            "confidence {} is extremely low",
            claim.confidence
        ));
    }

    if has_knowledge_summary_shape(&value_json) {
        return MapGateResult::Reject("knowledge summary is not canonical map topology".to_string());
    }

    if claim.claim_type == "location_edge" {
        if value_json
            .get("is_topological_hint")
            .and_then(|value| value.as_bool())
            == Some(false)
        {
            return MapGateResult::Reject(
                "non-map character movement/current location hint".to_string(),
            );
        }

        let edge_type = required_string(&value_json, "edge_type").unwrap_or_default();
        if !VALID_PLACE_EDGE_TYPES.contains(&edge_type.as_str()) {
            return MapGateResult::Reject(format!("invalid edge_type: {}", edge_type));
        }
    } else if claim.claim_type != "location_introduction" {
        return MapGateResult::Reject(format!(
            "unsupported map claim_type: {}",
            claim.claim_type
        ));
    }

    let Some(from_place) = context.from_place else {
        return unresolved_endpoint_result(context.to_place.is_none());
    };
    let Some(to_place) = context.to_place else {
        return unresolved_endpoint_result(false);
    };

    if from_place.entity_type != "place" || to_place.entity_type != "place" {
        return MapGateResult::Reject("map endpoints must resolve to place entities".to_string());
    }

    if from_place.id == to_place.id {
        return MapGateResult::Reject("self edge is not valid map topology".to_string());
    }

    if claim.confidence < WEAK_CONFIDENCE_THRESHOLD {
        return MapGateResult::Uncertain(format!("confidence {} is weak", claim.confidence));
    }

    if let Some((child_id, parent_id)) =
        hierarchy_candidate(claim, &value_json, from_place, to_place)
    {
        if would_create_parent_cycle(&child_id, &parent_id, context.existing_parent_links) {
            return MapGateResult::Reject("contains hierarchy would create cycle".to_string());
        }
    }

    MapGateResult::Pass
}

fn unresolved_endpoint_result(both_unresolved: bool) -> MapGateResult {
    if both_unresolved {
        MapGateResult::Reject("both map endpoints are unresolved".to_string())
    } else {
        MapGateResult::Uncertain("one map endpoint is unresolved".to_string())
    }
}

fn has_evidence_span_ids(value_json: &serde_json::Value) -> bool {
    value_json
        .get("evidence_span_ids")
        .and_then(|value| value.as_array())
        .map(|ids| {
            ids.iter()
                .any(|id| id.as_str().is_some_and(|value| !value.trim().is_empty()))
        })
        .unwrap_or(false)
}

fn has_knowledge_summary_shape(value_json: &serde_json::Value) -> bool {
    ["category", "topic", "assertion_text"]
        .iter()
        .any(|key| value_json.get(*key).is_some())
}

fn required_string(value_json: &serde_json::Value, key: &str) -> Option<String> {
    value_json
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn hierarchy_candidate(
    claim: &ClaimRecord,
    value_json: &serde_json::Value,
    from_place: &EntityRecord,
    to_place: &EntityRecord,
) -> Option<(String, String)> {
    if claim.claim_type == "location_introduction" {
        return Some((to_place.id.clone(), from_place.id.clone()));
    }

    let edge_type = required_string(value_json, "edge_type")?;
    match edge_type.as_str() {
        "contains" => Some((to_place.id.clone(), from_place.id.clone())),
        "inside" | "part_of" => Some((from_place.id.clone(), to_place.id.clone())),
        _ => None,
    }
}

fn would_create_parent_cycle(
    child_id: &str,
    parent_id: &str,
    existing_parent_links: &[(String, String)],
) -> bool {
    if child_id == parent_id {
        return true;
    }

    let mut cursor = parent_id;
    for _ in 0..existing_parent_links.len() {
        let Some((_, next_parent)) = existing_parent_links
            .iter()
            .find(|(existing_child, _)| existing_child == cursor)
        else {
            return false;
        };

        if next_parent == child_id {
            return true;
        }

        cursor = next_parent;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_claim(
        claim_type: &str,
        value_json: serde_json::Value,
        primary_source_span_id: &str,
        confidence: f64,
    ) -> ClaimRecord {
        ClaimRecord {
            id: "claim1".to_string(),
            book_id: "book1".to_string(),
            chapter_index: 1,
            claim_type: claim_type.to_string(),
            subject_mention: Some("黑风谷".to_string()),
            object_mention: Some("青云城".to_string()),
            subject_entity_id: Some("place_from".to_string()),
            object_entity_id: Some("place_to".to_string()),
            predicate: "黑风谷 north_of 青云城".to_string(),
            value_json: Some(value_json.to_string()),
            value_text: None,
            primary_source_span_id: primary_source_span_id.to_string(),
            ai_run_id: "run1".to_string(),
            confidence,
            risk_level: "high".to_string(),
            status: "proposed".to_string(),
            supersedes_claim_id: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn location_edge_value(from: &str, to: &str, edge_type: &str) -> serde_json::Value {
        json!({
            "edge_type": edge_type,
            "direction_hint": null,
            "distance_hint": null,
            "is_topological_hint": true,
            "evidence_span_ids": ["span1"],
            "from_place_mention": from,
            "to_place_mention": to
        })
    }

    fn entity(id: &str, entity_type: &str) -> EntityRecord {
        EntityRecord {
            id: id.to_string(),
            book_id: "book1".to_string(),
            entity_type: entity_type.to_string(),
            canonical_name: id.to_string(),
            display_name: id.to_string(),
            short_summary: None,
            importance_score: 0.5,
            first_seen_chapter: 1,
            last_seen_chapter: 1,
            status: "active".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn gate(
        claim: &ClaimRecord,
        from_place: Option<&EntityRecord>,
        to_place: Option<&EntityRecord>,
        existing_parent_links: &[(String, String)],
    ) -> MapGateResult {
        structural_gate(MapGateContext {
            claim,
            from_place,
            to_place,
            existing_parent_links,
        })
    }

    #[test]
    fn map_gate_rejects_missing_evidence() {
        let mut value = location_edge_value("黑风谷", "青云城", "north_of");
        value["evidence_span_ids"] = json!([]);
        let claim = make_claim("location_edge", value, "", 0.8);
        let from = entity("place_from", "place");
        let to = entity("place_to", "place");

        let result = gate(&claim, Some(&from), Some(&to), &[]);

        assert!(matches!(result, MapGateResult::Reject(reason) if reason.contains("evidence")));
    }

    #[test]
    fn map_gate_rejects_self_edge() {
        let claim = make_claim(
            "location_edge",
            location_edge_value("青云城", "青云城", "north_of"),
            "span1",
            0.8,
        );
        let place = entity("same_place", "place");

        let result = gate(&claim, Some(&place), Some(&place), &[]);

        assert!(matches!(result, MapGateResult::Reject(reason) if reason.contains("self")));
    }

    #[test]
    fn map_gate_rejects_invalid_edge_type() {
        let claim = make_claim(
            "location_edge",
            location_edge_value("黑风谷", "青云城", "temporary_visit"),
            "span1",
            0.8,
        );
        let from = entity("place_from", "place");
        let to = entity("place_to", "place");

        let result = gate(&claim, Some(&from), Some(&to), &[]);

        assert!(matches!(result, MapGateResult::Reject(reason) if reason.contains("edge_type")));
    }

    #[test]
    fn map_gate_rejects_character_movement() {
        let mut value = location_edge_value("张三", "青云城", "route_to");
        value["is_topological_hint"] = json!(false);
        value["subject_mention"] = json!("张三");
        let claim = make_claim("location_edge", value, "span1", 0.8);
        let from = entity("char1", "character");
        let to = entity("place_to", "place");

        let result = gate(&claim, Some(&from), Some(&to), &[]);

        assert!(matches!(result, MapGateResult::Reject(reason) if reason.contains("movement") || reason.contains("non-map")));
    }

    #[test]
    fn map_gate_rejects_knowledge_summary() {
        let mut value = location_edge_value("北境", "东域", "near");
        value["category"] = json!("geography");
        value["topic"] = json!("北境地理");
        value["assertion_text"] = json!("北境常年冰封。");
        let claim = make_claim("location_edge", value, "span1", 0.8);
        let from = entity("place_from", "place");
        let to = entity("place_to", "place");

        let result = gate(&claim, Some(&from), Some(&to), &[]);

        assert!(matches!(result, MapGateResult::Reject(reason) if reason.contains("knowledge")));
    }

    #[test]
    fn map_gate_rejects_immediate_hierarchy_cycle() {
        let claim = make_claim(
            "location_edge",
            location_edge_value("青云城", "东域", "contains"),
            "span1",
            0.8,
        );
        let city = entity("city", "place");
        let region = entity("region", "place");
        let existing = vec![("city".to_string(), "region".to_string())];

        let result = gate(&claim, Some(&city), Some(&region), &existing);

        assert!(matches!(result, MapGateResult::Reject(reason) if reason.contains("cycle")));
    }

    #[test]
    fn map_gate_passes_valid_map_edge() {
        let claim = make_claim(
            "location_edge",
            location_edge_value("黑风谷", "青云城", "north_of"),
            "span1",
            0.8,
        );
        let from = entity("place_from", "place");
        let to = entity("place_to", "place");

        let result = gate(&claim, Some(&from), Some(&to), &[]);

        assert_eq!(result, MapGateResult::Pass);
    }

    #[test]
    fn map_gate_rejects_non_place_endpoint() {
        let claim = make_claim(
            "location_edge",
            location_edge_value("青云门", "青云城", "near"),
            "span1",
            0.8,
        );
        let organization = entity("org1", "organization");
        let place = entity("place_to", "place");

        let result = gate(&claim, Some(&organization), Some(&place), &[]);

        assert!(matches!(result, MapGateResult::Reject(reason) if reason.contains("place")));
    }

    #[test]
    fn map_gate_uncertain_when_one_side_unresolved() {
        let claim = make_claim(
            "location_edge",
            location_edge_value("黑风谷", "青云城", "north_of"),
            "span1",
            0.8,
        );
        let to = entity("place_to", "place");

        let result = gate(&claim, None, Some(&to), &[]);

        assert!(matches!(result, MapGateResult::Uncertain(reason) if reason.contains("unresolved")));
    }

    #[test]
    fn map_gate_uncertain_when_confidence_is_weak() {
        let claim = make_claim(
            "location_edge",
            location_edge_value("黑风谷", "青云城", "north_of"),
            "span1",
            0.35,
        );
        let from = entity("place_from", "place");
        let to = entity("place_to", "place");

        let result = gate(&claim, Some(&from), Some(&to), &[]);

        assert!(matches!(result, MapGateResult::Uncertain(reason) if reason.contains("confidence")));
    }
}
