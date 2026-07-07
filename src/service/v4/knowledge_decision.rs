use crate::service::v4::knowledge_judge::{
    KnowledgeCardAction, KnowledgeJudgeDecision, KnowledgeJudgeOutput,
};
use crate::service::v4::reducer::{
    KnowledgeAssertionStatus, KnowledgeWriteCardAction, KnowledgeWriteCommand,
    KnowledgeWriteDecision, KnowledgeWriteEntityRef, KnowledgeWriteProvenance,
};
use crate::service::v4::{ledger_payload, ledger_payload::LedgerPayloadObject};
use crate::storage::db::v4::claim_repo::ClaimRecord;

#[derive(Debug, Clone)]
pub struct LedgerKnowledgeClaim {
    pub claim_id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub category: String,
    pub topic_display: String,
    pub assertion_text: String,
    pub importance_score: f64,
    pub referenced_entities: Vec<KnowledgeWriteEntityRef>,
    pub primary_source_span_id: String,
    pub confidence: f64,
    pub status: String,
}

#[derive(Debug, Clone)]
pub enum KnowledgeDecision {
    Invalid(KnowledgeDecisionRejection),
    NoWrite(KnowledgeNoWrite),
    Quarantine(KnowledgeQuarantine),
    Write(KnowledgeWriteCommand),
}

#[derive(Debug, Clone)]
pub struct KnowledgeDecisionRejection {
    pub claim_id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct KnowledgeNoWrite {
    pub claim_id: String,
    pub status: KnowledgeNoWriteStatus,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnowledgeNoWriteStatus {
    Rejected,
    Uncertain,
}

#[derive(Debug, Clone)]
pub struct KnowledgeQuarantine {
    pub claim_id: String,
    pub reason: String,
}

impl LedgerKnowledgeClaim {
    pub fn from_claim(claim: &ClaimRecord) -> anyhow::Result<Self> {
        ledger_payload::ensure_claim_type(claim, "knowledge_assertion", "knowledge_assertion")?;
        let object = ledger_payload::claim_payload_object(claim, "knowledge")?;
        ledger_payload::reject_forbidden_fields(
            claim,
            &object,
            "knowledge",
            "judge field",
            &[
                "judge_decision",
                "card_action",
                "target_card_id",
                "assertion_status",
                "judge_confidence",
                "reason_code",
                "explanation_for_log",
            ],
        )?;
        let required_string = |key| ledger_payload::required_string(&object, "knowledge", key);

        Ok(Self {
            claim_id: claim.id.clone(),
            book_id: claim.book_id.clone(),
            chapter_index: claim.chapter_index,
            category: required_string("category")?,
            topic_display: required_string("topic_display")
                .or_else(|_| required_string("raw_topic"))?,
            assertion_text: required_string("assertion_text").or_else(|_| {
                claim
                    .value_text
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("knowledge claim missing assertion_text"))
            })?,
            importance_score: object
                .get("importance_score")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.5),
            referenced_entities: parse_referenced_entities(&object),
            primary_source_span_id: claim.primary_source_span_id.clone(),
            confidence: claim.confidence,
            status: claim.status.clone(),
        })
    }
}

pub fn materialize_knowledge_decision(
    claim: LedgerKnowledgeClaim,
    judge_output: KnowledgeJudgeOutput,
) -> anyhow::Result<KnowledgeDecision> {
    if claim.status != "proposed" {
        return Ok(KnowledgeDecision::NoWrite(KnowledgeNoWrite {
            claim_id: claim.claim_id,
            status: KnowledgeNoWriteStatus::Uncertain,
            reason_code: format!("ledger_status_{}", claim.status),
        }));
    }

    if judge_output.decision == KnowledgeJudgeDecision::Reject {
        return Ok(KnowledgeDecision::NoWrite(KnowledgeNoWrite {
            claim_id: claim.claim_id,
            status: KnowledgeNoWriteStatus::Rejected,
            reason_code: judge_output.reason_code,
        }));
    }
    if judge_output.card_action == KnowledgeCardAction::Uncertain {
        return Ok(KnowledgeDecision::NoWrite(KnowledgeNoWrite {
            claim_id: claim.claim_id,
            status: KnowledgeNoWriteStatus::Uncertain,
            reason_code: judge_output.reason_code,
        }));
    }

    let claim_id = claim.claim_id.clone();
    match knowledge_write_command_from_judge(claim, judge_output) {
        Ok(command) => Ok(KnowledgeDecision::Write(command)),
        Err(err) => Ok(KnowledgeDecision::Quarantine(KnowledgeQuarantine {
            claim_id,
            reason: err.to_string(),
        })),
    }
}

