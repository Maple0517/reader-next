use crate::service::v4::claim_lifecycle::{
    accept_claim, mark_uncertain_claim, quarantine_claim, reject_claim,
};
use crate::service::v4::identity_decision::{self, IdentityDecision, IdentityNoWriteStatus};
use crate::service::v4::identity_judge::{self, IdentityGateResult, IdentityJudge};
use crate::service::v4::reducer;
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::identity_repo::{IdentityLinkRecord, IdentityRepo};
use sqlx::SqlitePool;

#[derive(Debug, Default)]
pub struct IdentitySegmentProcessResult {
    pub claims_accepted: usize,
    pub claims_rejected: usize,
    pub claims_quarantined: usize,
    pub claims_uncertain: usize,
}

pub(crate) async fn process_identity_claims_for_segment(
    book_id: &str,
    pool: &SqlitePool,
    claim_repo: &ClaimRepo,
    entity_repo: &EntityRepo,
    identity_claims: &[ClaimRecord],
    judge: &dyn IdentityJudge,
) -> anyhow::Result<IdentitySegmentProcessResult> {
    let identity_repo = IdentityRepo::new(pool.clone());
    let active_identity_links = identity_repo
        .list_identity_links_by_book(book_id, Some("active"))
        .await
        .unwrap_or_default();
    let mut result = IdentitySegmentProcessResult::default();

    for claim in identity_claims {
        let claim = resolve_identity_claim_entities(book_id, entity_repo, claim).await?;
        persist_claim_entity_ids(pool, &claim).await?;

        let entity_a = match &claim.subject_entity_id {
            Some(id) => entity_repo.get_by_id(id).await?,
            None => None,
        };
        let entity_b = match &claim.object_entity_id {
            Some(id) => entity_repo.get_by_id(id).await?,
            None => None,
        };
        let blocked = blocked_by_active_not_same_identity(
            &active_identity_links,
            claim.subject_entity_id.as_deref(),
            claim.object_entity_id.as_deref(),
        );

        match identity_judge::structural_gate(&claim, entity_a.as_ref(), entity_b.as_ref(), blocked)
        {
            IdentityGateResult::Pass => match judge.judge(&claim).await {
                Ok(output) => {
                    let decision = identity_decision::LedgerIdentityClaim::from_claim(&claim).map(
                        |ledger_claim| {
                            identity_decision::materialize_identity_decision(ledger_claim, output)
                        },
                    );
                    match decision {
                        Ok(Ok(decision)) => {
                            apply_identity_decision_lifecycle(
                                pool,
                                claim_repo,
                                decision,
                                &mut result,
                            )
                            .await?;
                        }
                        Ok(Err(err)) | Err(err) => {
                            tracing::warn!(
                                "Identity claim {} could not materialize decision: {}",
                                claim.id,
                                err
                            );
                            reject_claim(claim_repo, &claim.id, &mut result.claims_rejected)
                                .await?;
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        "Identity judge failed for claim {}: {}. Marking rejected.",
                        claim.id,
                        err
                    );
                    reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
                }
            },
            IdentityGateResult::Reject(reason) => {
                tracing::debug!("Identity claim {} rejected: {}", claim.id, reason);
                reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
            }
            IdentityGateResult::Uncertain(reason) => {
                tracing::debug!("Identity claim {} uncertain: {}", claim.id, reason);
                mark_uncertain_claim(claim_repo, &claim.id, &mut result.claims_uncertain).await?;
            }
        }
    }

    Ok(result)
}

async fn apply_identity_decision_lifecycle(
    pool: &SqlitePool,
    claim_repo: &ClaimRepo,
    decision: IdentityDecision,
    result: &mut IdentitySegmentProcessResult,
) -> anyhow::Result<()> {
    match decision {
        IdentityDecision::Write(command) => {
            let claim_id = identity_command_claim_id(&command);
            match reducer::apply_identity_write(command, pool).await {
                Ok(_) => {
                    accept_claim(claim_repo, &claim_id, &mut result.claims_accepted).await?;
                }
                Err(err) => {
                    tracing::warn!(
                        "Identity command for claim {} failed: {}. Marking uncertain.",
                        claim_id,
                        err
                    );
                    mark_uncertain_claim(claim_repo, &claim_id, &mut result.claims_uncertain)
                        .await?;
                }
            }
        }
        IdentityDecision::NoWrite(no_write) => match no_write.status {
            IdentityNoWriteStatus::Rejected => {
                reject_claim(claim_repo, &no_write.claim_id, &mut result.claims_rejected).await?;
            }
            IdentityNoWriteStatus::Uncertain => {
                mark_uncertain_claim(claim_repo, &no_write.claim_id, &mut result.claims_uncertain)
                    .await?;
            }
        },
        IdentityDecision::Invalid(rejection) => {
            tracing::warn!(
                "Identity decision rejected claim {}: {}",
                rejection.claim_id,
                rejection.reason
            );
            reject_claim(claim_repo, &rejection.claim_id, &mut result.claims_rejected).await?;
        }
        IdentityDecision::Quarantine(quarantine) => {
            tracing::warn!(
                "Identity decision quarantined claim {}: {}",
                quarantine.claim_id,
                quarantine.reason
            );
            quarantine_claim(
                claim_repo,
                &quarantine.claim_id,
                &mut result.claims_quarantined,
            )
            .await?;
        }
    }

    Ok(())
}

fn identity_command_claim_id(command: &reducer::IdentityWriteCommand) -> String {
    match command {
        reducer::IdentityWriteCommand::Merge { provenance, .. }
        | reducer::IdentityWriteCommand::Link { provenance, .. } => provenance.claim_id.clone(),
    }
}

async fn resolve_identity_claim_entities(
    book_id: &str,
    entity_repo: &EntityRepo,
    claim: &ClaimRecord,
) -> anyhow::Result<ClaimRecord> {
    let mut claim = claim.clone();
    if claim.subject_entity_id.is_none() {
        if let Some(ref mention) = claim.subject_mention {
            if let Some(entity) = entity_repo.find_entity_by_alias(book_id, mention).await? {
                claim.subject_entity_id = Some(entity.id);
            }
        }
    }
    if claim.object_entity_id.is_none() {
        if let Some(ref mention) = claim.object_mention {
            if let Some(entity) = entity_repo.find_entity_by_alias(book_id, mention).await? {
                claim.object_entity_id = Some(entity.id);
            }
        }
    }
    Ok(claim)
}

async fn persist_claim_entity_ids(pool: &SqlitePool, claim: &ClaimRecord) -> anyhow::Result<()> {
    sqlx::query("UPDATE claims SET subject_entity_id = ?, object_entity_id = ? WHERE id = ?")
        .bind(claim.subject_entity_id.as_deref())
        .bind(claim.object_entity_id.as_deref())
        .bind(&claim.id)
        .execute(pool)
        .await?;
    Ok(())
}

fn blocked_by_active_not_same_identity(
    links: &[IdentityLinkRecord],
    left: Option<&str>,
    right: Option<&str>,
) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    let pair = canonicalize_identity_pair(left, right);
    links.iter().any(|link| {
        link.link_type == "not_same_identity"
            && link.status == "active"
            && canonicalize_identity_pair(&link.entity_a_id, &link.entity_b_id) == pair
    })
}

