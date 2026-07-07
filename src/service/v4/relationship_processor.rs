use crate::service::v4::claim_lifecycle::{
    accept_claim, mark_uncertain_claim, quarantine_claim, redirect_claim, reject_claim,
};
use crate::service::v4::relationship_decision::{
    self, RelationshipDecision, RelationshipGateDecision, RelationshipNoWriteStatus,
};
use crate::service::v4::relationship_judge::{JudgeDecision, JudgeOutput};
use crate::service::v4::{character_processor, reducer};
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
use crate::storage::db::v4::entity_repo::EntityRepo;
use sqlx::SqlitePool;

/// Trait for making relationship judge decisions.
///
/// Implementations:
/// - `DefaultJudge`: placeholder that marks all relationship claims as uncertain.
/// - `MockJudge`: deterministic test judge.
/// - Real AI judge: `relationship_judge::RealAiRelationshipJudge`.
#[axum::async_trait]
pub trait Judge: Send + Sync {
    async fn judge(&self, claim: &ClaimRecord) -> anyhow::Result<JudgeOutput>;
}

#[derive(Default)]
pub struct DefaultJudge;

impl DefaultJudge {
    pub fn new() -> Self {
        Self
    }
}

#[axum::async_trait]
impl Judge for DefaultJudge {
    async fn judge(&self, _claim: &ClaimRecord) -> anyhow::Result<JudgeOutput> {
        Ok(JudgeOutput {
            decision: JudgeDecision::Uncertain,
            reason_code: "default_judge".to_string(),
            confidence: 0.0,
            normalized_relation_group: None,
            normalized_relation_label: None,
            directionality: None,
            current_state: None,
            strength: None,
            polarity: None,
            importance_score: None,
            redirect_to: None,
            redirect_dimension_key: None,
            explanation_for_log: "Default judge: no real AI judge wired yet".to_string(),
        })
    }
}

#[cfg(test)]
pub struct MockJudge {
    pub output: JudgeOutput,
}

#[cfg(test)]
impl MockJudge {
    pub fn new(output: JudgeOutput) -> Self {
        Self { output }
    }
}

#[cfg(test)]
#[axum::async_trait]
impl Judge for MockJudge {
    async fn judge(&self, _claim: &ClaimRecord) -> anyhow::Result<JudgeOutput> {
        Ok(self.output.clone())
    }
}

#[derive(Debug, Default)]
pub struct RelationshipSegmentProcessResult {
    pub claims_accepted: usize,
    pub claims_rejected: usize,
    pub claims_redirected: usize,
    pub claims_quarantined: usize,
    pub claims_uncertain: usize,
    pub derived_property_claims: usize,
}