fn knowledge_write_command_from_judge(
    claim: LedgerKnowledgeClaim,
    judge_output: KnowledgeJudgeOutput,
) -> anyhow::Result<KnowledgeWriteCommand> {
    let card_action = match judge_output.card_action {
        KnowledgeCardAction::CreateNewCard => KnowledgeWriteCardAction::CreateNewCard,
        KnowledgeCardAction::UseExistingCard => KnowledgeWriteCardAction::UseExistingCard,
        KnowledgeCardAction::Uncertain => {
            anyhow::bail!("knowledge no-write card action cannot become a write command")
        }
    };
    let decision = match judge_output.decision {
        KnowledgeJudgeDecision::AddNew => KnowledgeWriteDecision::AddNew,
        KnowledgeJudgeDecision::Supplement => KnowledgeWriteDecision::Supplement,
        KnowledgeJudgeDecision::ReviseExisting => KnowledgeWriteDecision::ReviseExisting,
        KnowledgeJudgeDecision::ContradictExisting => KnowledgeWriteDecision::ContradictExisting,
        KnowledgeJudgeDecision::MarkRumor => KnowledgeWriteDecision::MarkRumor,
        KnowledgeJudgeDecision::MarkUncertain => KnowledgeWriteDecision::MarkUncertain,
        KnowledgeJudgeDecision::MarkFalseInWorld => KnowledgeWriteDecision::MarkFalseInWorld,
        KnowledgeJudgeDecision::Reject => {
            anyhow::bail!("knowledge reject decision cannot become a write command")
        }
    };
    let assertion_status =
        KnowledgeAssertionStatus::from_write_status(&judge_output.assertion_status)?;

    Ok(KnowledgeWriteCommand {
        book_id: claim.book_id,
        category: claim.category,
        topic_display: claim.topic_display,
        assertion_text: claim.assertion_text,
        importance_score: claim.importance_score,
        referenced_entities: claim.referenced_entities,
        card_action,
        target_card_id: judge_output.target_card_id,
        decision,
        affected_assertion_ids: judge_output.affected_assertion_ids,
        assertion_status,
        current_summary: judge_output.current_summary,
        confidence: judge_output.confidence,
        chapter_index: claim.chapter_index,
        provenance: KnowledgeWriteProvenance {
            claim_id: claim.claim_id,
            evidence_span_ids: vec![claim.primary_source_span_id],
        },
    })
}

fn parse_referenced_entities(object: &LedgerPayloadObject) -> Vec<KnowledgeWriteEntityRef> {
    object
        .get("referenced_entity_mentions")
        .and_then(serde_json::Value::as_array)
        .map(|mentions| {
            mentions
                .iter()
                .filter_map(|mention| {
                    let entity_id = mention
                        .get("resolved_entity_id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())?;
                    let role = mention
                        .get("role")
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or("related");
                    Some(KnowledgeWriteEntityRef {
                        entity_id: entity_id.to_string(),
                        role: role.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::knowledge_judge::{
        KnowledgeCardAction, KnowledgeJudgeDecision, KnowledgeJudgeOutput,
    };
    use crate::service::v4::reducer::{
        KnowledgeAssertionStatus, KnowledgeWriteCardAction, KnowledgeWriteDecision,
    };
    use crate::storage::db::v4::claim_repo::ClaimRecord;

    fn knowledge_claim(value_json: Option<&str>) -> ClaimRecord {
        ClaimRecord {
            id: "claim-knowledge-1".to_string(),
            book_id: "b1".to_string(),
            chapter_index: 4,
            claim_type: "knowledge_assertion".to_string(),
            subject_mention: None,
            object_mention: None,
            subject_entity_id: None,
            object_entity_id: None,
            predicate: "knowledge".to_string(),
            value_json: value_json.map(str::to_string),
            value_text: Some("Cultivation has stable realm tiers.".to_string()),
            primary_source_span_id: "span-knowledge-1".to_string(),
            ai_run_id: "run-knowledge-1".to_string(),
            confidence: 0.84,
            status: "proposed".to_string(),
            risk_level: "high".to_string(),
            supersedes_claim_id: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn add_new_output() -> KnowledgeJudgeOutput {
        KnowledgeJudgeOutput {
            decision: KnowledgeJudgeDecision::AddNew,
            card_action: KnowledgeCardAction::CreateNewCard,
            target_card_id: None,
            affected_assertion_ids: Vec::new(),
            assertion_status: "active".to_string(),
            current_summary: Some("Cultivation realm summary".to_string()),
            confidence: 0.91,
            reason_code: "test_add_new".to_string(),
            explanation_for_log: "new knowledge".to_string(),
        }
    }

    #[test]
    fn knowledge_materializer_keeps_judge_output_inside_decision_layer() {
        let claim = knowledge_claim(Some(
            r#"{
                "category":"power_system",
                "raw_topic":"Cultivation Realms",
                "topic_display":"Cultivation Realms",
                "assertion_text":"Cultivation has stable realm tiers.",
                "importance_score":0.8,
                "referenced_entity_mentions":[],
                "status_hint":"fact"
            }"#,
        ));
        let original_value_json = claim.value_json.clone();
        let ledger_claim = LedgerKnowledgeClaim::from_claim(&claim).unwrap();

        let decision = materialize_knowledge_decision(ledger_claim, add_new_output()).unwrap();

        match decision {
            KnowledgeDecision::Write(command) => {
                assert_eq!(command.category, "power_system");
                assert_eq!(command.topic_display, "Cultivation Realms");
                assert_eq!(command.card_action, KnowledgeWriteCardAction::CreateNewCard);
                assert_eq!(command.decision, KnowledgeWriteDecision::AddNew);
                assert_eq!(command.assertion_status, KnowledgeAssertionStatus::Active);
                assert_eq!(
                    command.current_summary.as_deref(),
                    Some("Cultivation realm summary")
                );
                assert_eq!(command.confidence, 0.91);
                assert_eq!(command.provenance.claim_id, "claim-knowledge-1");
                assert_eq!(
                    command.provenance.evidence_span_ids,
                    vec!["span-knowledge-1"]
                );
            }
            other => panic!("expected knowledge write command, got {other:?}"),
        }
        assert_eq!(claim.value_json, original_value_json);
    }
}
