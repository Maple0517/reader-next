use crate::service::v4::claim_lifecycle::{
    accept_claim, mark_uncertain_claim, quarantine_claim, reject_claim,
};
use crate::service::v4::map_conflict_judge::{MapConflictJudge, MapConflictJudgeInput};
use crate::service::v4::map_gate::{self, MapGateContext, MapGateResult};
use crate::service::v4::place_decision::{
    self, LedgerPlaceClaim, LedgerPlaceClaimKind, PlaceDecision, PlaceDecisionContext,
    PlaceGateDecision, PlaceNoWriteStatus, PlaceResolutionForDecision,
};
use crate::service::v4::place_reducer;
use crate::service::v4::place_resolver::{PlaceResolutionAction, PlaceResolver};
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::identity_repo::IdentityRepo;
use sqlx::SqlitePool;

#[derive(Debug, Default)]
pub struct PlaceSegmentProcessResult {
    pub claims_accepted: usize,
    pub claims_rejected: usize,
    pub claims_quarantined: usize,
    pub claims_uncertain: usize,
}

pub(crate) async fn process_place_claims_for_segment(
    book_id: &str,
    chapter_index: i64,
    pool: &SqlitePool,
    claim_repo: &ClaimRepo,
    _segment_id: &str,
    location_claims: &[ClaimRecord],
    map_judge: &dyn MapConflictJudge,
) -> anyhow::Result<PlaceSegmentProcessResult> {
    let entity_repo = EntityRepo::new(pool.clone());
    let identity_repo = IdentityRepo::new(pool.clone());
    let place_resolver = PlaceResolver::new(EntityRepo::new(pool.clone()), identity_repo);
    let existing_parent_links = load_existing_parent_links(book_id, pool).await?;
    let mut result = PlaceSegmentProcessResult::default();

    for claim in location_claims {
        let ledger_claim = match LedgerPlaceClaim::from_claim(claim) {
            Ok(ledger_claim) => ledger_claim,
            Err(err) => {
                tracing::debug!("Place claim {} rejected: {}", claim.id, err);
                reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
                continue;
            }
        };

        match &ledger_claim.kind {
            LedgerPlaceClaimKind::Introduction {
                display_name,
                place_type,
                aliases,
                parent_place_mention,
                ..
            } => {
                let resolution = place_resolver
                    .resolve_place(
                        book_id,
                        display_name,
                        place_type,
                        parent_place_mention.as_deref(),
                        aliases,
                        ledger_claim.confidence,
                    )
                    .await?;
                if resolution.action == PlaceResolutionAction::Uncertain {
                    mark_uncertain_claim(claim_repo, &claim.id, &mut result.claims_uncertain)
                        .await?;
                    continue;
                }
                let organization_link =
                    resolution
                        .organization_link_candidate_id
                        .map(
                            |organization_entity_id| place_reducer::PlaceWriteOrganizationLink {
                                organization_entity_id,
                                link_type: "organization_place_pair".to_string(),
                            },
                        );
                let decision = place_decision::materialize_place_decision(
                    ledger_claim,
                    PlaceDecisionContext {
                        resolution: PlaceResolutionForDecision::Place {
                            resolved_place_entity_id: resolution.place_entity_id,
                            parent_place_id: resolution.parent_place_id,
                            organization_link,
                        },
                        gate: PlaceGateDecision::Pass,
                        judge_output: None,
                    },
                )?;
                apply_place_decision_lifecycle(pool, claim_repo, decision, &mut result).await?;
            }
            LedgerPlaceClaimKind::Edge {
                from_mention,
                to_mention,
                edge_type,
                ..
            } => {
                let from_resolution = place_resolver
                    .resolve_place(
                        book_id,
                        from_mention,
                        "unknown",
                        None,
                        &[],
                        ledger_claim.confidence,
                    )
                    .await?;
                let to_resolution = place_resolver
                    .resolve_place(
                        book_id,
                        to_mention,
                        "unknown",
                        None,
                        &[],
                        ledger_claim.confidence,
                    )
                    .await?;
                let (Some(from_place_id), Some(to_place_id)) = (
                    from_resolution.place_entity_id.clone(),
                    to_resolution.place_entity_id.clone(),
                ) else {
                    mark_uncertain_claim(claim_repo, &claim.id, &mut result.claims_uncertain)
                        .await?;
                    continue;
                };

                let mut resolved_claim = claim.clone();
                resolved_claim.subject_entity_id = Some(from_place_id.clone());
                resolved_claim.object_entity_id = Some(to_place_id.clone());
                resolved_claim.value_json = Some(gate_value_json_with_evidence(&resolved_claim)?);
                let from_place = entity_repo.get_by_id(&from_place_id).await?;
                let to_place = entity_repo.get_by_id(&to_place_id).await?;
                let gate = match map_gate::structural_gate(MapGateContext {
                    claim: &resolved_claim,
                    from_place: from_place.as_ref(),
                    to_place: to_place.as_ref(),
                    existing_parent_links: &existing_parent_links,
                }) {
                    MapGateResult::Pass => PlaceGateDecision::Pass,
                    MapGateResult::Reject(reason) => PlaceGateDecision::Reject(reason),
                    MapGateResult::Uncertain(reason) => PlaceGateDecision::Uncertain(reason),
                };

                let judge_output = if matches!(gate, PlaceGateDecision::Pass) {
                    Some(
                        map_judge
                            .judge(&MapConflictJudgeInput {
                                book_id: book_id.to_string(),
                                chapter_index,
                                claim_id: claim.id.clone(),
                                claim_type: claim.claim_type.clone(),
                                from_place_mention: from_mention.clone(),
                                to_place_mention: to_mention.clone(),
                                edge_type: edge_type.clone(),
                                evidence_spans: claim_evidence_span_ids(claim),
                                existing_map_context: load_existing_map_context(book_id, pool)
                                    .await?,
                                boundary_warnings: Vec::new(),
                            })
                            .await?,
                    )
                } else {
                    None
                };

                let decision = place_decision::materialize_place_decision(
                    ledger_claim,
                    PlaceDecisionContext {
                        resolution: PlaceResolutionForDecision::Edge {
                            from_place_id,
                            to_place_id,
                        },
                        gate,
                        judge_output,
                    },
                )?;
                apply_place_decision_lifecycle(pool, claim_repo, decision, &mut result).await?;
            }
        }
    }

    Ok(result)
}