pub(crate) async fn process_relationship_claims_for_segment(
    book_id: &str,
    chapter_index: i64,
    pool: &SqlitePool,
    claim_repo: &ClaimRepo,
    entity_repo: &EntityRepo,
    ai_run_id: &str,
    relationship_claims: &[ClaimRecord],
    judge: &dyn Judge,
) -> anyhow::Result<RelationshipSegmentProcessResult> {
    let mut result = RelationshipSegmentProcessResult::default();
    let mut gate_accepted = Vec::new();
    let mut redirect_claims = Vec::new();
    let mut relationship_commands = Vec::new();

    for rel_claim in relationship_claims {
        let claim = relationship_decision::resolve_relationship_claim_entities(
            book_id,
            entity_repo,
            rel_claim,
        )
        .await?;
        let subject_entity = match &claim.subject_entity_id {
            Some(id) => entity_repo.get_by_id(id).await?,
            None => None,
        };
        let object_entity = match &claim.object_entity_id {
            Some(id) => entity_repo.get_by_id(id).await?,
            None => None,
        };

        let gate_decision = relationship_decision::materialize_relationship_gate_decision(
            &claim,
            subject_entity.as_ref(),
            object_entity.as_ref(),
        )?;

        match gate_decision {
            RelationshipGateDecision::NeedsJudge(ledger_claim) => {
                gate_accepted.push((claim, ledger_claim));
            }
            RelationshipGateDecision::Decided(decision) => {
                apply_relationship_decision_lifecycle(
                    book_id,
                    chapter_index,
                    claim_repo,
                    ai_run_id,
                    &claim,
                    decision,
                    &mut relationship_commands,
                    &mut redirect_claims,
                    &mut result,
                )
                .await?;
            }
        }
    }

    for (claim, ledger_claim) in &gate_accepted {
        match judge.judge(claim).await {
            Ok(judge_output) => {
                match relationship_decision::materialize_relationship_decision(
                    ledger_claim.clone(),
                    judge_output,
                ) {
                    Ok(decision) => {
                        apply_relationship_decision_lifecycle(
                            book_id,
                            chapter_index,
                            claim_repo,
                            ai_run_id,
                            claim,
                            decision,
                            &mut relationship_commands,
                            &mut redirect_claims,
                            &mut result,
                        )
                        .await?;
                    }
                    Err(err) => {
                        tracing::warn!(
                            "Relationship judge output for claim {} could not materialize decision: {}",
                            claim.id,
                            err
                        );
                        reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    "AI judge failed for claim {}: {}. Marking as rejected.",
                    claim.id,
                    err
                );
                reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
            }
        }
    }

    for command in relationship_commands {
        let claim_id = command.provenance.claim_id.clone();
        match reducer::apply_relationship_write(command, pool).await {
            Ok(_) => {
                accept_claim(claim_repo, &claim_id, &mut result.claims_accepted).await?;
            }
            Err(err) => {
                tracing::warn!(
                    "Relationship command for claim {} failed: {}. Marking uncertain.",
                    claim_id,
                    err
                );
                mark_uncertain_claim(claim_repo, &claim_id, &mut result.claims_uncertain).await?;
            }
        }
    }

    if !redirect_claims.is_empty() {
        result.derived_property_claims = redirect_claims.len();
        character_processor::process_character_claims_for_segment(
            pool,
            claim_repo,
            &redirect_claims,
        )
        .await?;
    }

    Ok(result)
}

async fn apply_relationship_decision_lifecycle(
    book_id: &str,
    chapter_index: i64,
    claim_repo: &ClaimRepo,
    ai_run_id: &str,
    claim: &ClaimRecord,
    decision: RelationshipDecision,
    relationship_commands: &mut Vec<reducer::RelationshipWriteCommand>,
    redirect_claims: &mut Vec<ClaimRecord>,
    result: &mut RelationshipSegmentProcessResult,
) -> anyhow::Result<()> {
    match decision {
        RelationshipDecision::Write(command) => {
            relationship_commands.push(command);
        }
        RelationshipDecision::NoWrite(no_write) => match no_write.status {
            RelationshipNoWriteStatus::Rejected => {
                tracing::debug!(
                    "Relationship claim {} rejected: {}",
                    claim.id,
                    no_write.reason_code
                );
                reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
            }
            RelationshipNoWriteStatus::Redirected => {
                let redirect_to = no_write.redirect_to.as_deref().unwrap_or("minor_event");
                if redirect_to == "property_update" {
                    if let Some(dim_key) = no_write.redirect_dimension_key {
                        let value_text = claim.object_mention.clone().unwrap_or_default();
                        let redirect_json = serde_json::json!({
                            "redirected_from_claim_id": claim.id,
                        });
                        let derived_claim = claim_repo
                            .create_claim(
                                book_id,
                                chapter_index,
                                "property_update",
                                claim.subject_mention.as_deref(),
                                claim.object_mention.as_deref(),
                                claim.subject_entity_id.as_deref(),
                                claim.object_entity_id.as_deref(),
                                &format!("{} = {}", dim_key, value_text),
                                Some(&value_text),
                                Some(&redirect_json.to_string()),
                                &claim.primary_source_span_id,
                                ai_run_id,
                                no_write.confidence,
                                "low",
                            )
                            .await?;
                        redirect_claims.push(derived_claim);
                    }
                }
                redirect_claim(claim_repo, &claim.id, &mut result.claims_redirected).await?;
            }
            RelationshipNoWriteStatus::Uncertain => {
                tracing::debug!(
                    "Relationship claim {} uncertain: {}",
                    claim.id,
                    no_write.reason_code
                );
                mark_uncertain_claim(claim_repo, &claim.id, &mut result.claims_uncertain).await?;
            }
        },
        RelationshipDecision::Invalid(rejection) => {
            tracing::warn!(
                "Relationship decision rejected claim {}: {}",
                rejection.claim_id,
                rejection.reason
            );
            reject_claim(claim_repo, &rejection.claim_id, &mut result.claims_rejected).await?;
        }
        RelationshipDecision::Quarantine(quarantine) => {
            tracing::warn!(
                "Relationship decision quarantined claim {}: {}",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::relationship_decision::RelationshipQuarantine;
    use crate::service::v4::test_support::setup_v4_processor_test;

    #[tokio::test]
    async fn relationship_quarantine_decision_marks_claim_quarantined() {
        let ctx =
            setup_v4_processor_test("relationship-lifecycle", 3, "关系文本", "关系文本").await;
        let claim_repo = ClaimRepo::new(ctx.pool.clone());
        let claim = claim_repo
            .create_claim(
                "b1",
                3,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                None,
                None,
                "relationship_update",
                None,
                Some(
                    r#"{"relation_hint":"结盟","relation_group":"alliance","relation_label":"盟友","directionality":"undirected"}"#,
                ),
                &ctx.span_id,
                &ctx.run_id,
                0.9,
                "high",
            )
            .await
            .unwrap();
        let mut result = RelationshipSegmentProcessResult::default();

        apply_relationship_decision_lifecycle(
            "b1",
            3,
            &claim_repo,
            &ctx.run_id,
            &claim,
            RelationshipDecision::Quarantine(RelationshipQuarantine {
                claim_id: claim.id.clone(),
                reason: "missing resolved pair".to_string(),
            }),
            &mut Vec::new(),
            &mut Vec::new(),
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
