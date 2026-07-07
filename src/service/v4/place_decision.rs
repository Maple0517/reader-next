use crate::service::v4::ledger_payload;
use crate::service::v4::map_conflict_judge::{MapConflictDecision, MapConflictJudgeOutput};
use crate::service::v4::place_reducer::{
    PlaceWriteAction, PlaceWriteCommand, PlaceWriteConflictAction, PlaceWriteEdgeAction,
    PlaceWriteOrganizationLink, PlaceWritePlaceAction, PlaceWriteProvenance,
};
use crate::storage::db::v4::claim_repo::ClaimRecord;

#[derive(Debug, Clone)]
pub struct LedgerPlaceClaim {
    pub claim_id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub kind: LedgerPlaceClaimKind,
    pub primary_source_span_id: String,
    pub confidence: f64,
    pub status: String,
}

#[derive(Debug, Clone)]
pub enum LedgerPlaceClaimKind {
    Introduction {
        display_name: String,
        place_type: String,
        aliases: Vec<String>,
        parent_place_mention: Option<String>,
        scale_level: i64,
        importance_score: f64,
        map_visible: bool,
    },
    Edge {
        from_mention: String,
        to_mention: String,
        edge_type: String,
        direction_hint: Option<String>,
        distance_hint: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum PlaceDecision {
    Invalid(PlaceDecisionRejection),
    NoWrite(PlaceNoWrite),
    Quarantine(PlaceQuarantine),
    Write(PlaceWriteCommand),
}

#[derive(Debug, Clone)]
pub struct PlaceDecisionRejection {
    pub claim_id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct PlaceNoWrite {
    pub claim_id: String,
    pub status: PlaceNoWriteStatus,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceNoWriteStatus {
    Rejected,
    Uncertain,
}

#[derive(Debug, Clone)]
pub struct PlaceQuarantine {
    pub claim_id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct PlaceDecisionContext {
    pub resolution: PlaceResolutionForDecision,
    pub gate: PlaceGateDecision,
    pub judge_output: Option<MapConflictJudgeOutput>,
}

#[derive(Debug, Clone)]
pub enum PlaceResolutionForDecision {
    Place {
        resolved_place_entity_id: Option<String>,
        parent_place_id: Option<String>,
        organization_link: Option<PlaceWriteOrganizationLink>,
    },
    Edge {
        from_place_id: String,
        to_place_id: String,
    },
}

#[derive(Debug, Clone)]
pub enum PlaceGateDecision {
    Pass,
    Reject(String),
    Uncertain(String),
}

impl LedgerPlaceClaim {
    pub fn from_claim(claim: &ClaimRecord) -> anyhow::Result<Self> {
        ledger_payload::ensure_claim_type_in(
            claim,
            &["location_introduction", "location_edge"],
            "place claim",
        )?;
        let object = ledger_payload::claim_payload_object(claim, "place")?;
        ledger_payload::reject_forbidden_fields(
            claim,
            &object,
            "place",
            "judge field",
            &[
                "judge_decision",
                "normalized_edge_type",
                "normalized_direction_hint",
                "normalized_distance_hint",
                "conflict_type",
                "reason_code",
                "judge_confidence",
                "judge_output",
                "from_place_id",
                "to_place_id",
                "parent_place_id",
                "organization_link_candidate_id",
            ],
        )?;

        let kind = if claim.claim_type == "location_introduction" {
            LedgerPlaceClaimKind::Introduction {
                display_name: claim
                    .subject_mention
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        anyhow::anyhow!("location_introduction missing subject_mention")
                    })?,
                place_type: ledger_payload::optional_string(&object, "place_type")
                    .unwrap_or_else(|| "unknown".to_string()),
                aliases: ledger_payload::optional_string_array(&object, "aliases"),
                parent_place_mention: ledger_payload::optional_string(
                    &object,
                    "parent_place_mention",
                ),
                scale_level: object
                    .get("scale_level")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
                importance_score: object
                    .get("importance_score")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(claim.confidence.clamp(0.0, 1.0)),
                map_visible: object
                    .get("map_visible_hint")
                    .or_else(|| object.get("map_visible"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true),
            }
        } else {
            LedgerPlaceClaimKind::Edge {
                from_mention: claim
                    .subject_mention
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("location_edge missing subject_mention"))?,
                to_mention: claim
                    .object_mention
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("location_edge missing object_mention"))?,
                edge_type: ledger_payload::optional_string(&object, "edge_type")
                    .ok_or_else(|| anyhow::anyhow!("location_edge missing edge_type"))?,
                direction_hint: ledger_payload::optional_string(&object, "direction_hint"),
                distance_hint: ledger_payload::optional_string(&object, "distance_hint"),
            }
        };

        Ok(Self {
            claim_id: claim.id.clone(),
            book_id: claim.book_id.clone(),
            chapter_index: claim.chapter_index,
            kind,
            primary_source_span_id: claim.primary_source_span_id.clone(),
            confidence: claim.confidence,
            status: claim.status.clone(),
        })
    }
}

pub fn materialize_place_decision(
    claim: LedgerPlaceClaim,
    context: PlaceDecisionContext,
) -> anyhow::Result<PlaceDecision> {
    if claim.status != "proposed" {
        return Ok(PlaceDecision::NoWrite(PlaceNoWrite {
            claim_id: claim.claim_id,
            status: PlaceNoWriteStatus::Uncertain,
            reason_code: format!("ledger_status_{}", claim.status),
        }));
    }

    match context.gate {
        PlaceGateDecision::Pass => {}
        PlaceGateDecision::Reject(reason) => {
            return Ok(PlaceDecision::NoWrite(PlaceNoWrite {
                claim_id: claim.claim_id,
                status: PlaceNoWriteStatus::Rejected,
                reason_code: reason,
            }));
        }
        PlaceGateDecision::Uncertain(reason) => {
            return Ok(PlaceDecision::NoWrite(PlaceNoWrite {
                claim_id: claim.claim_id,
                status: PlaceNoWriteStatus::Uncertain,
                reason_code: reason,
            }));
        }
    }

    if let Some(judge_output) = &context.judge_output {
        match judge_output.decision {
            MapConflictDecision::Reject => {
                return Ok(PlaceDecision::NoWrite(PlaceNoWrite {
                    claim_id: claim.claim_id,
                    status: PlaceNoWriteStatus::Rejected,
                    reason_code: judge_output.reason_code.clone(),
                }));
            }
            MapConflictDecision::Uncertain => {
                return Ok(PlaceDecision::NoWrite(PlaceNoWrite {
                    claim_id: claim.claim_id,
                    status: PlaceNoWriteStatus::Uncertain,
                    reason_code: judge_output.reason_code.clone(),
                }));
            }
            MapConflictDecision::Accept | MapConflictDecision::Conflict => {}
        }
    }

    let claim_id = claim.claim_id.clone();
    match place_write_command_from_decision(claim, context) {
        Ok(command) => Ok(PlaceDecision::Write(command)),
        Err(err) => Ok(PlaceDecision::Quarantine(PlaceQuarantine {
            claim_id,
            reason: err.to_string(),
        })),
    }
}

fn place_write_command_from_decision(
    claim: LedgerPlaceClaim,
    context: PlaceDecisionContext,
) -> anyhow::Result<PlaceWriteCommand> {
    let provenance = PlaceWriteProvenance {
        claim_id: claim.claim_id,
        evidence_span_ids: vec![claim.primary_source_span_id],
        confidence: claim.confidence,
    };
    let action = match claim.kind {
        LedgerPlaceClaimKind::Introduction {
            display_name,
            place_type,
            aliases,
            parent_place_mention: _,
            scale_level,
            importance_score,
            map_visible,
        } => {
            let PlaceResolutionForDecision::Place {
                resolved_place_entity_id,
                parent_place_id,
                organization_link,
            } = context.resolution
            else {
                anyhow::bail!("place introduction decision missing place resolution");
            };
            PlaceWriteAction::UpsertPlace(PlaceWritePlaceAction {
                resolved_place_entity_id,
                display_name,
                place_type,
                aliases,
                parent_place_id,
                organization_link,
                scale_level,
                importance_score,
                map_visible,
            })
        }
        LedgerPlaceClaimKind::Edge {
            from_mention: _,
            to_mention: _,
            edge_type,
            direction_hint,
            distance_hint,
        } => {
            let PlaceResolutionForDecision::Edge {
                from_place_id,
                to_place_id,
            } = context.resolution
            else {
                anyhow::bail!("location edge decision missing edge resolution");
            };
            let Some(judge_output) = context.judge_output else {
                anyhow::bail!("location edge decision missing map judge output");
            };
            match judge_output.decision {
                MapConflictDecision::Accept => PlaceWriteAction::UpsertEdge(PlaceWriteEdgeAction {
                    from_place_id,
                    to_place_id,
                    edge_type: judge_output.normalized_edge_type.unwrap_or(edge_type),
                    direction_hint: judge_output.normalized_direction_hint.or(direction_hint),
                    distance_hint: judge_output.normalized_distance_hint.or(distance_hint),
                    confidence: judge_output.confidence,
                    allow_parent_change: true,
                }),
                MapConflictDecision::Conflict => {
                    PlaceWriteAction::RecordConflict(PlaceWriteConflictAction {
                        existing_edge_id: None,
                        conflict_type: judge_output
                            .conflict_type
                            .unwrap_or_else(|| "unknown_conflict".to_string()),
                        reason_code: judge_output.reason_code,
                    })
                }
                MapConflictDecision::Reject => {
                    anyhow::bail!("map reject decision cannot become a write command");
                }
                MapConflictDecision::Uncertain => {
                    anyhow::bail!("map uncertain decision cannot become a write command");
                }
            }
        }
    };

    Ok(PlaceWriteCommand {
        book_id: claim.book_id,
        chapter_index: claim.chapter_index,
        provenance,
        action,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::map_conflict_judge::{MapConflictDecision, MapConflictJudgeOutput};
    use crate::service::v4::place_reducer::PlaceWriteAction;

    fn location_edge_claim(value_json: serde_json::Value) -> ClaimRecord {
        ClaimRecord {
            id: "claim-edge-1".to_string(),
            book_id: "book1".to_string(),
            chapter_index: 7,
            claim_type: "location_edge".to_string(),
            subject_mention: Some("青云城".to_string()),
            object_mention: Some("黑风谷".to_string()),
            subject_entity_id: None,
            object_entity_id: None,
            predicate: "青云城 north_of 黑风谷".to_string(),
            value_json: Some(value_json.to_string()),
            value_text: None,
            primary_source_span_id: "span1".to_string(),
            ai_run_id: "run1".to_string(),
            confidence: 0.86,
            risk_level: "medium".to_string(),
            status: "proposed".to_string(),
            supersedes_claim_id: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    #[test]
    fn place_ledger_claim_rejects_map_judge_fields() {
        let claim = location_edge_claim(serde_json::json!({
            "edge_type": "north_of",
            "direction_hint": null,
            "distance_hint": null,
            "is_topological_hint": true,
            "evidence_span_ids": ["span1"],
            "judge_decision": "accept"
        }));

        let err = LedgerPlaceClaim::from_claim(&claim).unwrap_err();

        assert!(err.to_string().contains("judge field judge_decision"));
    }

    #[test]
    fn place_materializer_keeps_map_judge_output_inside_decision_layer() {
        let claim = location_edge_claim(serde_json::json!({
            "edge_type": "north_of",
            "direction_hint": "北面",
            "distance_hint": "三日路程",
            "is_topological_hint": true,
            "evidence_span_ids": ["span1"]
        }));
        let ledger_claim = LedgerPlaceClaim::from_claim(&claim).unwrap();
        let judge_output = MapConflictJudgeOutput {
            decision: MapConflictDecision::Accept,
            normalized_edge_type: Some("north_of".to_string()),
            normalized_direction_hint: Some("北面".to_string()),
            normalized_distance_hint: Some("三日路程".to_string()),
            conflict_type: None,
            reason_code: "accepted_topology".to_string(),
            confidence: 0.91,
            explanation_for_log: "valid topology".to_string(),
        };

        let decision = materialize_place_decision(
            ledger_claim,
            PlaceDecisionContext {
                resolution: PlaceResolutionForDecision::Edge {
                    from_place_id: "place-from".to_string(),
                    to_place_id: "place-to".to_string(),
                },
                gate: PlaceGateDecision::Pass,
                judge_output: Some(judge_output),
            },
        )
        .unwrap();

        let PlaceDecision::Write(command) = decision else {
            panic!("expected write decision");
        };
        let PlaceWriteAction::UpsertEdge(edge) = command.action else {
            panic!("expected edge command");
        };
        assert_eq!(command.provenance.claim_id, claim.id);
        assert_eq!(edge.from_place_id, "place-from");
        assert_eq!(edge.to_place_id, "place-to");
        assert_eq!(edge.edge_type, "north_of");
        assert_eq!(edge.confidence, 0.91);
    }

    #[test]
    fn place_materializer_turns_map_reject_into_no_write_rejected() {
        let claim = location_edge_claim(serde_json::json!({
            "edge_type": "north_of",
            "is_topological_hint": true,
            "evidence_span_ids": ["span1"]
        }));
        let ledger_claim = LedgerPlaceClaim::from_claim(&claim).unwrap();
        let judge_output = MapConflictJudgeOutput {
            decision: MapConflictDecision::Reject,
            normalized_edge_type: None,
            normalized_direction_hint: None,
            normalized_distance_hint: None,
            conflict_type: None,
            reason_code: "not_topology".to_string(),
            confidence: 0.8,
            explanation_for_log: "not a map fact".to_string(),
        };

        let decision = materialize_place_decision(
            ledger_claim,
            PlaceDecisionContext {
                resolution: PlaceResolutionForDecision::Edge {
                    from_place_id: "place-from".to_string(),
                    to_place_id: "place-to".to_string(),
                },
                gate: PlaceGateDecision::Pass,
                judge_output: Some(judge_output),
            },
        )
        .unwrap();

        let PlaceDecision::NoWrite(no_write) = decision else {
            panic!("expected no-write rejected decision");
        };
        assert_eq!(no_write.status, PlaceNoWriteStatus::Rejected);
        assert_eq!(no_write.reason_code, "not_topology");
    }

    #[test]
    fn place_materializer_records_conflict_without_raw_judge_payload() {
        let claim = location_edge_claim(serde_json::json!({
            "edge_type": "north_of",
            "is_topological_hint": true,
            "evidence_span_ids": ["span1"]
        }));
        let ledger_claim = LedgerPlaceClaim::from_claim(&claim).unwrap();
        let judge_output = MapConflictJudgeOutput {
            decision: MapConflictDecision::Conflict,
            normalized_edge_type: Some("north_of".to_string()),
            normalized_direction_hint: Some("北面".to_string()),
            normalized_distance_hint: None,
            conflict_type: Some("duplicate_conflicting_direction".to_string()),
            reason_code: "opposite_direction".to_string(),
            confidence: 0.77,
            explanation_for_log: "opposite edge already exists".to_string(),
        };

        let decision = materialize_place_decision(
            ledger_claim,
            PlaceDecisionContext {
                resolution: PlaceResolutionForDecision::Edge {
                    from_place_id: "place-from".to_string(),
                    to_place_id: "place-to".to_string(),
                },
                gate: PlaceGateDecision::Pass,
                judge_output: Some(judge_output),
            },
        )
        .unwrap();

        let PlaceDecision::Write(command) = decision else {
            panic!("expected write decision");
        };
        let PlaceWriteAction::RecordConflict(conflict) = command.action else {
            panic!("expected conflict command");
        };
        assert_eq!(conflict.conflict_type, "duplicate_conflicting_direction");
        assert_eq!(conflict.reason_code, "opposite_direction");

        let decision_source = include_str!("place_decision.rs");
        let raw_judge_serialization = concat!("serde_json::to_string", "(&judge_output)");
        assert!(
            !decision_source.contains(raw_judge_serialization),
            "raw map judge output must stay inside decision and not be serialized into a reducer command"
        );
    }
}