fn canonicalize_identity_pair(left: &str, right: &str) -> (String, String) {
    if left <= right {
        (left.to_string(), right.to_string())
    } else {
        (right.to_string(), left.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::identity_judge::{
        IdentityJudgeDecision, IdentityJudgeOutput, MockIdentityJudge,
    };
    use crate::service::v4::test_support::setup_v4_processor_test;
    use crate::storage::db::v4::claim_repo::ClaimRepo;
    use crate::storage::db::v4::entity_repo::EntityRepo;
    use sqlx::SqlitePool;

    async fn setup() -> (SqlitePool, ClaimRepo, EntityRepo, String, String) {
        let ctx = setup_v4_processor_test("identity", 9, "text", "identity text").await;

        (
            ctx.pool.clone(),
            ClaimRepo::new(ctx.pool.clone()),
            EntityRepo::new(ctx.pool.clone()),
            ctx.span_id,
            ctx.run_id,
        )
    }

    #[tokio::test]
    async fn identity_segment_processor_applies_typed_merge_without_mutating_claim_value_json() {
        let (pool, claim_repo, entity_repo, span_id, run_id) = setup().await;
        let victim = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.6, 2)
            .await
            .unwrap();
        let survivor = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        let original_value_json = r#"{"source_observation":"identity_reveal"}"#;
        let claim = claim_repo
            .create_claim(
                "b1",
                9,
                "identity_reveal",
                Some("黑衣人"),
                Some("张三"),
                Some(&victim.id),
                Some(&survivor.id),
                "identity reveal",
                None,
                Some(original_value_json),
                &span_id,
                &run_id,
                0.9,
                "high",
            )
            .await
            .unwrap();
        let judge = MockIdentityJudge::new(IdentityJudgeOutput {
            decision: IdentityJudgeDecision::Merge,
            link_type: "same_identity".to_string(),
            survivor_hint: "entity_b".to_string(),
            confidence: 0.93,
            reason_code: "explicit_reveal".to_string(),
            explanation_for_log: "same person".to_string(),
            property_conflicts: vec![],
            relationship_migration_hint: "safe".to_string(),
        });

        let result = process_identity_claims_for_segment(
            "b1",
            &pool,
            &claim_repo,
            &entity_repo,
            &[claim.clone()],
            &judge,
        )
        .await
        .unwrap();

        assert_eq!(result.claims_accepted, 1);
        let stored = claim_repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(stored.status, "accepted");
        assert_eq!(stored.value_json.as_deref(), Some(original_value_json));
        assert_eq!(
            entity_repo
                .get_by_id(&victim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "merged"
        );
    }

    #[tokio::test]
    async fn identity_quarantine_decision_marks_claim_quarantined() {
        let (pool, claim_repo, _entity_repo, span_id, run_id) = setup().await;
        let claim = claim_repo
            .create_claim(
                "b1",
                9,
                "identity_reveal",
                Some("黑衣人"),
                Some("张三"),
                None,
                None,
                "identity reveal",
                None,
                Some(r#"{"identity_kind":"same_identity","reason_hint":"摘下面具"}"#),
                &span_id,
                &run_id,
                0.9,
                "high",
            )
            .await
            .unwrap();
        let mut result = IdentitySegmentProcessResult::default();

        apply_identity_decision_lifecycle(
            &pool,
            &claim_repo,
            IdentityDecision::Quarantine(identity_decision::IdentityQuarantine {
                claim_id: claim.id.clone(),
                reason: "missing resolved pair".to_string(),
            }),
            &mut result,
        )
        .await
        .unwrap();

        assert_eq!(result.claims_quarantined, 1);
        assert_eq!(result.claims_uncertain, 0);
        let stored = claim_repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(stored.status, "quarantined");
    }
}
