use crate::service::v4::claim_lifecycle::{
    accept_claim, mark_uncertain_claim, quarantine_claim, reject_claim,
};
use crate::service::v4::knowledge_decision::{
    self, KnowledgeDecision, KnowledgeNoWriteStatus, LedgerKnowledgeClaim,
};
use crate::service::v4::knowledge_judge::{
    self, KnowledgeGateResult, KnowledgeJudgeAssertionInput, KnowledgeRevisionJudge,
};
use crate::service::v4::reducer;
use crate::service::v4::topic_resolver::{TopicCandidateMatch, TopicResolver};
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo, SourceSpanRecord};
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
use sqlx::SqlitePool;

#[derive(Debug, Default)]
pub struct KnowledgeSegmentProcessResult {
    pub claims_accepted: usize,
    pub claims_rejected: usize,
    pub claims_quarantined: usize,
    pub claims_uncertain: usize,
}

pub(crate) async fn process_knowledge_claims_for_segment(
    book_id: &str,
    chapter_index: i64,
    pool: &SqlitePool,
    claim_repo: &ClaimRepo,
    segment_id: &str,
    knowledge_claims: &[ClaimRecord],
    knowledge_judge: &dyn KnowledgeRevisionJudge,
) -> anyhow::Result<KnowledgeSegmentProcessResult> {
    let knowledge_repo = KnowledgeRepo::new(pool.clone());
    let topic_resolver = TopicResolver::new(knowledge_repo);
    let source_spans = claim_repo.list_spans_by_segment(segment_id).await?;
    let entity_repo = EntityRepo::new(pool.clone());
    let mut result = KnowledgeSegmentProcessResult::default();

    for original_claim in knowledge_claims {
        let claim =
            resolve_knowledge_claim_references(book_id, &entity_repo, original_claim).await?;
        let ledger_claim = match LedgerKnowledgeClaim::from_claim(&claim) {
            Ok(ledger_claim) => ledger_claim,
            Err(err) => {
                tracing::debug!("Knowledge claim {} rejected: {}", claim.id, err);
                reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
                continue;
            }
        };
        let topic_resolution = topic_resolver
            .resolve(
                book_id,
                &ledger_claim.category,
                &ledger_claim.topic_display,
                chapter_index,
            )
            .await?;

        match knowledge_judge::structural_gate(&claim, &source_spans, Some(&topic_resolution)) {
            KnowledgeGateResult::Pass => {}
            KnowledgeGateResult::Reject(reason) => {
                tracing::debug!(
                    "Knowledge claim {} rejected by structural gate: {}",
                    claim.id,
                    reason
                );
                reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
                continue;
            }
            KnowledgeGateResult::Uncertain(reason) => {
                tracing::debug!("Knowledge claim {} uncertain: {}", claim.id, reason);
                mark_uncertain_claim(claim_repo, &claim.id, &mut result.claims_uncertain).await?;
                continue;
            }
        }

        let input = build_knowledge_judge_input(
            book_id,
            chapter_index,
            &claim,
            &ledger_claim,
            &topic_resolution,
            &source_spans,
            pool,
        )
        .await?;

        match knowledge_judge.judge(&input).await {
            Ok(output) => {
                let matching_candidate_count = topic_resolution
                    .candidates
                    .iter()
                    .filter(|candidate| {
                        candidate.match_kind != TopicCandidateMatch::RecentSameCategory
                    })
                    .count();
                if let Err(err) = knowledge_judge::validate_judge_output_for_context(
                    &output,
                    matching_candidate_count,
                ) {
                    tracing::warn!(
                        "Knowledge judge output invalid for claim {}: {}",
                        claim.id,
                        err
                    );
                    reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
                    continue;
                }

                let decision =
                    knowledge_decision::materialize_knowledge_decision(ledger_claim, output)?;
                apply_knowledge_decision_lifecycle(pool, claim_repo, decision, &mut result).await?;
            }
            Err(err) => {
                tracing::warn!("Knowledge judge failed for claim {}: {}", claim.id, err);
                reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
            }
        }
    }

    Ok(result)
}

