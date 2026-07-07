use crate::service::v4::identity_judge::{self, IdentityJudgeDecision, IdentityJudgeOutput};
use crate::service::v4::reducer::{IdentityWriteCommand, IdentityWriteProvenance};
use crate::storage::db::v4::claim_repo::ClaimRecord;

#[derive(Debug, Clone)]
pub struct LedgerIdentityClaim {
    pub claim_id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub claim_type: String,
    pub subject_entity_id: Option<String>,
    pub object_entity_id: Option<String>,
    pub primary_source_span_id: String,
    pub confidence: f64,
    pub status: String,
}

#[derive(Debug, Clone)]
pub enum IdentityDecision {
    Invalid(IdentityDecisionRejection),
    NoWrite(IdentityNoWrite),
    Quarantine(IdentityQuarantine),
    Write(IdentityWriteCommand),
}

#[derive(Debug, Clone)]
pub struct IdentityDecisionRejection {
    pub claim_id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct IdentityNoWrite {
    pub claim_id: String,
    pub status: IdentityNoWriteStatus,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityNoWriteStatus {
    Rejected,
    Uncertain,
}

#[derive(Debug, Clone)]
pub struct IdentityQuarantine {
    pub claim_id: String,
    pub reason: String,
}

impl LedgerIdentityClaim {
    pub fn from_claim(claim: &ClaimRecord) -> anyhow::Result<Self> {
        if !matches!(
            claim.claim_type.as_str(),
            "identity_reveal"
                | "entity_merge_candidate"
                | "entity_split_candidate"
                | "not_same_identity"
        ) {
            anyhow::bail!(
                "claim {} is not an identity claim: {}",
                claim.id,
                claim.claim_type
            );
        }

        Ok(Self {
            claim_id: claim.id.clone(),
            book_id: claim.book_id.clone(),
            chapter_index: claim.chapter_index,
            claim_type: claim.claim_type.clone(),
            subject_entity_id: claim.subject_entity_id.clone(),
            object_entity_id: claim.object_entity_id.clone(),
            primary_source_span_id: claim.primary_source_span_id.clone(),
            confidence: claim.confidence,
            status: claim.status.clone(),
        })
    }
}

pub fn materialize_identity_decision(
    claim: LedgerIdentityClaim,
    judge_output: IdentityJudgeOutput,
) -> anyhow::Result<IdentityDecision> {
    if claim.status != "proposed" {
        return Ok(IdentityDecision::NoWrite(IdentityNoWrite {
            claim_id: claim.claim_id,
            status: IdentityNoWriteStatus::Uncertain,
            reason_code: format!("ledger_status_{}", claim.status),
        }));
    }

    match judge_output.decision {
        IdentityJudgeDecision::Reject => Ok(IdentityDecision::NoWrite(IdentityNoWrite {
            claim_id: claim.claim_id,
            status: IdentityNoWriteStatus::Rejected,
            reason_code: judge_output.reason_code,
        })),
        IdentityJudgeDecision::Uncertain | IdentityJudgeDecision::SplitRequired => {
            Ok(IdentityDecision::NoWrite(IdentityNoWrite {
                claim_id: claim.claim_id,
                status: IdentityNoWriteStatus::Uncertain,
                reason_code: judge_output.reason_code,
            }))
        }
        IdentityJudgeDecision::Merge
        | IdentityJudgeDecision::PossibleSameIdentity
        | IdentityJudgeDecision::NotSameIdentity => {
            if claim.subject_entity_id.is_none() || claim.object_entity_id.is_none() {
                return Ok(IdentityDecision::Quarantine(IdentityQuarantine {
                    claim_id: claim.claim_id,
                    reason: "identity write decision missing entity_a or entity_b id".to_string(),
                }));
            }

            if let Err(err) = identity_judge::validate_judge_decision(&judge_output, true) {
                return Ok(IdentityDecision::Invalid(IdentityDecisionRejection {
                    claim_id: claim.claim_id,
                    reason: err.to_string(),
                }));
            }

            let claim_id = claim.claim_id.clone();
            match identity_write_command_from_judge(claim, judge_output) {
                Ok(command) => Ok(IdentityDecision::Write(command)),
                Err(err) => Ok(IdentityDecision::Quarantine(IdentityQuarantine {
                    claim_id,
                    reason: err.to_string(),
                })),
            }
        }
    }
}

fn identity_write_command_from_judge(
    claim: LedgerIdentityClaim,
    judge_output: IdentityJudgeOutput,
) -> anyhow::Result<IdentityWriteCommand> {
    let entity_a_id = claim
        .subject_entity_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("identity claim {} missing entity_a id", claim.claim_id))?;
    let entity_b_id = claim
        .object_entity_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("identity claim {} missing entity_b id", claim.claim_id))?;
    let provenance = IdentityWriteProvenance {
        claim_id: claim.claim_id,
        evidence_span_ids: vec![claim.primary_source_span_id],
    };