async fn apply_place_decision_lifecycle(
    pool: &SqlitePool,
    claim_repo: &ClaimRepo,
    decision: PlaceDecision,
    result: &mut PlaceSegmentProcessResult,
) -> anyhow::Result<()> {
    match decision {
        PlaceDecision::Write(command) => {
            let claim_id = command.provenance.claim_id.clone();
            match place_reducer::apply_place_write(command, pool).await {
                Ok(reduction) => {
                    for claim_id in reduction.claims_accepted {
                        accept_claim(claim_repo, &claim_id, &mut result.claims_accepted).await?;
                    }
                    for claim_id in reduction.claims_rejected {
                        reject_claim(claim_repo, &claim_id, &mut result.claims_rejected).await?;
                    }
                    for claim_id in reduction.claims_skipped {
                        mark_uncertain_claim(claim_repo, &claim_id, &mut result.claims_uncertain)
                            .await?;
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        "Place command for claim {} failed: {}. Marking uncertain.",
                        claim_id,
                        err
                    );
                    mark_uncertain_claim(claim_repo, &claim_id, &mut result.claims_uncertain)
                        .await?;
                }
            }
        }
        PlaceDecision::NoWrite(no_write) => match no_write.status {
            PlaceNoWriteStatus::Rejected => {
                reject_claim(claim_repo, &no_write.claim_id, &mut result.claims_rejected).await?;
            }
            PlaceNoWriteStatus::Uncertain => {
                mark_uncertain_claim(claim_repo, &no_write.claim_id, &mut result.claims_uncertain)
                    .await?;
            }
        },
        PlaceDecision::Invalid(rejection) => {
            tracing::warn!(
                "Place decision rejected claim {}: {}",
                rejection.claim_id,
                rejection.reason
            );
            reject_claim(claim_repo, &rejection.claim_id, &mut result.claims_rejected).await?;
        }
        PlaceDecision::Quarantine(quarantine) => {
            tracing::warn!(
                "Place decision quarantined claim {}: {}",
                quarantine.claim_id,
                quarantine.reason
            );
            quarantine_claim(
                pool,
                &quarantine.claim_id,
                &quarantine.reason,
                &mut result.claims_quarantined,
            )
            .await?;
        }
    }

    Ok(())
}

async fn load_existing_parent_links(
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<Vec<(String, String)>> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT entity_id, parent_place_id
         FROM place_details
         WHERE book_id = ? AND status = 'active' AND parent_place_id IS NOT NULL",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn load_existing_map_context(
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<Vec<String>> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT from_place_id, to_place_id, edge_type
         FROM place_edges
         WHERE book_id = ? AND status = 'active'
         ORDER BY created_at DESC
         LIMIT 20",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(from, to, edge_type)| format!("{from} {edge_type} {to}"))
        .collect())
}

fn claim_evidence_span_ids(claim: &ClaimRecord) -> Vec<String> {
    let primary = claim.primary_source_span_id.trim();
    if primary.is_empty() {
        Vec::new()
    } else {
        vec![primary.to_string()]
    }
}