async fn apply_knowledge_decision_lifecycle(
    pool: &SqlitePool,
    claim_repo: &ClaimRepo,
    decision: KnowledgeDecision,
    result: &mut KnowledgeSegmentProcessResult,
) -> anyhow::Result<()> {
    match decision {
        KnowledgeDecision::Write(command) => {
            let claim_id = command.provenance.claim_id.clone();
            match reducer::apply_knowledge_write(command, pool).await {
                Ok(_) => {
                    accept_claim(claim_repo, &claim_id, &mut result.claims_accepted).await?;
                }
                Err(err) => {
                    tracing::warn!(
                        "Knowledge command for claim {} failed: {}. Marking uncertain.",
                        claim_id,
                        err
                    );
                    mark_uncertain_claim(claim_repo, &claim_id, &mut result.claims_uncertain)
                        .await?;
                }
            }
        }
        KnowledgeDecision::NoWrite(no_write) => match no_write.status {
            KnowledgeNoWriteStatus::Rejected => {
                reject_claim(claim_repo, &no_write.claim_id, &mut result.claims_rejected).await?;
            }
            KnowledgeNoWriteStatus::Uncertain => {
                mark_uncertain_claim(claim_repo, &no_write.claim_id, &mut result.claims_uncertain)
                    .await?;
            }
        },
        KnowledgeDecision::Invalid(rejection) => {
            tracing::warn!(
                "Knowledge decision rejected claim {}: {}",
                rejection.claim_id,
                rejection.reason
            );
            reject_claim(claim_repo, &rejection.claim_id, &mut result.claims_rejected).await?;
        }
        KnowledgeDecision::Quarantine(quarantine) => {
            tracing::warn!(
                "Knowledge decision quarantined claim {}: {}",
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

async fn resolve_knowledge_claim_references(
    book_id: &str,
    entity_repo: &EntityRepo,
    claim: &ClaimRecord,
) -> anyhow::Result<ClaimRecord> {
    let Some(raw_value_json) = claim.value_json.as_deref() else {
        return Ok(claim.clone());
    };
    let mut value: serde_json::Value = serde_json::from_str(raw_value_json)?;
    let mut changed = false;
    if let Some(mentions) = value
        .get_mut("referenced_entity_mentions")
        .and_then(|value| value.as_array_mut())
    {
        for mention in mentions {
            let has_resolved = mention
                .get("resolved_entity_id")
                .and_then(|value| value.as_str())
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false);
            if has_resolved {
                continue;
            }
            let Some(name) = mention.get("mention").and_then(|value| value.as_str()) else {
                continue;
            };
            if let Some(entity_id) = find_entity_id_for_mention(entity_repo, book_id, name).await? {
                mention["resolved_entity_id"] = serde_json::Value::String(entity_id);
                changed = true;
            }
        }
    }

    if !changed {
        return Ok(claim.clone());
    }

    let mut updated_claim = claim.clone();
    updated_claim.value_json = Some(value.to_string());
    Ok(updated_claim)
}

async fn build_knowledge_judge_input(
    book_id: &str,
    chapter_index: i64,
    claim: &ClaimRecord,
    ledger_claim: &LedgerKnowledgeClaim,
    topic_resolution: &crate::service::v4::topic_resolver::TopicResolution,
    source_spans: &[SourceSpanRecord],
    pool: &SqlitePool,
) -> anyhow::Result<knowledge_judge::KnowledgeJudgeInput> {
    let knowledge_repo = KnowledgeRepo::new(pool.clone());
    let mut existing_assertions = Vec::new();
    for candidate in &topic_resolution.candidates {
        for assertion in knowledge_repo
            .list_assertions_for_card(&candidate.card_id)
            .await?
        {
            existing_assertions.push(KnowledgeJudgeAssertionInput {
                assertion_id: assertion.id,
                card_id: assertion.card_id,
                assertion_text: assertion.assertion_text,
                status: assertion.status,
                chapter_index: assertion.chapter_index,
            });
        }
    }

    Ok(knowledge_judge::KnowledgeJudgeInput {
        book_id: book_id.to_string(),
        chapter_index,
        claim_id: claim.id.clone(),
        category: ledger_claim.category.clone(),
        assertion_text: ledger_claim.assertion_text.clone(),
        status_hint: None,
        proposed_topic: topic_resolution.proposed.clone(),
        candidate_cards: topic_resolution.candidates.clone(),
        existing_assertions,
        evidence_spans: source_spans
            .iter()
            .filter(|span| span.id == claim.primary_source_span_id)
            .map(|span| span.text_excerpt.clone())
            .collect(),
    })
}

async fn find_entity_id_for_mention(
    entity_repo: &EntityRepo,
    book_id: &str,
    mention: &str,
) -> anyhow::Result<Option<String>> {
    Ok(entity_repo
        .find_entity_by_alias(book_id, mention)
        .await?
        .map(|entity| entity.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::knowledge_judge::{
        KnowledgeCardAction, KnowledgeJudgeDecision, KnowledgeJudgeOutput,
        MockKnowledgeRevisionJudge,
    };
    use crate::service::v4::test_support::setup_v4_processor_test;
    use crate::storage::db::v4::claim_repo::ClaimRepo;
    use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
    use sqlx::SqlitePool;

    async fn setup() -> (SqlitePool, ClaimRepo, KnowledgeRepo, String, String, String) {
        let ctx = setup_v4_processor_test("knowledge", 8, "text", "knowledge text").await;

        (
            ctx.pool.clone(),
            ClaimRepo::new(ctx.pool.clone()),
            KnowledgeRepo::new(ctx.pool.clone()),
            ctx.segment_id,
            ctx.span_id,
            ctx.run_id,
        )
    }

    #[tokio::test]
    async fn knowledge_segment_processor_applies_typed_command_without_mutating_claim_value_json() {
        let (pool, claim_repo, knowledge_repo, segment_id, span_id, run_id) = setup().await;
        let original_value_json = r#"{
            "category":"power_system",
            "raw_topic":"Cultivation Realms",
            "topic_display":"Cultivation Realms",
            "assertion_text":"Cultivation has stable realm tiers.",
            "importance_score":0.8,
            "referenced_entity_mentions":[],
            "status_hint":"fact"
        }"#;
        let claim = claim_repo
            .create_claim(
                "b1",
                8,
                "knowledge_assertion",
                None,
                None,
                None,
                None,
                "knowledge",
                Some("Cultivation has stable realm tiers."),
                Some(original_value_json),
                &span_id,
                &run_id,
                0.9,
                "high",
            )
            .await
            .unwrap();
        let judge = MockKnowledgeRevisionJudge::new(KnowledgeJudgeOutput {
            decision: KnowledgeJudgeDecision::AddNew,
            card_action: KnowledgeCardAction::CreateNewCard,
            target_card_id: None,
            affected_assertion_ids: Vec::new(),
            assertion_status: "active".to_string(),
            current_summary: Some("Cultivation realm summary".to_string()),
            confidence: 0.93,
            reason_code: "test_add_new".to_string(),
            explanation_for_log: "new knowledge".to_string(),
        });

        let result = process_knowledge_claims_for_segment(
            "b1",
            8,
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
        assert_eq!(
            knowledge_repo
                .list_cards("b1", None, None)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn knowledge_quarantine_decision_marks_claim_quarantined() {
        let (pool, claim_repo, _knowledge_repo, _segment_id, span_id, run_id) = setup().await;
        let claim = claim_repo
            .create_claim(
                "b1",
                8,
                "knowledge_assertion",
                None,
                None,
                None,
                None,
                "knowledge",
                Some("Cultivation has stable realm tiers."),
                Some(
                    r#"{"category":"power_system","topic_display":"Cultivation Realms","assertion_text":"Cultivation has stable realm tiers.","importance_score":0.8,"referenced_entity_mentions":[]}"#,
                ),
                &span_id,
                &run_id,
                0.9,
                "high",
            )
            .await
            .unwrap();
        let mut result = KnowledgeSegmentProcessResult::default();

        apply_knowledge_decision_lifecycle(
            &pool,
            &claim_repo,
            KnowledgeDecision::Quarantine(knowledge_decision::KnowledgeQuarantine {
                claim_id: claim.id.clone(),
                reason: "missing write target".to_string(),
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