    match judge_output.decision {
        IdentityJudgeDecision::Merge => Ok(IdentityWriteCommand::Merge {
            book_id: claim.book_id,
            entity_a_id,
            entity_b_id,
            link_type: judge_output.link_type,
            survivor_hint: judge_output.survivor_hint,
            confidence: judge_output.confidence,
            reason_code: judge_output.reason_code,
            provenance,
        }),
        IdentityJudgeDecision::PossibleSameIdentity | IdentityJudgeDecision::NotSameIdentity => {
            Ok(IdentityWriteCommand::Link {
                book_id: claim.book_id,
                entity_a_id,
                entity_b_id,
                link_type: judge_output.link_type,
                confidence: judge_output.confidence,
                provenance,
            })
        }
        IdentityJudgeDecision::Reject
        | IdentityJudgeDecision::Uncertain
        | IdentityJudgeDecision::SplitRequired => {
            anyhow::bail!("identity no-write decision cannot become a write command")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::reducer::IdentityWriteCommand;
    use crate::storage::db::v4::claim_repo::ClaimRecord;

    fn identity_claim(value_json: Option<&str>) -> ClaimRecord {
        ClaimRecord {
            id: "claim-identity-1".to_string(),
            book_id: "b1".to_string(),
            chapter_index: 9,
            claim_type: "identity_reveal".to_string(),
            subject_mention: Some("黑衣人".to_string()),
            object_mention: Some("张三".to_string()),
            subject_entity_id: Some("char-shadow".to_string()),
            object_entity_id: Some("char-zhang".to_string()),
            predicate: "黑衣人 is revealed as 张三".to_string(),
            value_json: value_json.map(str::to_string),
            value_text: None,
            primary_source_span_id: "span-identity-1".to_string(),
            ai_run_id: "run-identity-1".to_string(),
            confidence: 0.84,
            status: "proposed".to_string(),
            risk_level: "high".to_string(),
            supersedes_claim_id: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn merge_output() -> IdentityJudgeOutput {
        IdentityJudgeOutput {
            decision: IdentityJudgeDecision::Merge,
            link_type: "same_identity".to_string(),
            survivor_hint: "entity_b".to_string(),
            confidence: 0.91,
            reason_code: "explicit_reveal".to_string(),
            explanation_for_log: "same person".to_string(),
            property_conflicts: vec![],
            relationship_migration_hint: "safe".to_string(),
        }
    }

    #[test]
    fn identity_materializer_keeps_judge_output_inside_decision_layer() {
        let claim = identity_claim(Some(
            r#"{
                "source_observation":"identity_reveal",
                "note":"ledger payload only"
            }"#,
        ));
        let original_value_json = claim.value_json.clone();
        let ledger_claim = LedgerIdentityClaim::from_claim(&claim).unwrap();

        let decision = materialize_identity_decision(ledger_claim, merge_output()).unwrap();

        match decision {
            IdentityDecision::Write(IdentityWriteCommand::Merge {
                book_id,
                entity_a_id,
                entity_b_id,
                link_type,
                survivor_hint,
                confidence,
                reason_code,
                provenance,
            }) => {
                assert_eq!(book_id, "b1");
                assert_eq!(entity_a_id, "char-shadow");
                assert_eq!(entity_b_id, "char-zhang");
                assert_eq!(link_type, "same_identity");
                assert_eq!(survivor_hint, "entity_b");
                assert_eq!(confidence, 0.91);
                assert_eq!(reason_code, "explicit_reveal");
                assert_eq!(provenance.claim_id, "claim-identity-1");
                assert_eq!(provenance.evidence_span_ids, vec!["span-identity-1"]);
            }
            other => panic!("expected identity merge command, got {other:?}"),
        }
        assert_eq!(claim.value_json, original_value_json);
    }

    #[test]
    fn identity_materializer_turns_link_decision_into_link_command() {
        let claim = identity_claim(None);
        let ledger_claim = LedgerIdentityClaim::from_claim(&claim).unwrap();
        let output = IdentityJudgeOutput {
            decision: IdentityJudgeDecision::NotSameIdentity,
            link_type: "not_same_identity".to_string(),
            survivor_hint: "unknown".to_string(),
            confidence: 0.88,
            reason_code: "explicitly_distinct".to_string(),
            explanation_for_log: "two people".to_string(),
            property_conflicts: vec![],
            relationship_migration_hint: "unknown".to_string(),
        };

        let decision = materialize_identity_decision(ledger_claim, output).unwrap();

        match decision {
            IdentityDecision::Write(IdentityWriteCommand::Link {
                book_id,
                entity_a_id,
                entity_b_id,
                link_type,
                confidence,
                provenance,
            }) => {
                assert_eq!(book_id, "b1");
                assert_eq!(entity_a_id, "char-shadow");
                assert_eq!(entity_b_id, "char-zhang");
                assert_eq!(link_type, "not_same_identity");
                assert_eq!(confidence, 0.88);
                assert_eq!(provenance.claim_id, "claim-identity-1");
            }
            other => panic!("expected identity link command, got {other:?}"),
        }
    }

    #[test]
    fn identity_materializer_quarantines_write_without_resolved_pair() {
        let mut claim = identity_claim(None);
        claim.object_entity_id = None;
        let ledger_claim = LedgerIdentityClaim::from_claim(&claim).unwrap();

        let decision = materialize_identity_decision(ledger_claim, merge_output()).unwrap();

        match decision {
            IdentityDecision::Quarantine(quarantine) => {
                assert_eq!(quarantine.claim_id, "claim-identity-1");
                assert!(quarantine.reason.contains("entity_b"));
            }
            other => panic!("expected identity quarantine, got {other:?}"),
        }
    }
}