fn gate_value_json_with_evidence(claim: &ClaimRecord) -> anyhow::Result<String> {
    let mut value = claim
        .value_json
        .as_deref()
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()?
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let has_evidence = value
        .get("evidence_span_ids")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|ids| {
            ids.iter()
                .any(|id| id.as_str().is_some_and(|value| !value.trim().is_empty()))
        });
    if !has_evidence {
        value.insert(
            "evidence_span_ids".to_string(),
            serde_json::Value::Array(
                claim_evidence_span_ids(claim)
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    Ok(serde_json::Value::Object(value).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::map_conflict_judge::{
        MapConflictDecision, MapConflictJudgeOutput, MockMapConflictJudge,
    };
    use crate::service::v4::test_support::setup_v4_processor_test;
    use crate::storage::db::v4::claim_repo::ClaimRepo;
    use crate::storage::db::v4::entity_repo::EntityRepo;
    use crate::storage::db::v4::place_repo::PlaceRepo;
    use crate::storage::db::v4::quality_repo::QualityRepo;
    use sqlx::SqlitePool;

    async fn setup() -> (
        SqlitePool,
        ClaimRepo,
        EntityRepo,
        PlaceRepo,
        String,
        String,
        String,
    ) {
        let ctx =
            setup_v4_processor_test("place", 9, "青云城通往黑风谷。", "青云城通往黑风谷").await;

        (
            ctx.pool.clone(),
            ClaimRepo::new(ctx.pool.clone()),
            EntityRepo::new(ctx.pool.clone()),
            PlaceRepo::new(ctx.pool.clone()),
            ctx.segment_id,
            ctx.span_id,
            ctx.run_id,
        )
    }

    async fn create_place(
        entity_repo: &EntityRepo,
        place_repo: &PlaceRepo,
        name: &str,
        place_type: &str,
    ) -> String {
        let entity = entity_repo
            .create_entity("b1", "place", name, name, None, 0.7, 1)
            .await
            .unwrap();
        place_repo
            .upsert_place_detail(
                "b1", &entity.id, place_type, None, 0, 0.7, true, 1, 1, "active",
            )
            .await
            .unwrap();
        entity.id
    }

    #[tokio::test]
    async fn place_segment_processor_applies_typed_command_without_mutating_claim_value_json() {
        let (pool, claim_repo, entity_repo, place_repo, segment_id, span_id, run_id) =
            setup().await;
        let from_id = create_place(&entity_repo, &place_repo, "青云城", "city").await;
        let to_id = create_place(&entity_repo, &place_repo, "黑风谷", "dungeon").await;
        assert!(!from_id.is_empty());
        assert!(!to_id.is_empty());

        let original_value_json = r#"{
            "edge_type":"route_to",
            "direction_hint":"北面",
            "distance_hint":"三日路程",
            "is_topological_hint":true,
            "evidence_span_ids":["span1"]
        }"#;
        let claim = claim_repo
            .create_claim(
                "b1",
                9,
                "location_edge",
                Some("青云城"),
                Some("黑风谷"),
                None,
                None,
                "location_edge",
                None,
                Some(original_value_json),
                &span_id,
                &run_id,
                0.9,
                "medium",
            )
            .await
            .unwrap();
        let judge = MockMapConflictJudge::new(MapConflictJudgeOutput {
            decision: MapConflictDecision::Accept,
            normalized_edge_type: Some("route_to".to_string()),
            normalized_direction_hint: Some("北面".to_string()),
            normalized_distance_hint: Some("三日路程".to_string()),
            conflict_type: None,
            reason_code: "accepted_topology".to_string(),
            confidence: 0.94,
            explanation_for_log: "valid route".to_string(),
        });

        let result = process_place_claims_for_segment(
            "b1",
            9,
            &pool,
            &claim_repo,
            &segment_id,
            &[claim.clone()],
            &judge,
        )
        .await
        .unwrap();

        assert_eq!(result.claims_accepted, 1);
        let stored = claim_repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(stored.status, "accepted");
        assert_eq!(stored.value_json.as_deref(), Some(original_value_json));
        let edges: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM place_edges WHERE book_id = 'b1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(edges.0, 1);
    }

    #[tokio::test]
    async fn place_quarantine_decision_marks_claim_quarantined() {
        let (pool, claim_repo, _entity_repo, _place_repo, _segment_id, span_id, run_id) =
            setup().await;
        let claim = claim_repo
            .create_claim(
                "b1",
                9,
                "location_edge",
                Some("青云城"),
                Some("黑风谷"),
                None,
                None,
                "location_edge",
                None,
                Some(r#"{"edge_type":"route_to","is_topological_hint":true}"#),
                &span_id,
                &run_id,
                0.9,
                "high",
            )
            .await
            .unwrap();
        let mut result = PlaceSegmentProcessResult::default();

        apply_place_decision_lifecycle(
            &pool,
            &claim_repo,
            PlaceDecision::Quarantine(place_decision::PlaceQuarantine {
                claim_id: claim.id.clone(),
                reason: "missing edge resolution".to_string(),
            }),
            &mut result,
        )
        .await
        .unwrap();

        assert_eq!(result.claims_quarantined, 1);
        assert_eq!(result.claims_uncertain, 0);
        let stored = claim_repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(stored.status, "quarantined");
        let workflow = QualityRepo::new(pool)
            .get_quarantined_claim_by_claim_id("b1", &claim.id)
            .await
            .unwrap()
            .expect("quarantine workflow should be created");
        assert_eq!(workflow.status, "open");
        assert_eq!(workflow.suggested_action, "needs_manual_review");
    }
}
