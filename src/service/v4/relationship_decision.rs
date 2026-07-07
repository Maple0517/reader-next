use crate::service::v4::ledger_payload;
use crate::service::v4::reducer::{RelationshipWriteCommand, RelationshipWriteProvenance};
use crate::service::v4::relationship_judge::{self, GateResult, JudgeDecision, JudgeOutput};
use crate::storage::db::v4::claim_repo::ClaimRecord;
use crate::storage::db::v4::entity_repo::{EntityRecord, EntityRepo};

#[derive(Debug, Clone)]
pub struct LedgerRelationshipClaim {
    pub claim_id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub subject_mention: Option<String>,
    pub object_mention: Option<String>,
    pub subject_entity_id: Option<String>,
    pub object_entity_id: Option<String>,
    pub relation_hint: String,
    pub relation_group: String,
    pub relation_label: String,
    pub directionality: String,
    pub importance_hint: f64,
    pub is_long_term_or_significant_hint: bool,
    pub primary_source_span_id: String,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub enum RelationshipDecision {
    Invalid(RelationshipDecisionRejection),
    NoWrite(RelationshipNoWrite),
    Quarantine(RelationshipQuarantine),
    Write(RelationshipWriteCommand),
}

#[derive(Debug, Clone)]
pub enum RelationshipGateDecision {
    NeedsJudge(LedgerRelationshipClaim),
    Decided(RelationshipDecision),
}

#[derive(Debug, Clone)]
pub struct RelationshipDecisionRejection {
    pub claim_id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct RelationshipNoWrite {
    pub claim_id: String,
    pub status: RelationshipNoWriteStatus,
    pub reason_code: String,
    pub redirect_to: Option<String>,
    pub redirect_dimension_key: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationshipNoWriteStatus {
    Rejected,
    Redirected,
    Uncertain,
}

#[derive(Debug, Clone)]
pub struct RelationshipQuarantine {
    pub claim_id: String,
    pub reason: String,
}

impl LedgerRelationshipClaim {
    pub fn from_claim(claim: &ClaimRecord) -> anyhow::Result<Self> {
        ledger_payload::ensure_claim_type(claim, "relationship_update", "relationship_update")?;
        let object = ledger_payload::claim_payload_object(claim, "relationship")?;
        ledger_payload::reject_forbidden_fields(
            claim,
            &object,
            "relationship",
            "judge normalized field",
            &[
                "normalized_relation_group",
                "normalized_relation_label",
                "judge_confidence",
                "judge_reason_code",
            ],
        )?;
        let required_string =
            |key| ledger_payload::required_raw_string_field(&object, "relationship", key);

        Ok(Self {
            claim_id: claim.id.clone(),
            book_id: claim.book_id.clone(),
            chapter_index: claim.chapter_index,
            subject_mention: claim.subject_mention.clone(),
            object_mention: claim.object_mention.clone(),
            subject_entity_id: claim.subject_entity_id.clone(),
            object_entity_id: claim.object_entity_id.clone(),
            relation_hint: required_string("relation_hint")?,
            relation_group: required_string("relation_group")?,
            relation_label: required_string("relation_label")?,
            directionality: required_string("directionality")?,
            importance_hint: ledger_payload::optional_f64(&object, "importance_hint")
                .unwrap_or(0.5),
            is_long_term_or_significant_hint: object
                .get("is_long_term_or_significant_hint")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            primary_source_span_id: claim.primary_source_span_id.clone(),
            confidence: claim.confidence,
        })
    }
}

pub async fn resolve_relationship_claim_entities(
    book_id: &str,
    entity_repo: &EntityRepo,
    claim: &ClaimRecord,
) -> anyhow::Result<ClaimRecord> {
    let mut claim = claim.clone();
    if let Some(ref subject_mention) = claim.subject_mention {
        if let Some(entity) = entity_repo
            .find_entity_by_alias(book_id, subject_mention)
            .await?
        {
            claim.subject_entity_id = Some(entity.id);
        }
    }
    if let Some(ref object_mention) = claim.object_mention {
        if let Some(entity) = entity_repo
            .find_entity_by_alias(book_id, object_mention)
            .await?
        {
            claim.object_entity_id = Some(entity.id);
        }
    }
    Ok(claim)
}

pub fn materialize_relationship_gate_decision(
    claim: &ClaimRecord,
    subject_entity: Option<&EntityRecord>,
    object_entity: Option<&EntityRecord>,
) -> anyhow::Result<RelationshipGateDecision> {
    let ledger_claim = match LedgerRelationshipClaim::from_claim(claim) {
        Ok(ledger_claim) => ledger_claim,
        Err(err) => {
            return Ok(RelationshipGateDecision::Decided(
                RelationshipDecision::Invalid(RelationshipDecisionRejection {
                    claim_id: claim.id.clone(),
                    reason: err.to_string(),
                }),
            ));
        }
    };

    let gate_result = relationship_judge::structural_gate_with_relationship_fields(
        claim,
        subject_entity,
        object_entity,
        &ledger_claim.relation_group,
        ledger_claim.is_long_term_or_significant_hint,
    );

    match gate_result {
        GateResult::Pass => Ok(RelationshipGateDecision::NeedsJudge(ledger_claim)),
        GateResult::Reject(_reason) => Ok(RelationshipGateDecision::Decided(
            RelationshipDecision::NoWrite(RelationshipNoWrite {
                claim_id: ledger_claim.claim_id,
                status: RelationshipNoWriteStatus::Rejected,
                reason_code: "structural_gate_reject".to_string(),
                redirect_to: None,
                redirect_dimension_key: None,
                confidence: ledger_claim.confidence,
            }),
        )),
        GateResult::Redirect {
            target,
            dimension_key,
        } => Ok(RelationshipGateDecision::Decided(
            RelationshipDecision::NoWrite(RelationshipNoWrite {
                claim_id: ledger_claim.claim_id,
                status: RelationshipNoWriteStatus::Redirected,
                reason_code: "structural_gate_redirect".to_string(),
                redirect_to: Some(target),
                redirect_dimension_key: dimension_key,
                confidence: ledger_claim.confidence,
            }),
        )),
        GateResult::Uncertain(reason) => Ok(RelationshipGateDecision::Decided(
            RelationshipDecision::NoWrite(RelationshipNoWrite {
                claim_id: ledger_claim.claim_id,
                status: RelationshipNoWriteStatus::Uncertain,
                reason_code: format!("structural_gate_uncertain: {reason}"),
                redirect_to: None,
                redirect_dimension_key: None,
                confidence: ledger_claim.confidence,
            }),
        )),
    }
}

pub fn materialize_relationship_decision(
    claim: LedgerRelationshipClaim,
    judge_output: JudgeOutput,
) -> anyhow::Result<RelationshipDecision> {
    match judge_output.decision {
        JudgeDecision::Accept => {
            let claim_id = claim.claim_id.clone();
            match relationship_write_command_from_judge(claim, judge_output) {
                Ok(command) => Ok(RelationshipDecision::Write(command)),
                Err(err) => Ok(classify_relationship_write_failure(claim_id, err)),
            }
        }
        JudgeDecision::Reject => Ok(RelationshipDecision::NoWrite(RelationshipNoWrite {
            claim_id: claim.claim_id,
            status: RelationshipNoWriteStatus::Rejected,
            reason_code: judge_output.reason_code,
            redirect_to: None,
            redirect_dimension_key: None,
            confidence: judge_output.confidence,
        })),
        JudgeDecision::Redirect => Ok(RelationshipDecision::NoWrite(RelationshipNoWrite {
            claim_id: claim.claim_id,
            status: RelationshipNoWriteStatus::Redirected,
            reason_code: judge_output.reason_code,
            redirect_to: judge_output.redirect_to,
            redirect_dimension_key: judge_output.redirect_dimension_key,
            confidence: judge_output.confidence,
        })),
        JudgeDecision::Uncertain => Ok(RelationshipDecision::NoWrite(RelationshipNoWrite {
            claim_id: claim.claim_id,
            status: RelationshipNoWriteStatus::Uncertain,
            reason_code: judge_output.reason_code,
            redirect_to: None,
            redirect_dimension_key: None,
            confidence: judge_output.confidence,
        })),
    }
}

fn classify_relationship_write_failure(
    claim_id: String,
    err: anyhow::Error,
) -> RelationshipDecision {
    let reason = err.to_string();
    if reason.contains("missing subject id") || reason.contains("missing object id") {
        return RelationshipDecision::Quarantine(RelationshipQuarantine { claim_id, reason });
    }

    RelationshipDecision::Invalid(RelationshipDecisionRejection { claim_id, reason })
}

fn relationship_write_command_from_judge(
    claim: LedgerRelationshipClaim,
    judge_output: JudgeOutput,
) -> anyhow::Result<RelationshipWriteCommand> {
    let subject_character_id = claim.subject_entity_id.ok_or_else(|| {
        anyhow::anyhow!("relationship claim {} missing subject id", claim.claim_id)
    })?;
    let object_character_id = claim.object_entity_id.ok_or_else(|| {
        anyhow::anyhow!("relationship claim {} missing object id", claim.claim_id)
    })?;
    let relation_group = judge_output.normalized_relation_group.ok_or_else(|| {
        anyhow::anyhow!(
            "relationship claim {} missing normalized group",
            claim.claim_id
        )
    })?;
    let relation_label = judge_output.normalized_relation_label.ok_or_else(|| {
        anyhow::anyhow!(
            "relationship claim {} missing normalized label",
            claim.claim_id
        )
    })?;
    let directionality = judge_output.directionality.ok_or_else(|| {
        anyhow::anyhow!(
            "relationship claim {} missing directionality",
            claim.claim_id
        )
    })?;

    Ok(RelationshipWriteCommand {
        book_id: claim.book_id,
        subject_character_id,
        object_character_id,
        relation_group,
        relation_label,
        directionality,
        current_state: judge_output.current_state,
        strength: judge_output.strength.unwrap_or(0.5),
        polarity: judge_output
            .polarity
            .unwrap_or_else(|| "neutral".to_string()),
        importance_score: judge_output.importance_score.unwrap_or(0.5),
        confidence: judge_output.confidence,
        chapter_index: claim.chapter_index,
        provenance: RelationshipWriteProvenance {
            claim_id: claim.claim_id,
            evidence_span_ids: vec![claim.primary_source_span_id],
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::relationship_judge::{JudgeDecision, JudgeOutput};

    fn relationship_claim_with_payload(value_json: &str) -> ClaimRecord {
        ClaimRecord {
            id: "claim-1".to_string(),
            book_id: "b1".to_string(),
            chapter_index: 3,
            claim_type: "relationship_update".to_string(),
            subject_mention: Some("张三".to_string()),
            object_mention: Some("李四".to_string()),
            subject_entity_id: Some("char-a".to_string()),
            object_entity_id: Some("char-b".to_string()),
            predicate: "张三 and 李四".to_string(),
            value_json: Some(value_json.to_string()),
            value_text: None,
            primary_source_span_id: "span-1".to_string(),
            ai_run_id: "run-1".to_string(),
            confidence: 0.86,
            status: "proposed".to_string(),
            risk_level: "high".to_string(),
            supersedes_claim_id: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    #[test]
    fn ledger_relationship_claim_rejects_judge_normalized_fields() {
        let claim = relationship_claim_with_payload(
            r#"{
                "relation_hint":"朋友",
                "relation_group":"friendship",
                "relation_label":"friend",
                "directionality":"undirected",
                "importance_hint":0.7,
                "is_long_term_or_significant_hint":true,
                "normalized_relation_group":"friendship",
                "judge_confidence":0.9
            }"#,
        );

        let err = LedgerRelationshipClaim::from_claim(&claim).unwrap_err();

        assert!(
            err.to_string().contains("judge normalized"),
            "relationship ledger payload must preserve source observation only"
        );
    }

    #[test]
    fn relationship_materializer_keeps_judge_output_inside_decision_layer() {
        let claim = relationship_claim_with_payload(
            r#"{
                "relation_hint":"朋友",
                "relation_group":"friendship",
                "relation_label":"friend",
                "directionality":"undirected",
                "importance_hint":0.7,
                "is_long_term_or_significant_hint":true
            }"#,
        );
        let original_value_json = claim.value_json.clone();
        let ledger_claim = LedgerRelationshipClaim::from_claim(&claim).unwrap();
        let judge_output = JudgeOutput {
            decision: JudgeDecision::Accept,
            reason_code: "accepted_by_test".to_string(),
            confidence: 0.91,
            normalized_relation_group: Some("family".to_string()),
            normalized_relation_label: Some("sibling".to_string()),
            directionality: Some("undirected".to_string()),
            current_state: Some("close".to_string()),
            strength: Some(0.8),
            polarity: Some("positive".to_string()),
            importance_score: Some(0.76),
            redirect_to: None,
            redirect_dimension_key: None,
            explanation_for_log: "test output".to_string(),
        };

        let decision = materialize_relationship_decision(ledger_claim, judge_output).unwrap();

        match decision {
            RelationshipDecision::Write(command) => {
                assert_eq!(command.book_id, "b1");
                assert_eq!(command.subject_character_id, "char-a");
                assert_eq!(command.object_character_id, "char-b");
                assert_eq!(command.relation_group, "family");
                assert_eq!(command.relation_label, "sibling");
                assert_eq!(command.directionality, "undirected");
                assert_eq!(command.current_state.as_deref(), Some("close"));
                assert_eq!(command.strength, 0.8);
                assert_eq!(command.polarity, "positive");
                assert_eq!(command.importance_score, 0.76);
                assert_eq!(command.confidence, 0.91);
                assert_eq!(command.chapter_index, 3);
                assert_eq!(command.provenance.claim_id, "claim-1");
                assert_eq!(command.provenance.evidence_span_ids, vec!["span-1"]);
            }
            other => panic!("expected relationship write decision, got {other:?}"),
        }
        assert_eq!(claim.value_json, original_value_json);
    }

    #[test]
    fn relationship_materializer_invalidates_accept_missing_write_fields() {
        let claim = relationship_claim_with_payload(
            r#"{
                "relation_hint":"朋友",
                "relation_group":"friendship",
                "relation_label":"friend",
                "directionality":"undirected",
                "importance_hint":0.7,
                "is_long_term_or_significant_hint":true
            }"#,
        );
        let ledger_claim = LedgerRelationshipClaim::from_claim(&claim).unwrap();
        let judge_output = JudgeOutput {
            decision: JudgeDecision::Accept,
            reason_code: "accepted_but_incomplete".to_string(),
            confidence: 0.91,
            normalized_relation_group: None,
            normalized_relation_label: Some("朋友".to_string()),
            directionality: Some("undirected".to_string()),
            current_state: None,
            strength: None,
            polarity: None,
            importance_score: None,
            redirect_to: None,
            redirect_dimension_key: None,
            explanation_for_log: "test output".to_string(),
        };

        let decision = materialize_relationship_decision(ledger_claim, judge_output).unwrap();

        match decision {
            RelationshipDecision::Invalid(rejection) => {
                assert_eq!(rejection.claim_id, "claim-1");
                assert!(rejection.reason.contains("normalized group"));
            }
            other => panic!("expected invalid decision, got {other:?}"),
        }
    }

    #[test]
    fn relationship_materializer_quarantines_accept_without_resolved_object() {
        let mut claim = relationship_claim_with_payload(
            r#"{
                "relation_hint":"朋友",
                "relation_group":"friendship",
                "relation_label":"friend",
                "directionality":"undirected",
                "importance_hint":0.7,
                "is_long_term_or_significant_hint":true
            }"#,
        );
        claim.object_entity_id = None;
        let ledger_claim = LedgerRelationshipClaim::from_claim(&claim).unwrap();
        let judge_output = JudgeOutput {
            decision: JudgeDecision::Accept,
            reason_code: "accepted_but_unresolved".to_string(),
            confidence: 0.91,
            normalized_relation_group: Some("friendship".to_string()),
            normalized_relation_label: Some("朋友".to_string()),
            directionality: Some("undirected".to_string()),
            current_state: None,
            strength: None,
            polarity: None,
            importance_score: None,
            redirect_to: None,
            redirect_dimension_key: None,
            explanation_for_log: "test output".to_string(),
        };

        let decision = materialize_relationship_decision(ledger_claim, judge_output).unwrap();

        match decision {
            RelationshipDecision::Quarantine(quarantine) => {
                assert_eq!(quarantine.claim_id, "claim-1");
                assert!(quarantine.reason.contains("object id"));
            }
            other => panic!("expected quarantine decision, got {other:?}"),
        }
    }

    #[test]
    fn relationship_gate_materializer_turns_structural_reject_into_no_write() {
        let claim = relationship_claim_with_payload(
            r#"{
                "relation_hint":"偶遇",
                "relation_group":"invalid_group",
                "relation_label":"偶遇",
                "directionality":"undirected",
                "importance_hint":0.7,
                "is_long_term_or_significant_hint":true
            }"#,
        );
        let subject = crate::storage::db::v4::entity_repo::EntityRecord {
            id: "char-a".to_string(),
            book_id: "b1".to_string(),
            entity_type: "character".to_string(),
            canonical_name: "张三".to_string(),
            display_name: "张三".to_string(),
            short_summary: None,
            importance_score: 0.5,
            first_seen_chapter: 1,
            last_seen_chapter: 1,
            status: "active".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };
        let object = crate::storage::db::v4::entity_repo::EntityRecord {
            id: "char-b".to_string(),
            book_id: "b1".to_string(),
            entity_type: "character".to_string(),
            canonical_name: "李四".to_string(),
            display_name: "李四".to_string(),
            short_summary: None,
            importance_score: 0.5,
            first_seen_chapter: 1,
            last_seen_chapter: 1,
            status: "active".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };

        let gate_decision =
            materialize_relationship_gate_decision(&claim, Some(&subject), Some(&object)).unwrap();

        match gate_decision {
            RelationshipGateDecision::Decided(RelationshipDecision::NoWrite(no_write)) => {
                assert_eq!(no_write.claim_id, "claim-1");
                assert_eq!(no_write.status, RelationshipNoWriteStatus::Rejected);
                assert_eq!(no_write.reason_code, "structural_gate_reject");
            }
            other => panic!("expected structural no-write decision, got {other:?}"),
        }
    }
}
