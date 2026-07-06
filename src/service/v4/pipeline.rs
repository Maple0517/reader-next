use crate::service::v4::claim_writer;
use crate::service::v4::context_builder::ContextBuilder;
use crate::service::v4::extractor::Extractor;
use crate::service::v4::identity_judge::{self, IdentityGateResult, IdentityJudge};
use crate::service::v4::knowledge_judge::{
    self, KnowledgeGateResult, KnowledgeJudgeAssertionInput, KnowledgeRevisionJudge,
};
use crate::service::v4::map_conflict_judge::{
    self, MapConflictDecision, MapConflictJudge, MapConflictJudgeInput,
};
use crate::service::v4::map_gate::{self, MapGateContext, MapGateResult};
use crate::service::v4::place_resolver::{PlaceResolutionAction, PlaceResolver};
use crate::service::v4::projection;
use crate::service::v4::relationship_judge::{self, GateResult, JudgeDecision, JudgeOutput};
use crate::service::v4::relationship_projection;
use crate::service::v4::resolver;
use crate::service::v4::topic_resolver::{TopicCandidateMatch, TopicResolver};
use crate::service::v4::{place_reducer, reducer};
use crate::storage::db::v4::ai_run_repo::AiRunRepo;
use crate::storage::db::v4::chapter_repo::{self, ChapterRepo};
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo, SourceSpanRecord};
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::identity_repo::{IdentityLinkRecord, IdentityRepo};
use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
use crate::storage::db::v4::progress_repo::ProgressRepo;
use sqlx::SqlitePool;
use std::collections::HashMap;

const PROMPT_VERSION: &str = "v1";
const SCHEMA_VERSION: i64 = 1;
const DEFAULT_MODEL: &str = "unknown";

/// Trait for making relationship judge decisions.
///
/// Implementations:
/// - `DefaultJudge`: placeholder — marks all relationship claims as uncertain.
/// - `MockJudge`: deterministic test judge (accepts all).
/// - Real AI judge: TODO (needs AiModelService wiring).
#[axum::async_trait]
pub trait Judge: Send + Sync {
    async fn judge(&self, claim: &ClaimRecord) -> anyhow::Result<JudgeOutput>;
}

/// Placeholder judge that marks all relationship claims as uncertain.
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

/// Mock judge for testing. Returns the configured `JudgeOutput` for every claim.
pub struct MockJudge {
    pub output: JudgeOutput,
}

impl MockJudge {
    pub fn new(output: JudgeOutput) -> Self {
        Self { output }
    }
}

#[axum::async_trait]
impl Judge for MockJudge {
    async fn judge(&self, _claim: &ClaimRecord) -> anyhow::Result<JudgeOutput> {
        Ok(self.output.clone())
    }
}

/// Process a single chapter through the full V4 pipeline.
///
/// Flow:
/// 1. Ensure chapter exists in DB
/// 2. Check idempotency (already processed with same params)
/// 3. Create chapter_processing_run
/// 4. Segment chapter text
/// 5. Upsert segments + source_spans
/// 6. For each segment: context -> extract -> resolve -> claim_writer -> reducer
///    Phase 1 entity/property reducer runs first (ensures new characters are created).
///    Phase 2: structural gate -> judge -> relationship_reducer -> second-pass property_reducer.
///    Phase 3 identity and Phase 4 knowledge run as independent canonical stages.
/// 7. Invalidate + rebuild projection cache (including relationship cache)
/// 8. Update chapter_processing_runs to success
/// 9. Update processing_progress.max_processed_chapter
pub async fn process_chapter(
    book_id: &str,
    chapter_index: i64,
    raw_text: &str,
    pool: &SqlitePool,
    extractor: &dyn Extractor,
    model_name: Option<&str>,
    judge: Option<&dyn Judge>,
) -> anyhow::Result<()> {
    process_chapter_with_knowledge_judge(
        book_id,
        chapter_index,
        raw_text,
        pool,
        extractor,
        model_name,
        judge,
        None,
        None,
    )
    .await
}

pub async fn process_chapter_with_identity_judge(
    book_id: &str,
    chapter_index: i64,
    raw_text: &str,
    pool: &SqlitePool,
    extractor: &dyn Extractor,
    model_name: Option<&str>,
    judge: Option<&dyn Judge>,
    identity_judge: Option<&dyn IdentityJudge>,
) -> anyhow::Result<()> {
    process_chapter_with_knowledge_judge(
        book_id,
        chapter_index,
        raw_text,
        pool,
        extractor,
        model_name,
        judge,
        identity_judge,
        None,
    )
    .await
}

pub async fn process_chapter_with_knowledge_judge(
    book_id: &str,
    chapter_index: i64,
    raw_text: &str,
    pool: &SqlitePool,
    extractor: &dyn Extractor,
    model_name: Option<&str>,
    judge: Option<&dyn Judge>,
    identity_judge: Option<&dyn IdentityJudge>,
    knowledge_judge: Option<&dyn KnowledgeRevisionJudge>,
) -> anyhow::Result<()> {
    process_chapter_with_all_judges(
        book_id,
        chapter_index,
        raw_text,
        pool,
        extractor,
        model_name,
        judge,
        identity_judge,
        knowledge_judge,
        None,
    )
    .await
}

pub async fn process_chapter_with_map_conflict_judge(
    book_id: &str,
    chapter_index: i64,
    raw_text: &str,
    pool: &SqlitePool,
    extractor: &dyn Extractor,
    model_name: Option<&str>,
    judge: Option<&dyn Judge>,
    identity_judge: Option<&dyn IdentityJudge>,
    knowledge_judge: Option<&dyn KnowledgeRevisionJudge>,
    map_conflict_judge: Option<&dyn MapConflictJudge>,
) -> anyhow::Result<()> {
    process_chapter_with_all_judges(
        book_id,
        chapter_index,
        raw_text,
        pool,
        extractor,
        model_name,
        judge,
        identity_judge,
        knowledge_judge,
        map_conflict_judge,
    )
    .await
}

async fn process_chapter_with_all_judges(
    book_id: &str,
    chapter_index: i64,
    raw_text: &str,
    pool: &SqlitePool,
    extractor: &dyn Extractor,
    model_name: Option<&str>,
    judge: Option<&dyn Judge>,
    identity_judge: Option<&dyn IdentityJudge>,
    knowledge_judge: Option<&dyn KnowledgeRevisionJudge>,
    map_conflict_judge: Option<&dyn MapConflictJudge>,
) -> anyhow::Result<()> {
    let model = model_name.unwrap_or(DEFAULT_MODEL);
    let chapter_repo = ChapterRepo::new(pool.clone());
    let progress_repo = ProgressRepo::new(pool.clone());
    let claim_repo = ClaimRepo::new(pool.clone());
    let ai_run_repo = AiRunRepo::new(pool.clone());
    let entity_repo = EntityRepo::new(pool.clone());
    let context_builder = ContextBuilder::new(pool.clone());

    // 1. Compute hash and upsert chapter
    let text_hash = crate::util::hash::md5_hex(raw_text);
    let chapter = chapter_repo
        .upsert_chapter(book_id, chapter_index, None, raw_text, &text_hash)
        .await?;

    // 2. Check idempotency
    if progress_repo
        .is_already_processed(
            book_id,
            chapter_index,
            &text_hash,
            PROMPT_VERSION,
            SCHEMA_VERSION,
        )
        .await?
    {
        return Ok(());
    }

    // 3. Create chapter_processing_run
    let processing_run = progress_repo
        .create_run(
            book_id,
            chapter_index,
            &text_hash,
            PROMPT_VERSION,
            SCHEMA_VERSION,
        )
        .await?;

    // 4. Segment chapter
    let segments_data = chapter_repo::segment_chapter(book_id, &chapter.id, &text_hash, raw_text);

    // Check if segments already exist for this chapter (active or stale with same hash)
    let existing_segments = chapter_repo.list_active_segments(&chapter.id).await?;
    let has_matching_segments = !existing_segments.is_empty()
        && existing_segments
            .iter()
            .any(|s| s.chapter_hash == text_hash);

    if !has_matching_segments {
        // Clean up any stale segments with the same hash (e.g., after reset)
        // Delete spans first (FK dependency on segments)
        chapter_repo.delete_stale_spans(&chapter.id).await?;
        chapter_repo.delete_stale_segments(&chapter.id).await?;
        // Mark remaining active segments/spans as stale (hash changed)
        chapter_repo.mark_spans_stale(&chapter.id).await?;
        chapter_repo.mark_segments_stale(&chapter.id).await?;
    }

    // 5. Upsert segments + source_spans, collect (segment_id, segment_text, span_ids) for processing
    let mut segment_info: Vec<(String, String, Vec<String>)> = Vec::new();

    if has_matching_segments {
        // Reuse existing segments and spans
        for segment in &existing_segments {
            if segment.chapter_hash != text_hash {
                continue;
            }
            let spans = claim_repo.list_spans_by_segment(&segment.id).await?;
            let span_ids: Vec<String> = spans.iter().map(|s| s.id.clone()).collect();
            let segment_text: String = spans
                .iter()
                .map(|s| s.text_excerpt.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            segment_info.push((segment.id.clone(), segment_text, span_ids));
        }
    } else {
        // Create new segments and spans
        let mut next_span_index: i64 = 0;
        for (seg_template, spans_data) in &segments_data {
            let segment = chapter_repo
                .create_segment(
                    book_id,
                    &chapter.id,
                    &text_hash,
                    seg_template.segment_index,
                    &seg_template.segment_type,
                    None,
                    None,
                    seg_template.start_offset,
                    seg_template.end_offset,
                    seg_template.text_hash.as_deref(),
                )
                .await?;

            let mut span_ids = Vec::new();
            for (start, end, excerpt) in spans_data {
                let span = claim_repo
                    .create_span(
                        book_id,
                        &chapter.id,
                        &text_hash,
                        &segment.id,
                        next_span_index,
                        *start,
                        *end,
                        excerpt,
                    )
                    .await?;
                span_ids.push(span.id);
                next_span_index += 1;
            }

            // Reconstruct segment text from spans
            let segment_text: String = spans_data
                .iter()
                .map(|(_, _, text)| text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");

            segment_info.push((segment.id, segment_text, span_ids));
        }
    }

    // 6. For each segment: context -> extract -> resolve -> claim_writer -> reducer
    for (segment_id, segment_text, span_ids) in &segment_info {
        // a. Create AI run for this segment
        let input_hash = crate::util::hash::md5_hex(segment_text);
        let ai_run = ai_run_repo
            .create_run(
                book_id,
                &chapter.id,
                Some(segment_id),
                "extract",
                model,
                PROMPT_VERSION,
                SCHEMA_VERSION,
                &input_hash,
            )
            .await?;

        // b. Build context
        let context = context_builder
            .build_context(book_id, chapter_index, segment_text)
            .await?;

        // c. Extract observations via trait
        let observations = extractor.extract(&context, span_ids).await?;

        // Store observations as output_json in ai_run
        let output_json = serde_json::to_string(&observations).ok();

        // d. Resolve observations against entities
        let resolved = resolver::resolve(&observations, &entity_repo, book_id).await?;

        // e. Write claims
        let claim_result = claim_writer::write_claims(
            &resolved,
            book_id,
            chapter_index,
            &ai_run.id,
            &claim_repo,
            &chapter_repo,
        )
        .await?;

        // f. Phase 2/3 pipeline: split relationship + identity claims from generic reducer input
        let mut relationship_claims = Vec::new();
        let mut identity_claims = Vec::new();
        let mut knowledge_claims = Vec::new();
        let mut location_claims = Vec::new();
        let mut other_claims = Vec::new();
        for claim in claim_result.claims_created.iter().cloned() {
            if claim.claim_type == "relationship_update" {
                relationship_claims.push(claim);
            } else if is_identity_claim_type(&claim.claim_type) {
                identity_claims.push(claim);
            } else if claim.claim_type == "knowledge_assertion" {
                knowledge_claims.push(claim);
            } else if matches!(
                claim.claim_type.as_str(),
                "location_introduction" | "location_edge"
            ) {
                location_claims.push(claim);
            } else {
                other_claims.push(claim);
            }
        }

        // g. Phase 1 reducer FIRST (entity + property) — ensures new characters enter entities
        let _reduction_result = reducer::reduce_claims(&other_claims, book_id, pool).await?;

        // h. Phase 3: process identity claims after Phase 1 entities/properties exist
        if !identity_claims.is_empty() {
            let default_identity_judge = identity_judge::DefaultIdentityJudge::new();
            let identity_judge_ref = identity_judge.unwrap_or(&default_identity_judge);
            let identity_repo = IdentityRepo::new(pool.clone());
            let active_identity_links = identity_repo
                .list_identity_links_by_book(book_id, Some("active"))
                .await
                .unwrap_or_default();
            let mut identity_reducer_claims = Vec::new();

            for claim in &identity_claims {
                let claim = re_resolve_identity_claim(book_id, &entity_repo, claim).await?;
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

                match identity_judge::structural_gate(
                    &claim,
                    entity_a.as_ref(),
                    entity_b.as_ref(),
                    blocked,
                ) {
                    IdentityGateResult::Pass => match identity_judge_ref.judge(&claim).await {
                        Ok(output) => {
                            if let Err(err) = identity_judge::validate_judge_decision(
                                &output,
                                claim.subject_entity_id.is_some()
                                    && claim.object_entity_id.is_some(),
                            ) {
                                claim_repo
                                    .update_claim_value_json(
                                        &claim.id,
                                        &merge_identity_gate_reason(
                                            claim.value_json.as_deref(),
                                            "rejected",
                                            &format!("invalid judge output: {err}"),
                                        )?,
                                    )
                                    .await?;
                                claim_repo
                                    .update_claim_status(&claim.id, "rejected")
                                    .await?;
                                continue;
                            }

                            let updated_json =
                                merge_identity_judge_output(claim.value_json.as_deref(), &output)?;
                            claim_repo
                                .update_claim_value_json(&claim.id, &updated_json)
                                .await?;
                            let mut claim_for_reducer = claim.clone();
                            claim_for_reducer.value_json = Some(updated_json);

                            match output.decision {
                                identity_judge::IdentityJudgeDecision::Reject => {
                                    claim_repo
                                        .update_claim_status(&claim.id, "rejected")
                                        .await?;
                                }
                                identity_judge::IdentityJudgeDecision::Uncertain => {
                                    claim_repo
                                        .update_claim_status(&claim.id, "uncertain")
                                        .await?;
                                }
                                identity_judge::IdentityJudgeDecision::Merge
                                | identity_judge::IdentityJudgeDecision::PossibleSameIdentity
                                | identity_judge::IdentityJudgeDecision::NotSameIdentity
                                | identity_judge::IdentityJudgeDecision::SplitRequired => {
                                    identity_reducer_claims.push(claim_for_reducer);
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!(
                                "Identity judge failed for claim {}: {}. Marking as rejected.",
                                claim.id,
                                err
                            );
                            claim_repo
                                .update_claim_status(&claim.id, "rejected")
                                .await?;
                        }
                    },
                    IdentityGateResult::Reject(reason) => {
                        claim_repo
                            .update_claim_value_json(
                                &claim.id,
                                &merge_identity_gate_reason(
                                    claim.value_json.as_deref(),
                                    "rejected",
                                    &reason,
                                )?,
                            )
                            .await?;
                        claim_repo
                            .update_claim_status(&claim.id, "rejected")
                            .await?;
                    }
                    IdentityGateResult::Uncertain(reason) => {
                        claim_repo
                            .update_claim_value_json(
                                &claim.id,
                                &merge_identity_gate_reason(
                                    claim.value_json.as_deref(),
                                    "uncertain",
                                    &reason,
                                )?,
                            )
                            .await?;
                        claim_repo
                            .update_claim_status(&claim.id, "uncertain")
                            .await?;
                    }
                }
            }

            if !identity_reducer_claims.is_empty() {
                reducer::reduce_identity_claims(&identity_reducer_claims, book_id, pool).await?;
            }
        }

        // i. Phase 2: process relationship_update claims
        if !relationship_claims.is_empty() {
            let default_judge = DefaultJudge::new();
            let judge_ref = judge.unwrap_or(&default_judge);

            // Re-resolve relationship claims using newly created entities.
            // After Phase 1 reduction, new characters may have been added to entities.
            let mut re_resolved_claims = Vec::new();
            for rel_claim in &relationship_claims {
                let mut claim = rel_claim.clone();
                if let Some(ref subject_mention) = claim.subject_mention {
                    if let Ok(Some(entity)) = entity_repo
                        .find_entity_by_alias(book_id, subject_mention)
                        .await
                    {
                        claim.subject_entity_id = Some(entity.id);
                    }
                }
                if let Some(ref object_mention) = claim.object_mention {
                    if let Ok(Some(entity)) = entity_repo
                        .find_entity_by_alias(book_id, object_mention)
                        .await
                    {
                        claim.object_entity_id = Some(entity.id);
                    }
                }
                re_resolved_claims.push(claim);
            }

            // Run Structural Gate on each relationship claim
            let mut gate_accepted = Vec::new();
            let mut redirect_claims = Vec::new();

            for claim in &re_resolved_claims {
                let subject_entity = match &claim.subject_entity_id {
                    Some(id) => entity_repo.get_by_id(id).await?,
                    None => None,
                };
                let object_entity = match &claim.object_entity_id {
                    Some(id) => entity_repo.get_by_id(id).await?,
                    None => None,
                };

                let gate_result = relationship_judge::structural_gate(
                    claim,
                    subject_entity.as_ref(),
                    object_entity.as_ref(),
                );

                match gate_result {
                    GateResult::Pass => {
                        gate_accepted.push(claim.clone());
                    }
                    GateResult::Reject(reason) => {
                        tracing::debug!(
                            "Relationship claim {} rejected by structural gate: {}",
                            claim.id,
                            reason
                        );
                        claim_repo
                            .update_claim_status(&claim.id, "rejected")
                            .await?;
                    }
                    GateResult::Redirect {
                        target,
                        dimension_key,
                    } => {
                        if target == "property_update" {
                            if let Some(dim_key) = dimension_key {
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
                                        &ai_run.id,
                                        claim.confidence,
                                        "low",
                                    )
                                    .await?;
                                redirect_claims.push(derived_claim);
                            }
                        }
                        claim_repo
                            .update_claim_status(&claim.id, "redirected")
                            .await?;
                    }
                    GateResult::Uncertain(reason) => {
                        tracing::debug!("Relationship claim {} uncertain: {}", claim.id, reason);
                        claim_repo
                            .update_claim_status(&claim.id, "uncertain")
                            .await?;
                    }
                }
            }

            // Run AI Semantic Judge on gate-accepted claims
            let mut judge_accepted = Vec::new();
            for claim in &gate_accepted {
                match judge_ref.judge(claim).await {
                    Ok(judge_output) => {
                        match judge_output.decision {
                            JudgeDecision::Accept => {
                                let mut accepted_claim = claim.clone();
                                // Write normalized fields back to claim.value_json
                                let mut vj: serde_json::Value = accepted_claim
                                    .value_json
                                    .as_ref()
                                    .and_then(|j| serde_json::from_str(j).ok())
                                    .unwrap_or(serde_json::Value::Null);
                                if let serde_json::Value::Object(ref mut map) = vj {
                                    if let Some(group) = &judge_output.normalized_relation_group {
                                        map.insert(
                                            "normalized_relation_group".to_string(),
                                            serde_json::Value::String(group.clone()),
                                        );
                                    }
                                    if let Some(label) = &judge_output.normalized_relation_label {
                                        map.insert(
                                            "normalized_relation_label".to_string(),
                                            serde_json::Value::String(label.clone()),
                                        );
                                    }
                                    if let Some(dir) = &judge_output.directionality {
                                        map.insert(
                                            "directionality".to_string(),
                                            serde_json::Value::String(dir.clone()),
                                        );
                                    }
                                    if let Some(state) = &judge_output.current_state {
                                        map.insert(
                                            "current_state".to_string(),
                                            serde_json::Value::String(state.clone()),
                                        );
                                    }
                                    if let Some(strength) = judge_output.strength {
                                        map.insert(
                                            "strength".to_string(),
                                            serde_json::Value::Number(
                                                serde_json::Number::from_f64(strength)
                                                    .unwrap_or(serde_json::Number::from(0)),
                                            ),
                                        );
                                    }
                                    if let Some(polarity) = &judge_output.polarity {
                                        map.insert(
                                            "polarity".to_string(),
                                            serde_json::Value::String(polarity.clone()),
                                        );
                                    }
                                    if let Some(importance) = judge_output.importance_score {
                                        map.insert(
                                            "importance_score".to_string(),
                                            serde_json::Value::Number(
                                                serde_json::Number::from_f64(importance)
                                                    .unwrap_or(serde_json::Number::from(0)),
                                            ),
                                        );
                                    }
                                    map.insert(
                                        "judge_confidence".to_string(),
                                        serde_json::Value::Number(
                                            serde_json::Number::from_f64(judge_output.confidence)
                                                .unwrap_or(serde_json::Number::from(0)),
                                        ),
                                    );
                                    map.insert(
                                        "judge_reason_code".to_string(),
                                        serde_json::Value::String(judge_output.reason_code.clone()),
                                    );
                                }
                                accepted_claim.value_json = Some(vj.to_string());
                                claim_repo
                                    .update_claim_value_json(&claim.id, &vj.to_string())
                                    .await?;
                                judge_accepted.push(accepted_claim);
                            }
                            JudgeDecision::Reject => {
                                claim_repo
                                    .update_claim_status(&claim.id, "rejected")
                                    .await?;
                            }
                            JudgeDecision::Redirect => {
                                let redirect_to =
                                    judge_output.redirect_to.as_deref().unwrap_or("minor_event");
                                let dim_key = judge_output.redirect_dimension_key.clone();

                                if redirect_to == "property_update" {
                                    if let Some(dim_key) = dim_key {
                                        let value_text =
                                            claim.object_mention.clone().unwrap_or_default();
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
                                                &ai_run.id,
                                                judge_output.confidence,
                                                "low",
                                            )
                                            .await?;
                                        redirect_claims.push(derived_claim);
                                    }
                                }
                                // minor_event redirects: ledger-only, no canonical processing
                                claim_repo
                                    .update_claim_status(&claim.id, "redirected")
                                    .await?;
                            }
                            JudgeDecision::Uncertain => {
                                claim_repo
                                    .update_claim_status(&claim.id, "uncertain")
                                    .await?;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "AI judge failed for claim {}: {}. Marking as rejected.",
                            claim.id,
                            e
                        );
                        claim_repo
                            .update_claim_status(&claim.id, "rejected")
                            .await?;
                    }
                }
            }

            // Run relationship_reducer on accepted claims (independent transaction)
            let _rel_reduction =
                reducer::reduce_relationship_claims(&judge_accepted, book_id, pool).await?;

            // Second-pass property_reducer for derived redirect property_update claims
            if !redirect_claims.is_empty() {
                let _redirect_reduction =
                    reducer::reduce_claims(&redirect_claims, book_id, pool).await?;
            }
        }

        // j. Phase 4: process knowledge assertions after Phase 1/2/3 canonical writes
        if !knowledge_claims.is_empty() {
            let default_knowledge_judge = knowledge_judge::DefaultKnowledgeRevisionJudge::new();
            let knowledge_judge_ref = knowledge_judge.unwrap_or(&default_knowledge_judge);
            process_knowledge_claims_for_segment(
                book_id,
                chapter_index,
                pool,
                knowledge_judge_ref,
                &claim_repo,
                segment_id,
                &knowledge_claims,
            )
            .await?;
        }

        // k. Phase 5: process place/map claims after Phase 1-4 canonical writes
        if !location_claims.is_empty() {
            let default_map_judge = map_conflict_judge::DefaultMapConflictJudge::new();
            let map_judge_ref = map_conflict_judge.unwrap_or(&default_map_judge);
            if let Err(err) = process_location_claims_for_segment(
                book_id,
                chapter_index,
                pool,
                map_judge_ref,
                &claim_repo,
                &entity_repo,
                &location_claims,
            )
            .await
            {
                tracing::warn!(
                    "Phase 5 map processing failed for book {} chapter {}: {}",
                    book_id,
                    chapter_index,
                    err
                );
                let claim_ids = location_claims
                    .iter()
                    .map(|claim| claim.id.clone())
                    .collect::<Vec<_>>();
                if !claim_ids.is_empty() {
                    let _ = claim_repo
                        .batch_update_claim_status(&claim_ids, "uncertain")
                        .await;
                }
            }
        }

        // l. Mark AI run as success with output_json
        ai_run_repo
            .update_run_status(&ai_run.id, "success", output_json.as_deref(), None)
            .await?;
    }

    // 7. Invalidate + rebuild projection cache
    projection::invalidate_book_cache(book_id, pool).await?;
    relationship_projection::invalidate_relationship_cache(book_id, pool).await?;

    // Force rebuild by projecting (populates cache)
    let _ = projection::project_character_list(book_id, chapter_index, pool).await;
    let _ = relationship_projection::project_relationship_graph(book_id, chapter_index, pool).await;

    // 8. Mark chapter processing run as success
    progress_repo.complete_run(&processing_run.id).await?;

    // 9. Update processing_progress.max_processed_chapter
    progress_repo
        .advance_processed_chapter(book_id, chapter_index)
        .await?;

    Ok(())
}

async fn process_location_claims_for_segment(
    book_id: &str,
    chapter_index: i64,
    pool: &SqlitePool,
    map_judge: &dyn MapConflictJudge,
    claim_repo: &ClaimRepo,
    entity_repo: &EntityRepo,
    location_claims: &[ClaimRecord],
) -> anyhow::Result<()> {
    let identity_repo = IdentityRepo::new(pool.clone());
    let place_resolver = PlaceResolver::new(EntityRepo::new(pool.clone()), identity_repo);
    let mut introduction_claims = Vec::new();
    let mut edge_claims = Vec::new();

    for claim in location_claims
        .iter()
        .filter(|claim| claim.claim_type == "location_introduction")
    {
        match prepare_location_introduction_claim(&place_resolver, claim_repo, pool, book_id, claim)
            .await?
        {
            Some(prepared) => introduction_claims.push(prepared),
            None => {
                claim_repo
                    .update_claim_status(&claim.id, "uncertain")
                    .await?;
            }
        }
    }

    if !introduction_claims.is_empty() {
        place_reducer::reduce_location_claims(&introduction_claims, book_id, pool).await?;
    }

    let existing_parent_links = load_existing_parent_links(book_id, pool).await?;
    for claim in location_claims
        .iter()
        .filter(|claim| claim.claim_type == "location_edge")
    {
        match prepare_location_edge_claim(
            &place_resolver,
            claim_repo,
            entity_repo,
            pool,
            book_id,
            chapter_index,
            map_judge,
            claim,
            &existing_parent_links,
        )
        .await?
        {
            Some(prepared) => edge_claims.push(prepared),
            None => {}
        }
    }

    if !edge_claims.is_empty() {
        place_reducer::reduce_location_claims(&edge_claims, book_id, pool).await?;
    }

    Ok(())
}

async fn prepare_location_introduction_claim(
    place_resolver: &PlaceResolver,
    claim_repo: &ClaimRepo,
    pool: &SqlitePool,
    book_id: &str,
    claim: &ClaimRecord,
) -> anyhow::Result<Option<ClaimRecord>> {
    let mut value = parse_value_json_object(claim.value_json.as_deref());
    let place_type = value
        .get("place_type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let parent_place_mention = value
        .get("parent_place_mention")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let aliases = value
        .get("aliases")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let Some(place_mention) = claim.subject_mention.as_deref() else {
        return Ok(None);
    };
    let resolution = place_resolver
        .resolve_place(
            book_id,
            place_mention,
            &place_type,
            parent_place_mention.as_deref(),
            &aliases,
            claim.confidence,
        )
        .await?;
    if resolution.action == PlaceResolutionAction::Uncertain {
        return Ok(None);
    }

    if let Some(parent_id) = resolution.parent_place_id {
        value.insert(
            "parent_place_id".to_string(),
            serde_json::Value::String(parent_id),
        );
    }
    if let Some(org_id) = resolution.organization_link_candidate_id {
        value.insert(
            "organization_link_candidate_id".to_string(),
            serde_json::Value::String(org_id),
        );
        value
            .entry("entity_link_type".to_string())
            .or_insert_with(|| serde_json::Value::String("organization_place_pair".to_string()));
    }
    insert_claim_evidence_span_ids(&mut value, claim);

    let updated_json = serde_json::Value::Object(value).to_string();
    claim_repo
        .update_claim_value_json(&claim.id, &updated_json)
        .await?;
    let mut prepared = claim.clone();
    prepared.subject_entity_id = resolution.place_entity_id;
    prepared.value_json = Some(updated_json);
    persist_claim_entity_ids(pool, &prepared).await?;
    Ok(Some(prepared))
}

async fn prepare_location_edge_claim(
    place_resolver: &PlaceResolver,
    claim_repo: &ClaimRepo,
    entity_repo: &EntityRepo,
    pool: &SqlitePool,
    book_id: &str,
    chapter_index: i64,
    map_judge: &dyn MapConflictJudge,
    claim: &ClaimRecord,
    existing_parent_links: &[(String, String)],
) -> anyhow::Result<Option<ClaimRecord>> {
    let mut value = parse_value_json_object(claim.value_json.as_deref());
    let Some(from_mention) = claim.subject_mention.as_deref() else {
        claim_repo
            .update_claim_status(&claim.id, "uncertain")
            .await?;
        return Ok(None);
    };
    let Some(to_mention) = claim.object_mention.as_deref() else {
        claim_repo
            .update_claim_status(&claim.id, "uncertain")
            .await?;
        return Ok(None);
    };

    let from_resolution = place_resolver
        .resolve_place(
            book_id,
            from_mention,
            "unknown",
            None,
            &[],
            claim.confidence,
        )
        .await?;
    let to_resolution = place_resolver
        .resolve_place(book_id, to_mention, "unknown", None, &[], claim.confidence)
        .await?;
    let (Some(from_place_id), Some(to_place_id)) = (
        from_resolution.place_entity_id.clone(),
        to_resolution.place_entity_id.clone(),
    ) else {
        claim_repo
            .update_claim_status(&claim.id, "uncertain")
            .await?;
        return Ok(None);
    };

    value.insert(
        "from_place_id".to_string(),
        serde_json::Value::String(from_place_id.clone()),
    );
    value.insert(
        "to_place_id".to_string(),
        serde_json::Value::String(to_place_id.clone()),
    );
    insert_claim_evidence_span_ids(&mut value, claim);

    let mut prepared = claim.clone();
    prepared.subject_entity_id = Some(from_place_id.clone());
    prepared.object_entity_id = Some(to_place_id.clone());
    prepared.value_json = Some(serde_json::Value::Object(value.clone()).to_string());
    persist_claim_entity_ids(pool, &prepared).await?;

    let from_place = entity_repo.get_by_id(&from_place_id).await?;
    let to_place = entity_repo.get_by_id(&to_place_id).await?;
    match map_gate::structural_gate(MapGateContext {
        claim: &prepared,
        from_place: from_place.as_ref(),
        to_place: to_place.as_ref(),
        existing_parent_links,
    }) {
        MapGateResult::Pass => {}
        MapGateResult::Reject(reason) => {
            merge_map_gate_reason(&mut value, "reject", &reason);
            let updated_json = serde_json::Value::Object(value).to_string();
            claim_repo
                .update_claim_value_json(&claim.id, &updated_json)
                .await?;
            set_claim_risk_level(pool, &claim.id, "medium").await?;
            prepared.risk_level = "medium".to_string();
            prepared.value_json = Some(updated_json);
            return Ok(Some(prepared));
        }
        MapGateResult::Uncertain(reason) => {
            merge_map_gate_reason(&mut value, "uncertain", &reason);
            let updated_json = serde_json::Value::Object(value).to_string();
            claim_repo
                .update_claim_value_json(&claim.id, &updated_json)
                .await?;
            claim_repo
                .update_claim_status(&claim.id, "uncertain")
                .await?;
            return Ok(None);
        }
    }

    let edge_type = value
        .get("edge_type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("connected_to")
        .to_string();
    let judge_input = MapConflictJudgeInput {
        book_id: book_id.to_string(),
        chapter_index,
        claim_id: claim.id.clone(),
        claim_type: claim.claim_type.clone(),
        from_place_mention: from_mention.to_string(),
        to_place_mention: to_mention.to_string(),
        edge_type,
        evidence_spans: claim_evidence_span_ids(claim),
        existing_map_context: load_existing_map_context(book_id, pool).await?,
        boundary_warnings: Vec::new(),
    };
    let judge_output = map_judge.judge(&judge_input).await?;
    merge_map_judge_output(&mut value, &judge_output)?;
    let updated_json = serde_json::Value::Object(value).to_string();
    claim_repo
        .update_claim_value_json(&claim.id, &updated_json)
        .await?;
    prepared.value_json = Some(updated_json);

    if judge_output.decision == MapConflictDecision::Uncertain {
        claim_repo
            .update_claim_status(&claim.id, "uncertain")
            .await?;
        return Ok(None);
    }

    set_claim_risk_level(pool, &claim.id, "medium").await?;
    prepared.risk_level = "medium".to_string();
    Ok(Some(prepared))
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

async fn set_claim_risk_level(
    pool: &SqlitePool,
    claim_id: &str,
    risk_level: &str,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE claims SET risk_level = ?, updated_at = ? WHERE id = ?")
        .bind(risk_level)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(claim_id)
        .execute(pool)
        .await?;
    Ok(())
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

fn insert_claim_evidence_span_ids(
    value: &mut serde_json::Map<String, serde_json::Value>,
    claim: &ClaimRecord,
) {
    let ids = claim_evidence_span_ids(claim)
        .into_iter()
        .map(serde_json::Value::String)
        .collect::<Vec<_>>();
    value.insert(
        "evidence_span_ids".to_string(),
        serde_json::Value::Array(ids),
    );
}

fn claim_evidence_span_ids(claim: &ClaimRecord) -> Vec<String> {
    let primary = claim.primary_source_span_id.trim();
    if primary.is_empty() {
        Vec::new()
    } else {
        vec![primary.to_string()]
    }
}

fn merge_map_gate_reason(
    value: &mut serde_json::Map<String, serde_json::Value>,
    decision: &str,
    reason: &str,
) {
    value.insert(
        "judge_decision".to_string(),
        serde_json::Value::String(decision.to_string()),
    );
    value.insert(
        "reason_code".to_string(),
        serde_json::Value::String(reason.to_string()),
    );
}

fn merge_map_judge_output(
    value: &mut serde_json::Map<String, serde_json::Value>,
    output: &map_conflict_judge::MapConflictJudgeOutput,
) -> anyhow::Result<()> {
    let decision = match output.decision {
        MapConflictDecision::Accept => "accept",
        MapConflictDecision::Conflict => "conflict",
        MapConflictDecision::Uncertain => "uncertain",
        MapConflictDecision::Reject => "reject",
    };
    value.insert(
        "judge_decision".to_string(),
        serde_json::Value::String(decision.to_string()),
    );
    if let Some(edge_type) = &output.normalized_edge_type {
        value.insert(
            "normalized_edge_type".to_string(),
            serde_json::Value::String(edge_type.clone()),
        );
    }
    if let Some(direction) = &output.normalized_direction_hint {
        value.insert(
            "normalized_direction_hint".to_string(),
            serde_json::Value::String(direction.clone()),
        );
    }
    if let Some(distance) = &output.normalized_distance_hint {
        value.insert(
            "normalized_distance_hint".to_string(),
            serde_json::Value::String(distance.clone()),
        );
    }
    if let Some(conflict_type) = &output.conflict_type {
        value.insert(
            "conflict_type".to_string(),
            serde_json::Value::String(conflict_type.clone()),
        );
    }
    value.insert(
        "reason_code".to_string(),
        serde_json::Value::String(output.reason_code.clone()),
    );
    value.insert(
        "judge_confidence".to_string(),
        serde_json::Value::Number(
            serde_json::Number::from_f64(output.confidence)
                .ok_or_else(|| anyhow::anyhow!("invalid map judge confidence"))?,
        ),
    );
    value.insert("judge_output".to_string(), serde_json::to_value(output)?);
    Ok(())
}

async fn process_knowledge_claims_for_segment(
    book_id: &str,
    chapter_index: i64,
    pool: &SqlitePool,
    knowledge_judge: &dyn KnowledgeRevisionJudge,
    claim_repo: &ClaimRepo,
    segment_id: &str,
    knowledge_claims: &[ClaimRecord],
) -> anyhow::Result<()> {
    let knowledge_repo = KnowledgeRepo::new(pool.clone());
    let topic_resolver = TopicResolver::new(knowledge_repo);
    let source_spans = claim_repo.list_spans_by_segment(segment_id).await?;
    let mut reducer_claims = Vec::new();
    let mut judge_outputs = HashMap::new();
    let entity_repo = EntityRepo::new(pool.clone());

    for original_claim in knowledge_claims {
        let claim = re_resolve_knowledge_claim_references(
            book_id,
            &entity_repo,
            claim_repo,
            original_claim,
        )
        .await?;
        let fields = match parse_knowledge_claim_value(&claim) {
            Ok(fields) => fields,
            Err(err) => {
                claim_repo
                    .update_claim_status(&claim.id, "rejected")
                    .await?;
                tracing::debug!("Knowledge claim {} rejected: {}", claim.id, err);
                continue;
            }
        };
        let topic_resolution = topic_resolver
            .resolve(book_id, &fields.category, &fields.topic, chapter_index)
            .await?;

        match knowledge_judge::structural_gate(&claim, &source_spans, Some(&topic_resolution)) {
            KnowledgeGateResult::Pass => {}
            KnowledgeGateResult::Reject(reason) => {
                claim_repo
                    .update_claim_status(&claim.id, "rejected")
                    .await?;
                tracing::debug!(
                    "Knowledge claim {} rejected by structural gate: {}",
                    claim.id,
                    reason
                );
                continue;
            }
            KnowledgeGateResult::Uncertain(reason) => {
                claim_repo
                    .update_claim_status(&claim.id, "uncertain")
                    .await?;
                tracing::debug!("Knowledge claim {} uncertain: {}", claim.id, reason);
                continue;
            }
        }

        let input = build_knowledge_judge_input(
            book_id,
            chapter_index,
            &claim,
            &fields,
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
                    claim_repo
                        .update_claim_status(&claim.id, "rejected")
                        .await?;
                    tracing::warn!(
                        "Knowledge judge output invalid for claim {}: {}",
                        claim.id,
                        err
                    );
                    continue;
                }
                if output.decision == knowledge_judge::KnowledgeJudgeDecision::Reject {
                    claim_repo
                        .update_claim_status(&claim.id, "rejected")
                        .await?;
                    continue;
                }
                if output.card_action == knowledge_judge::KnowledgeCardAction::Uncertain {
                    claim_repo
                        .update_claim_status(&claim.id, "uncertain")
                        .await?;
                    continue;
                }

                let value_json =
                    merge_knowledge_judge_output(claim.value_json.as_deref(), &output)?;
                claim_repo
                    .update_claim_value_json(&claim.id, &value_json)
                    .await?;
                let mut reducer_claim = claim.clone();
                reducer_claim.value_json = Some(value_json);
                judge_outputs.insert(claim.id.clone(), output);
                reducer_claims.push(reducer_claim);
            }
            Err(err) => {
                claim_repo
                    .update_claim_status(&claim.id, "rejected")
                    .await?;
                tracing::warn!("Knowledge judge failed for claim {}: {}", claim.id, err);
            }
        }
    }

    if !reducer_claims.is_empty() {
        if let Err(err) =
            reducer::reduce_knowledge_claims(&reducer_claims, book_id, pool, &judge_outputs).await
        {
            tracing::warn!(
                "Knowledge reducer failed for book {} chapter {}: {}. Marking knowledge claims uncertain.",
                book_id,
                chapter_index,
                err
            );
            for claim in &reducer_claims {
                claim_repo
                    .update_claim_status(&claim.id, "uncertain")
                    .await?;
            }
        }
    }

    Ok(())
}

async fn re_resolve_knowledge_claim_references(
    book_id: &str,
    entity_repo: &EntityRepo,
    claim_repo: &ClaimRepo,
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

    let updated_json = value.to_string();
    claim_repo
        .update_claim_value_json(&claim.id, &updated_json)
        .await?;
    let mut updated_claim = claim.clone();
    updated_claim.value_json = Some(updated_json);
    Ok(updated_claim)
}

struct KnowledgePipelineClaimFields {
    category: String,
    topic: String,
    assertion_text: String,
    status_hint: Option<String>,
}

fn parse_knowledge_claim_value(
    claim: &ClaimRecord,
) -> anyhow::Result<KnowledgePipelineClaimFields> {
    let value: serde_json::Value = serde_json::from_str(
        claim
            .value_json
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("knowledge claim missing value_json"))?,
    )?;
    let category = required_json_string(&value, "category")
        .ok_or_else(|| anyhow::anyhow!("knowledge claim missing category"))?;
    let topic = required_json_string(&value, "topic_display")
        .or_else(|| required_json_string(&value, "raw_topic"))
        .ok_or_else(|| anyhow::anyhow!("knowledge claim missing topic"))?;
    let assertion_text = required_json_string(&value, "assertion_text")
        .or_else(|| claim.value_text.clone())
        .ok_or_else(|| anyhow::anyhow!("knowledge claim missing assertion_text"))?;
    let status_hint = required_json_string(&value, "status_hint");
    Ok(KnowledgePipelineClaimFields {
        category,
        topic,
        assertion_text,
        status_hint,
    })
}

async fn build_knowledge_judge_input(
    book_id: &str,
    chapter_index: i64,
    claim: &ClaimRecord,
    fields: &KnowledgePipelineClaimFields,
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
        category: fields.category.clone(),
        assertion_text: fields.assertion_text.clone(),
        status_hint: fields.status_hint.clone(),
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

fn merge_knowledge_judge_output(
    current_value_json: Option<&str>,
    output: &knowledge_judge::KnowledgeJudgeOutput,
) -> anyhow::Result<String> {
    let mut value = parse_value_json_object(current_value_json);
    value.insert(
        "judge_decision".to_string(),
        serde_json::Value::String(format!("{:?}", output.decision)),
    );
    value.insert(
        "card_action".to_string(),
        serde_json::Value::String(format!("{:?}", output.card_action)),
    );
    if let Some(card_id) = &output.target_card_id {
        value.insert(
            "target_card_id".to_string(),
            serde_json::Value::String(card_id.clone()),
        );
    }
    value.insert(
        "assertion_status".to_string(),
        serde_json::Value::String(output.assertion_status.clone()),
    );
    value.insert(
        "judge_confidence".to_string(),
        serde_json::Value::Number(
            serde_json::Number::from_f64(output.confidence)
                .unwrap_or_else(|| serde_json::Number::from(0)),
        ),
    );
    value.insert(
        "reason_code".to_string(),
        serde_json::Value::String(output.reason_code.clone()),
    );
    value.insert(
        "explanation_for_log".to_string(),
        serde_json::Value::String(output.explanation_for_log.clone()),
    );
    Ok(serde_json::Value::Object(value).to_string())
}

fn required_json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn is_identity_claim_type(claim_type: &str) -> bool {
    matches!(
        claim_type,
        "identity_reveal"
            | "entity_merge_candidate"
            | "entity_split_candidate"
            | "not_same_identity"
    )
}

async fn re_resolve_identity_claim(
    book_id: &str,
    entity_repo: &EntityRepo,
    claim: &ClaimRecord,
) -> anyhow::Result<ClaimRecord> {
    let mut updated = claim.clone();
    if updated.subject_entity_id.is_none() {
        if let Some(mention) = updated.subject_mention.as_deref() {
            updated.subject_entity_id =
                find_entity_id_for_mention(entity_repo, book_id, mention).await?;
        }
    }
    if updated.object_entity_id.is_none() {
        if let Some(mention) = updated.object_mention.as_deref() {
            updated.object_entity_id =
                find_entity_id_for_mention(entity_repo, book_id, mention).await?;
        }
    }
    Ok(updated)
}

async fn find_entity_id_for_mention(
    entity_repo: &EntityRepo,
    book_id: &str,
    mention: &str,
) -> anyhow::Result<Option<String>> {
    if let Some(entity) = entity_repo.find_entity_by_alias(book_id, mention).await? {
        return Ok(Some(entity.id));
    }
    Ok(entity_repo
        .get_by_canonical_name(book_id, mention)
        .await?
        .map(|entity| entity.id))
}

async fn persist_claim_entity_ids(pool: &SqlitePool, claim: &ClaimRecord) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE claims SET subject_entity_id = ?, object_entity_id = ?, updated_at = ? WHERE id = ?",
    )
    .bind(claim.subject_entity_id.as_deref())
    .bind(claim.object_entity_id.as_deref())
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(&claim.id)
    .execute(pool)
    .await?;
    Ok(())
}

fn blocked_by_active_not_same_identity(
    active_links: &[IdentityLinkRecord],
    left: Option<&str>,
    right: Option<&str>,
) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    let pair = canonicalize_identity_pair(left, right);
    active_links.iter().any(|link| {
        link.status == "active"
            && link.link_type == "not_same_identity"
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

fn merge_identity_judge_output(
    current_value_json: Option<&str>,
    output: &identity_judge::IdentityJudgeOutput,
) -> anyhow::Result<String> {
    let mut value = parse_value_json_object(current_value_json);
    value.insert(
        "judge_decision".to_string(),
        serde_json::Value::String(
            match &output.decision {
                identity_judge::IdentityJudgeDecision::Merge => "merge",
                identity_judge::IdentityJudgeDecision::PossibleSameIdentity => {
                    "possible_same_identity"
                }
                identity_judge::IdentityJudgeDecision::NotSameIdentity => "not_same_identity",
                identity_judge::IdentityJudgeDecision::Reject => "reject",
                identity_judge::IdentityJudgeDecision::Uncertain => "uncertain",
                identity_judge::IdentityJudgeDecision::SplitRequired => "split_required",
            }
            .to_string(),
        ),
    );
    value.insert(
        "judge_confidence".to_string(),
        serde_json::Value::Number(
            serde_json::Number::from_f64(output.confidence)
                .unwrap_or_else(|| serde_json::Number::from(0)),
        ),
    );
    value.insert(
        "reason_code".to_string(),
        serde_json::Value::String(output.reason_code.clone()),
    );
    value.insert(
        "link_type".to_string(),
        serde_json::Value::String(output.link_type.clone()),
    );
    value.insert(
        "survivor_hint".to_string(),
        serde_json::Value::String(output.survivor_hint.clone()),
    );
    value.insert(
        "explanation_for_log".to_string(),
        serde_json::Value::String(output.explanation_for_log.clone()),
    );
    value.insert(
        "property_conflicts".to_string(),
        serde_json::Value::Array(
            output
                .property_conflicts
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
    value.insert(
        "relationship_migration_hint".to_string(),
        serde_json::Value::String(output.relationship_migration_hint.clone()),
    );
    Ok(serde_json::Value::Object(value).to_string())
}

fn merge_identity_gate_reason(
    current_value_json: Option<&str>,
    gate_decision: &str,
    reason: &str,
) -> anyhow::Result<String> {
    let mut value = parse_value_json_object(current_value_json);
    value.insert(
        "judge_decision".to_string(),
        serde_json::Value::String(gate_decision.to_string()),
    );
    value.insert(
        "gate_reason".to_string(),
        serde_json::Value::String(reason.to_string()),
    );
    Ok(serde_json::Value::Object(value).to_string())
}

fn parse_value_json_object(
    current_value_json: Option<&str>,
) -> serde_json::Map<String, serde_json::Value> {
    current_value_json
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::extractor::MockExtractor;
    use crate::storage::db;
    use crate::storage::db::v4::relationship_repo::RelationshipRepo;

    async fn setup_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("v4-pipeline-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        db::v4::init_v4(&pool).await.unwrap();
        pool
    }

    fn empty_extractor() -> MockExtractor {
        MockExtractor::new(vec![])
    }

    /// Mock judge that always accepts with configurable group/label.
    fn mock_accept_judge() -> MockJudge {
        MockJudge::new(JudgeOutput {
            decision: JudgeDecision::Accept,
            reason_code: "test_accept".to_string(),
            confidence: 0.9,
            normalized_relation_group: Some("friendship".to_string()),
            normalized_relation_label: Some("friends".to_string()),
            directionality: Some("undirected".to_string()),
            current_state: Some("close".to_string()),
            strength: Some(0.7),
            polarity: Some("positive".to_string()),
            importance_score: Some(0.8),
            redirect_to: None,
            redirect_dimension_key: None,
            explanation_for_log: "test accept".to_string(),
        })
    }

    /// Make a RelationshipUpdate observation with configurable fields.
    fn make_rel_obs(
        subject: &str,
        object: &str,
        group: &str,
        label: &str,
        confidence: f64,
    ) -> crate::service::v4::extractor::Observation {
        crate::service::v4::extractor::Observation::RelationshipUpdate {
            subject_mention: subject.to_string(),
            object_mention: object.to_string(),
            relation_hint: label.to_string(),
            relation_group: group.to_string(),
            relation_label: label.to_string(),
            directionality: "undirected".to_string(),
            evidence_span_ids: vec![],
            confidence,
            importance_hint: 0.7,
            is_long_term_or_significant_hint: true,
        }
    }

    /// Simple entity introduction observation.
    fn make_entity_obs(mention: &str) -> crate::service::v4::extractor::Observation {
        crate::service::v4::extractor::Observation::EntityIntroduction {
            subject_mention: mention.to_string(),
            entity_type: "character".to_string(),
            aliases: vec![],
            short_summary: format!("{} - a character", mention),
            evidence_span_ids: vec![],
            confidence: 0.9,
        }
    }

    fn make_identity_reveal_obs(
        revealed: &str,
        canonical: &str,
    ) -> crate::service::v4::extractor::Observation {
        crate::service::v4::extractor::Observation::IdentityReveal {
            revealed_mention: revealed.to_string(),
            canonical_mention: canonical.to_string(),
            reveal_type: "disguise".to_string(),
            reason_hint: Some("摘下面具".to_string()),
            evidence_span_ids: vec![],
            confidence: 0.95,
        }
    }

    fn make_identity_merge_candidate_obs(
        entity_a: &str,
        entity_b: &str,
        reason_hint: &str,
    ) -> crate::service::v4::extractor::Observation {
        crate::service::v4::extractor::Observation::EntityMergeCandidate {
            entity_a_mention: entity_a.to_string(),
            entity_b_mention: entity_b.to_string(),
            reason_hint: Some(reason_hint.to_string()),
            evidence_span_ids: vec![],
            confidence: 0.91,
        }
    }

    fn make_not_same_identity_obs(
        entity_a: &str,
        entity_b: &str,
    ) -> crate::service::v4::extractor::Observation {
        crate::service::v4::extractor::Observation::NotSameIdentity {
            entity_a_mention: entity_a.to_string(),
            entity_b_mention: entity_b.to_string(),
            reason_hint: Some("文本明确说明二者并非同一人".to_string()),
            evidence_span_ids: vec![],
            confidence: 0.96,
        }
    }

    fn make_knowledge_obs(
        category: &str,
        topic: &str,
        assertion_text: &str,
    ) -> crate::service::v4::extractor::Observation {
        crate::service::v4::extractor::Observation::KnowledgeAssertion {
            category: category.to_string(),
            topic: topic.to_string(),
            assertion_text: assertion_text.to_string(),
            confidence: 0.91,
            importance_score: 0.82,
            evidence_span_ids: vec![],
            referenced_entity_mentions: vec![],
            status_hint: Some("fact".to_string()),
            reason_hint: Some("long-term world rule".to_string()),
        }
    }

    fn make_location_intro_obs(
        place: &str,
        place_type: &str,
    ) -> crate::service::v4::extractor::Observation {
        crate::service::v4::extractor::Observation::LocationIntroduction {
            place_mention: place.to_string(),
            place_type: place_type.to_string(),
            parent_place_mention: None,
            aliases: vec![],
            description: Some(format!("{place} is a place.")),
            importance_score: 0.8,
            map_visible_hint: Some(true),
            evidence_span_ids: vec![],
            confidence: 0.9,
        }
    }

    fn make_location_edge_obs(
        from: &str,
        to: &str,
        edge_type: &str,
    ) -> crate::service::v4::extractor::Observation {
        crate::service::v4::extractor::Observation::LocationEdge {
            from_place_mention: from.to_string(),
            to_place_mention: to.to_string(),
            edge_type: edge_type.to_string(),
            direction_hint: None,
            distance_hint: Some("三日路程".to_string()),
            evidence_span_ids: vec![],
            confidence: 0.9,
            is_topological_hint: true,
        }
    }

    fn mock_map_accept_judge() -> crate::service::v4::map_conflict_judge::MockMapConflictJudge {
        crate::service::v4::map_conflict_judge::MockMapConflictJudge::new(
            crate::service::v4::map_conflict_judge::MapConflictJudgeOutput {
                decision: crate::service::v4::map_conflict_judge::MapConflictDecision::Accept,
                normalized_edge_type: Some("route_to".to_string()),
                normalized_direction_hint: None,
                normalized_distance_hint: Some("三日路程".to_string()),
                conflict_type: None,
                reason_code: "test_accept".to_string(),
                confidence: 0.92,
                explanation_for_log: "test accepts map edge".to_string(),
            },
        )
    }

    fn make_knowledge_obs_with_entity_ref(
        category: &str,
        topic: &str,
        assertion_text: &str,
        entity_mention: &str,
        role: &str,
    ) -> crate::service::v4::extractor::Observation {
        crate::service::v4::extractor::Observation::KnowledgeAssertion {
            category: category.to_string(),
            topic: topic.to_string(),
            assertion_text: assertion_text.to_string(),
            confidence: 0.91,
            importance_score: 0.82,
            evidence_span_ids: vec![],
            referenced_entity_mentions: vec![
                crate::service::v4::extractor::KnowledgeEntityMention {
                    mention: entity_mention.to_string(),
                    entity_type_hint: Some("character".to_string()),
                    role: role.to_string(),
                    confidence: 0.9,
                    resolved_entity_id: None,
                },
            ],
            status_hint: Some("fact".to_string()),
            reason_hint: Some("long-term world rule with entity reference".to_string()),
        }
    }

    #[tokio::test]
    async fn phase5_pipeline_creates_place_and_edge() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let extractor = MockExtractor::new(vec![
            make_location_intro_obs("青云城", "city"),
            make_location_intro_obs("黑风谷", "dungeon"),
            make_location_edge_obs("青云城", "黑风谷", "route_to"),
        ]);
        let map_judge = mock_map_accept_judge();

        process_chapter_with_map_conflict_judge(
            "b1",
            1,
            "青云城到黑风谷有一条古道。",
            &pool,
            &extractor,
            None,
            None,
            None,
            None,
            Some(&map_judge),
        )
        .await
        .unwrap();

        let place_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM place_details WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let edge_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM place_edges WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(place_count.0, 2);
        assert_eq!(edge_count.0, 1);
    }

    #[tokio::test]
    async fn phase5_pipeline_failure_does_not_rollback_phase1_4() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let extractor = MockExtractor::new(vec![
            make_entity_obs("张三"),
            make_location_intro_obs("非法地点", "invalid_place_type"),
        ]);
        let map_judge = mock_map_accept_judge();

        let result = process_chapter_with_map_conflict_judge(
            "b1",
            1,
            "张三路过非法地点。",
            &pool,
            &extractor,
            None,
            None,
            None,
            None,
            Some(&map_judge),
        )
        .await;
        assert!(
            result.is_ok(),
            "map reducer failure should not fail chapter processing after Phase 1 writes"
        );

        let entity_repo = EntityRepo::new(pool.clone());
        assert!(
            entity_repo
                .get_by_canonical_name("b1", "张三")
                .await
                .unwrap()
                .is_some(),
            "Phase 1 entity write should remain committed"
        );
        let progress = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(progress.max_processed_chapter, 1);
    }

    #[tokio::test]
    async fn phase5_pipeline_rejects_knowledge_summary_as_edge() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let extractor = MockExtractor::new(vec![make_knowledge_obs(
            "geography",
            "青云城地理",
            "黑风谷位于青云城以北，是重要地理信息。",
        )]);
        let map_judge = mock_map_accept_judge();

        process_chapter_with_map_conflict_judge(
            "b1",
            1,
            "黑风谷位于青云城以北。",
            &pool,
            &extractor,
            None,
            None,
            None,
            None,
            Some(&map_judge),
        )
        .await
        .unwrap();

        let map_edge_claims: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM claims WHERE book_id = 'b1' AND claim_type = 'location_edge'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let active_edges: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM place_edges WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(map_edge_claims.0, 0);
        assert_eq!(active_edges.0, 0);
    }

    fn read_arcane_throne_admin_chapters(
        limit: usize,
    ) -> anyhow::Result<Vec<(i64, String, String)>> {
        let root = std::env::var("ARCANE_THRONE_LOCAL_TXT_DIR").unwrap_or_else(|_| {
            "/Users/maple/OrbStack/docker/containers/reader-next-local/app/storage/data/admin/local_books/e8560cb473947c4edbd18ffa15345713".to_string()
        });
        let root = std::path::PathBuf::from(root);
        let text = std::fs::read_to_string(root.join("book.txt"))?;
        let chapter_index: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("chapters.json"))?)?;
        let chapters = chapter_index
            .get("chapters")
            .and_then(|value| value.as_array())
            .ok_or_else(|| anyhow::anyhow!("chapters.json missing chapters array"))?;
        let chars: Vec<char> = text.chars().collect();
        let mut selected = Vec::new();
        for chapter in chapters {
            let title = chapter
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if !title.contains('章') {
                continue;
            }
            let Some(index) = chapter.get("index").and_then(|value| value.as_i64()) else {
                continue;
            };
            let Some(start) = chapter.get("start").and_then(|value| value.as_u64()) else {
                continue;
            };
            let Some(end) = chapter.get("end").and_then(|value| value.as_u64()) else {
                continue;
            };
            if end <= start {
                continue;
            }
            let start = start as usize;
            let end = (end as usize).min(chars.len());
            let raw_text: String = chars[start..end].iter().collect();
            selected.push((index, title.to_string(), raw_text));
            if selected.len() >= limit {
                break;
            }
        }
        Ok(selected)
    }

    // --- Existing Phase 1 tests (updated for new signature with judge parameter) ---

    #[tokio::test]
    async fn process_chapter_creates_source_spans_and_ai_runs() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三走进了大殿，看到了李四。两人互相行礼。";
        let extractor = empty_extractor();

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, None)
            .await
            .unwrap();

        // Verify chapter exists
        let chapter_repo = ChapterRepo::new(pool.clone());
        let chapter = chapter_repo.get_chapter("b1", 1).await.unwrap();
        assert!(chapter.is_some(), "chapter should exist");

        // Verify source spans were created
        let chapter = chapter.unwrap();
        let segments = chapter_repo
            .list_active_segments(&chapter.id)
            .await
            .unwrap();
        assert!(!segments.is_empty(), "should have segments");

        let claim_repo = ClaimRepo::new(pool.clone());
        for seg in &segments {
            let spans = claim_repo.list_spans_by_segment(&seg.id).await.unwrap();
            assert!(!spans.is_empty(), "segment should have spans");
        }

        // Verify ai_runs were created
        let ai_run_repo = AiRunRepo::new(pool.clone());
        let runs = ai_run_repo
            .list_runs_by_chapter("b1", &chapter.id)
            .await
            .unwrap();
        assert!(!runs.is_empty(), "should have ai_runs");
        assert_eq!(runs[0].status, "success", "ai_run should be success");
        // Verify segment_id is populated
        assert!(
            runs[0].segment_id.is_some(),
            "ai_run.segment_id should be populated"
        );
        // Verify segment_id points to a valid segment
        let seg_id = runs[0].segment_id.as_ref().unwrap();
        let seg = chapter_repo.get_segment(seg_id).await.unwrap();
        assert!(
            seg.is_some(),
            "ai_run.segment_id should point to a valid segment"
        );
    }

    #[tokio::test]
    async fn process_long_chapter_creates_unique_source_spans() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let paragraphs: Vec<String> = (0..140)
            .map(|i| format!("第{}段，张三在大殿里观察灵纹，李四记录阵法变化。", i))
            .collect();
        let raw_text = paragraphs.join("\n\n");
        assert!(raw_text.chars().count() > 3000);

        let extractor = empty_extractor();
        process_chapter("b1", 1, &raw_text, &pool, &extractor, None, None)
            .await
            .unwrap();

        let chapter_repo = ChapterRepo::new(pool.clone());
        let chapter = chapter_repo
            .get_chapter("b1", 1)
            .await
            .unwrap()
            .expect("chapter should exist");
        let segments = chapter_repo
            .list_active_segments(&chapter.id)
            .await
            .unwrap();
        assert!(segments.len() > 1, "long chapter should split into segments");

        let claim_repo = ClaimRepo::new(pool.clone());
        let mut all_span_indexes = Vec::new();
        for segment in &segments {
            let spans = claim_repo.list_spans_by_segment(&segment.id).await.unwrap();
            assert!(!spans.is_empty(), "segment should have spans");
            all_span_indexes.extend(spans.into_iter().map(|span| span.span_index));
        }
        all_span_indexes.sort_unstable();
        all_span_indexes.dedup();
        assert_eq!(all_span_indexes.len(), paragraphs.len());
    }

    #[tokio::test]
    async fn process_chapter_idempotent_skip() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "第一章内容。";
        let extractor = empty_extractor();

        // First run
        process_chapter("b1", 1, raw_text, &pool, &extractor, None, None)
            .await
            .unwrap();

        // Get chapter to count runs
        let chapter_repo = ChapterRepo::new(pool.clone());
        let chapter = chapter_repo.get_chapter("b1", 1).await.unwrap().unwrap();
        let ai_run_repo = AiRunRepo::new(pool.clone());
        let runs_before = ai_run_repo
            .list_runs_by_chapter("b1", &chapter.id)
            .await
            .unwrap();

        // Second run (same text -> same hash -> should skip)
        process_chapter("b1", 1, raw_text, &pool, &extractor, None, None)
            .await
            .unwrap();
        let runs_after = ai_run_repo
            .list_runs_by_chapter("b1", &chapter.id)
            .await
            .unwrap();

        assert_eq!(
            runs_before.len(),
            runs_after.len(),
            "second run should be idempotent"
        );
    }

    #[tokio::test]
    async fn process_chapter_updates_progress() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();
        let extractor = empty_extractor();

        process_chapter("b1", 1, "第一章内容。", &pool, &extractor, None, None)
            .await
            .unwrap();

        let progress = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(progress.max_processed_chapter, 1);

        process_chapter("b1", 2, "第二章内容。", &pool, &extractor, None, None)
            .await
            .unwrap();

        let progress = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(progress.max_processed_chapter, 2);
    }

    #[tokio::test]
    async fn process_chapter_creates_processing_run() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();
        let extractor = empty_extractor();

        process_chapter("b1", 1, "测试内容。", &pool, &extractor, None, None)
            .await
            .unwrap();

        // Verify chapter_processing_run was created and marked success
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM chapter_processing_runs WHERE book_id = 'b1' AND status = 'success'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count.0, 1, "should have one successful processing run");
    }

    #[tokio::test]
    async fn process_chapter_empty_text_succeeds() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();
        let extractor = empty_extractor();

        // Empty text should still succeed (produces 1 segment with empty span)
        let result = process_chapter("b1", 1, "", &pool, &extractor, None, None).await;
        assert!(result.is_ok(), "empty text should succeed");

        let progress = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(progress.max_processed_chapter, 1);
    }

    #[tokio::test]
    async fn phase3_identity_claim_re_resolves_after_phase1_and_records_judge_output() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "黑衣人摘下面具，竟是张三。";
        let extractor = MockExtractor::new(vec![
            make_entity_obs("黑衣人"),
            make_entity_obs("张三"),
            make_identity_reveal_obs("黑衣人", "张三"),
        ]);
        let identity_judge = crate::service::v4::identity_judge::MockIdentityJudge::new(
            crate::service::v4::identity_judge::IdentityJudgeOutput {
                decision: crate::service::v4::identity_judge::IdentityJudgeDecision::Merge,
                link_type: "same_identity".to_string(),
                survivor_hint: "entity_b".to_string(),
                confidence: 0.96,
                reason_code: "explicit_reveal".to_string(),
                explanation_for_log: "explicit reveal".to_string(),
                property_conflicts: vec![],
                relationship_migration_hint: "safe".to_string(),
            },
        );

        process_chapter_with_identity_judge(
            "b1",
            1,
            raw_text,
            &pool,
            &extractor,
            None,
            None,
            Some(&identity_judge),
        )
        .await
        .unwrap();

        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let identity_claim = claims
            .iter()
            .find(|c| c.claim_type == "identity_reveal")
            .expect("identity claim");
        assert!(identity_claim.subject_entity_id.is_some());
        assert!(identity_claim.object_entity_id.is_some());
        assert_eq!(identity_claim.status, "accepted");
        let victim_id = identity_claim
            .subject_entity_id
            .as_deref()
            .expect("victim id");
        let survivor_id = identity_claim
            .object_entity_id
            .as_deref()
            .expect("survivor id");

        let victim_status: (String,) = sqlx::query_as("SELECT status FROM entities WHERE id = ?")
            .bind(victim_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let survivor_status: (String,) = sqlx::query_as("SELECT status FROM entities WHERE id = ?")
            .bind(survivor_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(victim_status.0, "merged");
        assert_eq!(survivor_status.0, "active");

        let redirect_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_identity_links WHERE book_id = 'b1' AND entity_a_id = ? AND entity_b_id = ? AND link_type = 'redirect' AND status = 'active'",
        )
        .bind(victim_id)
        .bind(survivor_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(redirect_count.0, 1);

        let payload: serde_json::Value =
            serde_json::from_str(identity_claim.value_json.as_deref().expect("value_json"))
                .unwrap();
        assert_eq!(payload["judge_decision"], "merge");
        assert_eq!(payload["judge_confidence"], 0.96);
        assert_eq!(payload["reason_code"], "explicit_reveal");
    }

    #[tokio::test]
    async fn phase4_pipeline_reduces_knowledge_assertion_into_card() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let extractor = MockExtractor::new(vec![make_knowledge_obs(
            "power_system",
            "Cultivation Realms",
            "Cultivation has stable realm tiers.",
        )]);
        let knowledge_judge = crate::service::v4::knowledge_judge::MockKnowledgeRevisionJudge::new(
            crate::service::v4::knowledge_judge::KnowledgeJudgeOutput {
                decision: crate::service::v4::knowledge_judge::KnowledgeJudgeDecision::AddNew,
                card_action:
                    crate::service::v4::knowledge_judge::KnowledgeCardAction::CreateNewCard,
                target_card_id: None,
                affected_assertion_ids: vec![],
                assertion_status: "active".to_string(),
                current_summary: Some("Cultivation has stable realm tiers.".to_string()),
                confidence: 0.91,
                reason_code: "test_add_new".to_string(),
                explanation_for_log: "test add new knowledge".to_string(),
            },
        );

        process_chapter_with_knowledge_judge(
            "b1",
            1,
            "修炼境界有稳定层级。",
            &pool,
            &extractor,
            None,
            None,
            None,
            Some(&knowledge_judge),
        )
        .await
        .unwrap();

        let knowledge_repo = crate::storage::db::v4::knowledge_repo::KnowledgeRepo::new(pool.clone());
        let cards = knowledge_repo
            .list_cards("b1", Some("power_system"), Some("active"))
            .await
            .unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].topic_key, "cultivation-realms");
        assert_eq!(
            cards[0].current_summary.as_deref(),
            Some("Cultivation has stable realm tiers.")
        );
        let claim_repo = ClaimRepo::new(pool);
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let knowledge_claim = claims
            .into_iter()
            .find(|claim| claim.claim_type == "knowledge_assertion")
            .expect("knowledge claim");
        assert_eq!(knowledge_claim.status, "accepted");
    }

    #[tokio::test]
    async fn phase4_uncertain_knowledge_judge_with_unsupported_status_skips_reducer() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let extractor = MockExtractor::new(vec![make_knowledge_obs(
            "power_system",
            "Cultivation Realms",
            "Cultivation has stable realm tiers.",
        )]);
        let knowledge_judge = crate::service::v4::knowledge_judge::MockKnowledgeRevisionJudge::new(
            crate::service::v4::knowledge_judge::KnowledgeJudgeOutput {
                decision: crate::service::v4::knowledge_judge::KnowledgeJudgeDecision::AddNew,
                card_action: crate::service::v4::knowledge_judge::KnowledgeCardAction::Uncertain,
                target_card_id: None,
                affected_assertion_ids: vec![],
                assertion_status: "unsupported".to_string(),
                current_summary: None,
                confidence: 0.35,
                reason_code: "test_uncertain".to_string(),
                explanation_for_log: "test uncertain unsupported".to_string(),
            },
        );

        process_chapter_with_knowledge_judge(
            "b1",
            1,
            "修炼境界是否稳定还不确定。",
            &pool,
            &extractor,
            None,
            None,
            None,
            Some(&knowledge_judge),
        )
        .await
        .unwrap();

        let knowledge_repo = crate::storage::db::v4::knowledge_repo::KnowledgeRepo::new(pool.clone());
        let cards = knowledge_repo
            .list_cards("b1", Some("power_system"), Some("active"))
            .await
            .unwrap();
        assert!(cards.is_empty());

        let claim_repo = ClaimRepo::new(pool);
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let knowledge_claim = claims
            .into_iter()
            .find(|claim| claim.claim_type == "knowledge_assertion")
            .expect("knowledge claim");
        assert_eq!(knowledge_claim.status, "uncertain");
    }

    #[tokio::test]
    async fn phase4_write_path_with_unsupported_status_creates_no_canonical_write() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let extractor = MockExtractor::new(vec![make_knowledge_obs(
            "power_system",
            "Cultivation Realms",
            "Cultivation has stable realm tiers.",
        )]);
        let knowledge_judge = crate::service::v4::knowledge_judge::MockKnowledgeRevisionJudge::new(
            crate::service::v4::knowledge_judge::KnowledgeJudgeOutput {
                decision: crate::service::v4::knowledge_judge::KnowledgeJudgeDecision::AddNew,
                card_action:
                    crate::service::v4::knowledge_judge::KnowledgeCardAction::CreateNewCard,
                target_card_id: None,
                affected_assertion_ids: vec![],
                assertion_status: "unsupported".to_string(),
                current_summary: Some("bad status".to_string()),
                confidence: 0.91,
                reason_code: "test_bad_status".to_string(),
                explanation_for_log: "write path invalid status".to_string(),
            },
        );

        process_chapter_with_knowledge_judge(
            "b1",
            1,
            "修炼境界有稳定层级。",
            &pool,
            &extractor,
            None,
            None,
            None,
            Some(&knowledge_judge),
        )
        .await
        .unwrap();

        let unsupported_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_assertions WHERE status = 'unsupported'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(unsupported_count, 0);

        let knowledge_repo = crate::storage::db::v4::knowledge_repo::KnowledgeRepo::new(pool.clone());
        let cards = knowledge_repo
            .list_cards("b1", Some("power_system"), Some("active"))
            .await
            .unwrap();
        assert!(cards.is_empty());

        let claim_repo = ClaimRepo::new(pool);
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let knowledge_claim = claims
            .into_iter()
            .find(|claim| claim.claim_type == "knowledge_assertion")
            .expect("knowledge claim");
        assert_eq!(knowledge_claim.status, "rejected");
    }

    #[tokio::test]
    async fn phase4_knowledge_reducer_failure_does_not_fail_chapter_or_rollback_entities() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let extractor = MockExtractor::new(vec![
            make_entity_obs("张三"),
            make_knowledge_obs(
                "power_system",
                "Cultivation Realms",
                "Cultivation has stable realm tiers.",
            ),
        ]);
        let knowledge_judge = crate::service::v4::knowledge_judge::MockKnowledgeRevisionJudge::new(
            crate::service::v4::knowledge_judge::KnowledgeJudgeOutput {
                decision: crate::service::v4::knowledge_judge::KnowledgeJudgeDecision::Supplement,
                card_action:
                    crate::service::v4::knowledge_judge::KnowledgeCardAction::UseExistingCard,
                target_card_id: Some("missing-card".to_string()),
                affected_assertion_ids: vec![],
                assertion_status: "active".to_string(),
                current_summary: Some("Cultivation has stable realm tiers.".to_string()),
                confidence: 0.91,
                reason_code: "test_bad_card".to_string(),
                explanation_for_log: "test missing card".to_string(),
            },
        );

        let result = process_chapter_with_knowledge_judge(
            "b1",
            1,
            "张三听说修炼境界有稳定层级。",
            &pool,
            &extractor,
            None,
            None,
            None,
            Some(&knowledge_judge),
        )
        .await;
        assert!(
            result.is_ok(),
            "knowledge reducer failure should not fail completed chapter processing"
        );

        let entity_repo = EntityRepo::new(pool.clone());
        assert!(
            entity_repo
                .get_by_canonical_name("b1", "张三")
                .await
                .unwrap()
                .is_some(),
            "Phase 1 entity write should remain committed"
        );
        let progress = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(progress.max_processed_chapter, 1);
    }

    #[tokio::test]
    async fn phase4_pipeline_resolves_same_chapter_entity_references_for_knowledge() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let extractor = MockExtractor::new(vec![
            make_entity_obs("张三"),
            make_knowledge_obs_with_entity_ref(
                "history",
                "Sect Founding",
                "张三 founded the ancient sect.",
                "张三",
                "subject",
            ),
        ]);
        let knowledge_judge = crate::service::v4::knowledge_judge::MockKnowledgeRevisionJudge::new(
            crate::service::v4::knowledge_judge::KnowledgeJudgeOutput {
                decision: crate::service::v4::knowledge_judge::KnowledgeJudgeDecision::AddNew,
                card_action:
                    crate::service::v4::knowledge_judge::KnowledgeCardAction::CreateNewCard,
                target_card_id: None,
                affected_assertion_ids: vec![],
                assertion_status: "active".to_string(),
                current_summary: Some("张三 founded the ancient sect.".to_string()),
                confidence: 0.91,
                reason_code: "test_entity_ref".to_string(),
                explanation_for_log: "test same chapter entity reference".to_string(),
            },
        );

        process_chapter_with_knowledge_judge(
            "b1",
            1,
            "张三创立了古老宗门。",
            &pool,
            &extractor,
            None,
            None,
            None,
            Some(&knowledge_judge),
        )
        .await
        .unwrap();

        let ref_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*)
             FROM knowledge_assertion_entities kae
             JOIN entities e ON e.id = kae.entity_id
             WHERE kae.book_id = 'b1' AND kae.role = 'subject' AND e.canonical_name = '张三'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(ref_count.0, 1);
    }

    #[tokio::test]
    async fn phase3_blocked_identity_pair_is_rejected_before_merge() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();
        let entity_repo = EntityRepo::new(pool.clone());
        let identity_repo = crate::storage::db::v4::identity_repo::IdentityRepo::new(pool.clone());

        let left = entity_repo
            .create_entity("b1", "character", "此张三", "此张三", None, 0.8, 1)
            .await
            .unwrap();
        let right = entity_repo
            .create_entity("b1", "character", "彼张三", "彼张三", None, 0.8, 1)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('block-ch', 'b1', 0, '此张三并非彼张三。', 'block-hash', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('block-seg', 'b1', 'block-ch', 'block-hash', 0, datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('block-span', 'b1', 'block-ch', 'block-hash', 'block-seg', 0, 0, 10, '此张三并非彼张三。', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('block-run', 'b1', 'block-ch', 'identity_judge', 'test', 'v1', 1, 'block-input', 'success', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('claim-block', 'b1', 0, 'not_same_identity', 'not same identity', 'block-span', 'block-run', 0.99, 'high', 'accepted', datetime('now'), datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        identity_repo
            .create_identity_link(
                "b1",
                &left.id,
                &right.id,
                "not_same_identity",
                0.99,
                "claim-block",
                "active",
            )
            .await
            .unwrap();

        let extractor = MockExtractor::new(vec![
            crate::service::v4::extractor::Observation::EntityMergeCandidate {
                entity_a_mention: "此张三".to_string(),
                entity_b_mention: "彼张三".to_string(),
                reason_hint: Some("看起来像同一人".to_string()),
                evidence_span_ids: vec![],
                confidence: 0.91,
            },
        ]);
        let identity_judge = crate::service::v4::identity_judge::MockIdentityJudge::new(
            crate::service::v4::identity_judge::IdentityJudgeOutput {
                decision: crate::service::v4::identity_judge::IdentityJudgeDecision::Merge,
                link_type: "same_identity".to_string(),
                survivor_hint: "entity_a".to_string(),
                confidence: 0.99,
                reason_code: "should_not_run".to_string(),
                explanation_for_log: "blocked pair should stop before merge".to_string(),
                property_conflicts: vec![],
                relationship_migration_hint: "safe".to_string(),
            },
        );

        process_chapter_with_identity_judge(
            "b1",
            1,
            "此张三并非彼张三。",
            &pool,
            &extractor,
            None,
            None,
            Some(&identity_judge),
        )
        .await
        .unwrap();

        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let merge_claim = claims
            .iter()
            .find(|c| c.claim_type == "entity_merge_candidate")
            .expect("merge candidate claim");
        assert_eq!(merge_claim.status, "rejected");
    }

    #[tokio::test]
    async fn phase3_invalid_identity_judge_output_is_rejected() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "黑衣人摘下面具，竟是张三。";
        let extractor = MockExtractor::new(vec![
            make_entity_obs("黑衣人"),
            make_entity_obs("张三"),
            make_identity_reveal_obs("黑衣人", "张三"),
        ]);
        let identity_judge = crate::service::v4::identity_judge::MockIdentityJudge::new(
            crate::service::v4::identity_judge::IdentityJudgeOutput {
                decision: crate::service::v4::identity_judge::IdentityJudgeDecision::Merge,
                link_type: "not_same_identity".to_string(),
                survivor_hint: "entity_b".to_string(),
                confidence: 0.96,
                reason_code: "invalid_contract".to_string(),
                explanation_for_log: "invalid merge/link_type combination".to_string(),
                property_conflicts: vec![],
                relationship_migration_hint: "safe".to_string(),
            },
        );

        process_chapter_with_identity_judge(
            "b1",
            1,
            raw_text,
            &pool,
            &extractor,
            None,
            None,
            Some(&identity_judge),
        )
        .await
        .unwrap();

        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let identity_claim = claims
            .iter()
            .find(|c| c.claim_type == "identity_reveal")
            .expect("identity claim");
        assert_eq!(identity_claim.status, "rejected");
    }

    #[tokio::test]
    async fn e2e_mock_extractor_full_pipeline() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三走进了大殿。张真人乃是主角，如今已突破到金丹境界。";

        let extractor = MockExtractor::new(vec![
            // 1. EntityIntroduction: "张三"
            crate::service::v4::extractor::Observation::EntityIntroduction {
                subject_mention: "张三".to_string(),
                entity_type: "character".to_string(),
                aliases: vec!["张真人".to_string()],
                short_summary: "主角".to_string(),
                evidence_span_ids: vec![],
                confidence: 0.95,
            },
            // 2. Alias: "小张"
            crate::service::v4::extractor::Observation::Alias {
                subject_mention: "张三".to_string(),
                alias: "小张".to_string(),
                alias_type: "nickname".to_string(),
                evidence_span_ids: vec![],
                confidence: 0.85,
            },
            // 3. PropertyUpdate: realm = 金丹
            crate::service::v4::extractor::Observation::PropertyUpdate {
                subject_mention: "张三".to_string(),
                dimension_key: "realm".to_string(),
                value_text: Some("金丹".to_string()),
                value_json: None,
                evidence_span_ids: vec![],
                confidence: 0.9,
            },
            // 4. Summary
            crate::service::v4::extractor::Observation::Summary {
                summary: "本章介绍张三突破金丹".to_string(),
                key_points: vec!["张三".to_string(), "金丹".to_string()],
                has_important_changes: true,
            },
        ]);

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, None)
            .await
            .unwrap();

        // --- Verify entities ---
        let entity_repo = EntityRepo::new(pool.clone());
        let entity = entity_repo
            .find_entity_by_alias("b1", "张三")
            .await
            .unwrap();
        assert!(entity.is_some(), "entity '张三' should exist");
        let entity = entity.unwrap();
        assert_eq!(entity.canonical_name, "张三");
        assert_eq!(entity.short_summary.as_deref(), Some("主角"));

        // --- Verify entity_aliases ---
        let aliases = entity_repo
            .list_aliases_by_entity(&entity.id)
            .await
            .unwrap();
        let alias_texts: Vec<&str> = aliases.iter().map(|a| a.alias.as_str()).collect();
        assert!(
            alias_texts.contains(&"小张"),
            "alias '小张' should exist, got {:?}",
            alias_texts
        );
        assert!(
            alias_texts.contains(&"张真人"),
            "alias '张真人' (from EntityIntroduction) should exist, got {:?}",
            alias_texts
        );

        // --- Verify entity_properties and entity_current_properties ---
        let property_repo = crate::storage::db::v4::property_repo::PropertyRepo::new(pool.clone());
        let current = property_repo
            .get_current_property("b1", &entity.id, "realm")
            .await
            .unwrap();
        assert!(current.is_some(), "current property 'realm' should exist");
        assert_eq!(
            current.unwrap().value_text.as_deref(),
            Some("金丹"),
            "current realm should be '金丹'"
        );

        // --- Verify chapter_summaries ---
        let chapter_repo = ChapterRepo::new(pool.clone());
        let summary = chapter_repo.get_chapter_summary("b1", 1).await.unwrap();
        assert!(summary.is_some(), "chapter summary should exist");
        assert_eq!(summary.unwrap().summary, "本章介绍张三突破金丹");

        // --- Verify claims ---
        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let claim_types: Vec<&str> = claims.iter().map(|c| c.claim_type.as_str()).collect();
        assert!(
            claim_types.contains(&"entity_introduction"),
            "should have entity_introduction claim, got {:?}",
            claim_types
        );
        assert!(
            claim_types.contains(&"alias"),
            "should have alias claim, got {:?}",
            claim_types
        );
        assert!(
            claim_types.contains(&"property_update"),
            "should have property_update claim, got {:?}",
            claim_types
        );
        // Summary should NOT create a claim
        assert!(
            !claim_types.contains(&"summary"),
            "Summary should not create a claim, got {:?}",
            claim_types
        );

        // Verify accepted claims (low/medium risk ones)
        let accepted: Vec<&ClaimRecord> =
            claims.iter().filter(|c| c.status == "accepted").collect();
        assert_eq!(
            accepted.len(),
            3,
            "entity_introduction, alias, property_update should all be accepted"
        );

        // --- Verify view_model_cache ---
        let cache_repo = crate::storage::db::v4::cache_repo::CacheRepo::new(pool.clone());
        let character_list_cache = cache_repo
            .get_cached("b1", "character_list", "__book__", 1)
            .await
            .unwrap();
        assert!(
            character_list_cache.is_some(),
            "character_list cache should be populated"
        );
        let list: Vec<crate::service::v4::projection::CharacterListItem> =
            serde_json::from_str(&character_list_cache.unwrap()).unwrap();
        assert_eq!(list.len(), 1, "should have 1 character in list");
        assert_eq!(list[0].name, "张三");
        assert_eq!(list[0].summary.as_deref(), Some("主角"));

        // character_card is populated on-demand by API, not by pipeline.
        // Verify it can be projected correctly:
        let card =
            crate::service::v4::projection::project_character_card(&entity.id, "b1", 1, &pool)
                .await
                .unwrap();
        assert_eq!(card.name, "张三");
        assert!(card.current_states.contains_key("realm"));
        assert_eq!(card.current_states["realm"].value, "金丹");
    }

    #[tokio::test]
    async fn e2e_high_risk_property_quarantined() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三登场。张三被击杀，当场死亡。";

        let extractor = MockExtractor::new(vec![
            // 1. Low-risk: entity introduction
            crate::service::v4::extractor::Observation::EntityIntroduction {
                subject_mention: "张三".to_string(),
                entity_type: "character".to_string(),
                aliases: vec![],
                short_summary: "悲剧角色".to_string(),
                evidence_span_ids: vec![],
                confidence: 0.9,
            },
            // 2. High-risk: death
            crate::service::v4::extractor::Observation::PropertyUpdate {
                subject_mention: "张三".to_string(),
                dimension_key: "life_status".to_string(),
                value_text: Some("死亡".to_string()),
                value_json: None,
                evidence_span_ids: vec![],
                confidence: 0.95,
            },
        ]);

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, None)
            .await
            .unwrap();

        // --- Verify entity was created (from low-risk entity_introduction) ---
        let entity_repo = EntityRepo::new(pool.clone());
        let entity = entity_repo
            .find_entity_by_alias("b1", "张三")
            .await
            .unwrap();
        assert!(
            entity.is_some(),
            "entity should be created from low-risk introduction"
        );
        let entity = entity.unwrap();

        // --- Verify claims ---
        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();

        // entity_introduction should be accepted (low risk)
        let intro_claim = claims
            .iter()
            .find(|c| c.claim_type == "entity_introduction")
            .unwrap();
        assert_eq!(intro_claim.status, "accepted");

        // property_update (death) should be quarantined (high risk)
        let death_claim = claims
            .iter()
            .find(|c| c.claim_type == "property_update")
            .unwrap();
        assert_eq!(
            death_claim.status, "quarantined",
            "death claim should be quarantined, got {}",
            death_claim.status
        );

        // --- Verify no canonical state written for life_status ---
        let property_repo = crate::storage::db::v4::property_repo::PropertyRepo::new(pool.clone());
        let current = property_repo
            .get_current_property("b1", &entity.id, "life_status")
            .await
            .unwrap();
        assert!(
            current.is_none(),
            "quarantined claim should NOT write to canonical state (entity_current_properties)"
        );
    }

    // --- Phase 2 pipeline integration tests ---

    #[tokio::test]
    async fn phase2_relationship_gate_pass_judge_accept_creates_relationship() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三和李四是好朋友。";
        let extractor = MockExtractor::new(vec![
            make_entity_obs("张三"),
            make_entity_obs("李四"),
            make_rel_obs("张三", "李四", "friendship", "朋友", 0.9),
        ]);
        let judge = mock_accept_judge();

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, Some(&judge))
            .await
            .unwrap();

        // Verify entities were created
        let entity_repo = EntityRepo::new(pool.clone());
        let e1 = entity_repo
            .find_entity_by_alias("b1", "张三")
            .await
            .unwrap();
        let e2 = entity_repo
            .find_entity_by_alias("b1", "李四")
            .await
            .unwrap();
        assert!(e1.is_some(), "张三 should exist");
        assert!(e2.is_some(), "李四 should exist");

        // Verify relationship was created by the relationship_reducer
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rels = rel_repo.list_by_book("b1", None, None).await.unwrap();
        assert_eq!(rels.len(), 1, "should have 1 relationship");
        assert_eq!(rels[0].relation_group, "friendship");
        assert_eq!(rels[0].status, "active");

        // Verify claim status
        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let rel_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.claim_type == "relationship_update")
            .collect();
        assert_eq!(rel_claims.len(), 1);
        assert_eq!(rel_claims[0].status, "accepted");
    }

    #[tokio::test]
    async fn phase2_relationship_gate_reject_no_relationship() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三和李四偶遇。";
        // Use an INVALID relation_group to trigger structural gate rejection
        let extractor = MockExtractor::new(vec![
            make_entity_obs("张三"),
            make_entity_obs("李四"),
            crate::service::v4::extractor::Observation::RelationshipUpdate {
                subject_mention: "张三".to_string(),
                object_mention: "李四".to_string(),
                relation_hint: "偶遇".to_string(),
                relation_group: "invalid_xyz".to_string(),
                relation_label: "偶遇".to_string(),
                directionality: "undirected".to_string(),
                evidence_span_ids: vec![],
                confidence: 0.8,
                importance_hint: 0.3,
                is_long_term_or_significant_hint: true,
            },
        ]);
        // Judge should NOT be called (gate rejects before judge)
        let judge = mock_accept_judge();

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, Some(&judge))
            .await
            .unwrap();

        // No relationships should exist (gate rejected)
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rels = rel_repo.list_by_book("b1", None, None).await.unwrap();
        assert_eq!(
            rels.len(),
            0,
            "no relationship should be created on gate reject"
        );

        // Relationship claim should be rejected
        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let rel_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.claim_type == "relationship_update")
            .collect();
        assert_eq!(rel_claims.len(), 1);
        assert_eq!(rel_claims[0].status, "rejected");
    }

    #[tokio::test]
    async fn phase2_gate_redirect_creates_property_claim_and_reduces() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三进入了青云门。";
        let extractor = MockExtractor::new(vec![
            make_entity_obs("张三"),
            // EntityIntroduction for a PLACE entity (non-character)
            crate::service::v4::extractor::Observation::EntityIntroduction {
                subject_mention: "青云门".to_string(),
                entity_type: "place".to_string(),
                aliases: vec![],
                short_summary: "修炼门派".to_string(),
                evidence_span_ids: vec![],
                confidence: 0.85,
            },
            // Relationship: 张三 <-> 青云门 (will be redirected because object is a place)
            make_rel_obs("张三", "青云门", "alliance", "归属", 0.8),
        ]);
        let judge = mock_accept_judge();

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, Some(&judge))
            .await
            .unwrap();

        // No canonical relationship should exist (object is a place, redirected)
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rels = rel_repo.list_by_book("b1", None, None).await.unwrap();
        assert_eq!(
            rels.len(),
            0,
            "no relationship should be created (redirect)"
        );

        // Verify the original relationship claim was redirected
        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let rel_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.claim_type == "relationship_update")
            .collect();
        assert_eq!(rel_claims.len(), 1);
        assert_eq!(rel_claims[0].status, "redirected");

        // A derived property_update claim should have been created
        let property_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.claim_type == "property_update")
            .collect();
        assert!(
            !property_claims.is_empty(),
            "should have at least one property_update claim (redirected from relationship)"
        );
    }

    #[tokio::test]
    async fn phase2_same_segment_new_character_and_relationship() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "新角色张三和李四登场，两人是师徒。";
        // Both characters are brand new (entity_introduction + relationship_update in same segment)
        let extractor = MockExtractor::new(vec![
            make_entity_obs("张三"),
            make_entity_obs("李四"),
            make_rel_obs("张三", "李四", "mentorship", "师徒", 0.9),
        ]);
        let judge = mock_accept_judge();

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, Some(&judge))
            .await
            .unwrap();

        // Both entities should exist
        let entity_repo = EntityRepo::new(pool.clone());
        let e1 = entity_repo
            .find_entity_by_alias("b1", "张三")
            .await
            .unwrap();
        let e2 = entity_repo
            .find_entity_by_alias("b1", "李四")
            .await
            .unwrap();
        assert!(e1.is_some(), "张三 should exist (created in Phase 1)");
        assert!(e2.is_some(), "李四 should exist (created in Phase 1)");

        // Relationship should exist (two-phase resolution: Phase 1 creates entities,
        // then relationship claims are re-resolved against new entities)
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rels = rel_repo.list_by_book("b1", None, None).await.unwrap();
        assert_eq!(
            rels.len(),
            1,
            "should have 1 relationship via two-phase resolution"
        );
        // Mock judge returns "friendship" for all accepted claims
        assert_eq!(rels[0].relation_group, "friendship");
        assert_eq!(rels[0].status, "active");
    }

    #[tokio::test]
    async fn phase1_e2e_still_passes_with_new_signature() {
        // Regression test: Phase 1 full pipeline should still work with updated signature
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三走进了大殿。张真人乃是主角，如今已突破到金丹境界。";

        let extractor = MockExtractor::new(vec![
            crate::service::v4::extractor::Observation::EntityIntroduction {
                subject_mention: "张三".to_string(),
                entity_type: "character".to_string(),
                aliases: vec!["张真人".to_string()],
                short_summary: "主角".to_string(),
                evidence_span_ids: vec![],
                confidence: 0.95,
            },
            crate::service::v4::extractor::Observation::Alias {
                subject_mention: "张三".to_string(),
                alias: "小张".to_string(),
                alias_type: "nickname".to_string(),
                evidence_span_ids: vec![],
                confidence: 0.85,
            },
            crate::service::v4::extractor::Observation::PropertyUpdate {
                subject_mention: "张三".to_string(),
                dimension_key: "realm".to_string(),
                value_text: Some("金丹".to_string()),
                value_json: None,
                evidence_span_ids: vec![],
                confidence: 0.9,
            },
            crate::service::v4::extractor::Observation::Summary {
                summary: "本章介绍张三突破金丹".to_string(),
                key_points: vec!["张三".to_string(), "金丹".to_string()],
                has_important_changes: true,
            },
        ]);

        // Pass None as judge (Phase 1 behavior: no relationship judge processing)
        process_chapter("b1", 1, raw_text, &pool, &extractor, None, None)
            .await
            .unwrap();

        // Same verifications as Phase 1 E2E
        let entity_repo = EntityRepo::new(pool.clone());
        let entity = entity_repo
            .find_entity_by_alias("b1", "张三")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(entity.canonical_name, "张三");
        assert_eq!(entity.short_summary.as_deref(), Some("主角"));

        let property_repo = crate::storage::db::v4::property_repo::PropertyRepo::new(pool.clone());
        let current = property_repo
            .get_current_property("b1", &entity.id, "realm")
            .await
            .unwrap();
        assert!(current.is_some());
        assert_eq!(current.unwrap().value_text.as_deref(), Some("金丹"));

        let chapter_repo = ChapterRepo::new(pool.clone());
        let summary = chapter_repo.get_chapter_summary("b1", 1).await.unwrap();
        assert!(summary.is_some());
        assert_eq!(summary.unwrap().summary, "本章介绍张三突破金丹");

        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo.list_claims_by_chapter("b1", 1).await.unwrap();
        let accepted: Vec<_> = claims.iter().filter(|c| c.status == "accepted").collect();
        assert_eq!(
            accepted.len(),
            3,
            "Phase 1: entity_introduction, alias, property_update accepted"
        );
    }

    // --- Phase 2 integration tests: relationship_count / relationships_in_chapter ---

    #[tokio::test]
    async fn phase2_memory_overview_relationship_count() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三和李四登场，两人是师徒关系。";
        let extractor = MockExtractor::new(vec![
            make_entity_obs("张三"),
            make_entity_obs("李四"),
            make_rel_obs("张三", "李四", "mentorship", "师徒", 0.9),
        ]);
        let judge = mock_accept_judge();

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, Some(&judge))
            .await
            .unwrap();

        // Verify relationship count via repo (same as MemoryOverviewView uses)
        let rel_repo = RelationshipRepo::new(pool.clone());
        let count = rel_repo.count_active_by_book("b1").await.unwrap();
        assert_eq!(count, 1, "should have 1 active relationship");

        // Verify inactive relationships are not counted
        let rels = rel_repo.list_by_book("b1", None, None).await.unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].status, "active");
    }

    #[tokio::test]
    async fn phase2_chapter_memory_relationships_in_chapter() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三和李四登场，两人是朋友关系。";
        let extractor = MockExtractor::new(vec![
            make_entity_obs("张三"),
            make_entity_obs("李四"),
            make_rel_obs("张三", "李四", "friendship", "朋友", 0.9),
        ]);
        let judge = mock_accept_judge(); // mock returns normalized_group = "friendship"

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, Some(&judge))
            .await
            .unwrap();

        // Verify relationships_in_chapter via repo (same as ChapterMemoryView uses)
        let rel_repo = RelationshipRepo::new(pool.clone());
        let chapter_rels = rel_repo
            .list_relationships_by_chapter("b1", 1)
            .await
            .unwrap();
        assert_eq!(
            chapter_rels.len(),
            1,
            "should have 1 relationship event in chapter 1"
        );

        let (event, relationship) = &chapter_rels[0];
        // Mock judge normalizes to "friendship"
        assert_eq!(event.relation_group, "friendship");
        assert_eq!(relationship.relation_group, "friendship");
        assert!(!relationship.subject_character_id.is_empty());
        assert!(!relationship.object_character_id.is_empty());

        // Chapter 2 should have no relationship events
        let ch2_rels = rel_repo
            .list_relationships_by_chapter("b1", 2)
            .await
            .unwrap();
        assert_eq!(
            ch2_rels.len(),
            0,
            "chapter 2 should have no relationship events"
        );
    }

    #[tokio::test]
    async fn phase2_multiple_relationships_different_groups() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三、李四、王五登场。张三是李四的师父，张三和王五是朋友。";
        let extractor = MockExtractor::new(vec![
            make_entity_obs("张三"),
            make_entity_obs("李四"),
            make_entity_obs("王五"),
            make_rel_obs("张三", "李四", "mentorship", "师徒", 0.9),
            make_rel_obs("张三", "王五", "friendship", "朋友", 0.8),
        ]);
        let judge = mock_accept_judge();

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, Some(&judge))
            .await
            .unwrap();

        let rel_repo = RelationshipRepo::new(pool.clone());
        let count = rel_repo.count_active_by_book("b1").await.unwrap();
        assert_eq!(count, 2, "should have 2 active relationships");

        let chapter_rels = rel_repo
            .list_relationships_by_chapter("b1", 1)
            .await
            .unwrap();
        assert_eq!(
            chapter_rels.len(),
            2,
            "should have 2 relationship events in chapter 1"
        );
    }

    // --- Real AI Smoke Test (gated by RUN_REAL_AI_TESTS=1) ---

    /// Helper: create AiModelService with config from env vars for smoke test.
    async fn create_smoke_test_ai_service() -> anyhow::Result<(
        std::sync::Arc<crate::service::ai_model_service::AiModelService>,
        std::path::PathBuf,
    )> {
        use crate::model::ai_model::{AiModelConfig, AiModelEndpointConfig};
        use crate::service::ai_model_service::AiModelService;
        use crate::service::json_document_service::JsonDocumentService;

        let base_url =
            std::env::var("AI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com".to_string());
        let api_key =
            std::env::var("AI_API_KEY").expect("AI_API_KEY must be set for real AI smoke test");
        let model = std::env::var("AI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

        let dir = std::env::temp_dir().join(format!("v4-smoke-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = crate::storage::db::init_pool(&database_url).await?;
        let docs = std::sync::Arc::new(JsonDocumentService::new(pool, dir.to_str().unwrap()));
        let service = AiModelService::new(docs, dir.to_str().unwrap());

        let mut config = AiModelConfig::default();
        config.text = AiModelEndpointConfig {
            enabled: true,
            base_url,
            api_key,
            model,
            path: String::new(),
            use_full_url: false,
        };
        service
            .save(config)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok((std::sync::Arc::new(service), dir))
    }

    #[tokio::test]
    async fn real_ai_smoke_test_identity_judge_merge_block_and_projection() {
        if std::env::var("RUN_REAL_AI_TESTS").unwrap_or_default() != "1" {
            return;
        }

        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        let (ai_service, _tmp_dir) = create_smoke_test_ai_service()
            .await
            .expect("Failed to create AI service for identity smoke test");
        let identity_judge =
            crate::service::v4::identity_judge::RealAiIdentityJudge::new(ai_service, pool.clone());
        let relationship_judge = mock_accept_judge();

        let merge_book = "identity-smoke-merge";
        progress_repo.init_progress(merge_book).await.unwrap();
        process_chapter_with_identity_judge(
            merge_book,
            1,
            "黑衣人救过李四。张三也救过李四，两段证据先作为两个身份记录。",
            &pool,
            &MockExtractor::new(vec![
                make_entity_obs("黑衣人"),
                make_entity_obs("张三"),
                make_entity_obs("李四"),
                make_rel_obs("黑衣人", "李四", "alliance", "救命恩人", 0.9),
                make_rel_obs("张三", "李四", "alliance", "救命恩人", 0.9),
            ]),
            None,
            Some(&relationship_judge),
            Some(&identity_judge),
        )
        .await
        .unwrap();
        assert_eq!(
            RelationshipRepo::new(pool.clone())
                .count_active_by_book(merge_book)
                .await
                .unwrap(),
            2,
            "fixture should start with duplicate relationship pairs before identity merge",
        );

        process_chapter_with_identity_judge(
            merge_book,
            2,
            "黑衣人摘下面具，众人才发现他就是张三；黑衣人和张三明确是同一真实人物。",
            &pool,
            &MockExtractor::new(vec![make_identity_reveal_obs("黑衣人", "张三")]),
            None,
            Some(&relationship_judge),
            Some(&identity_judge),
        )
        .await
        .unwrap();

        let identity_repo = IdentityRepo::new(pool.clone());
        let links = identity_repo
            .list_identity_links_by_book(merge_book, Some("active"))
            .await
            .unwrap();
        let redirect = links
            .iter()
            .find(|link| link.link_type == "redirect")
            .expect("real identity judge should allow an explicit reveal merge with redirect");
        let victim_id = redirect.entity_a_id.clone();

        let graph = relationship_projection::project_relationship_graph(merge_book, 2, &pool)
            .await
            .unwrap();
        assert_eq!(
            graph.total, 1,
            "relationship migration should dedupe duplicate same-pair relationships after merge",
        );
        assert!(
            graph.nodes.iter().all(|node| node.id != victim_id),
            "relationship graph should not expose merged victim as an active node",
        );
        let edge = graph.edges.first().expect("deduped relationship edge");
        assert!(
            edge.event_count >= 2,
            "deduped relationship should retain events from both original relationships",
        );
        assert!(
            !edge.latest_source_claim_id.trim().is_empty(),
            "latestSourceClaimId should stay populated after relationship dedupe",
        );

        let not_same_book = "identity-smoke-not-same";
        progress_repo.init_progress(not_same_book).await.unwrap();
        process_chapter_with_identity_judge(
            not_same_book,
            1,
            "此张三并非彼张三，二人只是同名，身份、经历和立场都不同。",
            &pool,
            &MockExtractor::new(vec![
                make_entity_obs("此张三"),
                make_entity_obs("彼张三"),
                make_not_same_identity_obs("此张三", "彼张三"),
            ]),
            None,
            None,
            Some(&identity_judge),
        )
        .await
        .unwrap();
        let not_same_links = identity_repo
            .list_identity_links_by_book(not_same_book, Some("active"))
            .await
            .unwrap();
        assert!(
            not_same_links
                .iter()
                .any(|link| link.link_type == "not_same_identity"),
            "real identity judge should create an active not_same_identity link for explicit not-same evidence",
        );

        let name_similarity_book = "identity-smoke-name-similarity";
        progress_repo
            .init_progress(name_similarity_book)
            .await
            .unwrap();
        process_chapter_with_identity_judge(
            name_similarity_book,
            1,
            "张三甲与张三乙名字相似，但文本没有说明他们是同一人。",
            &pool,
            &MockExtractor::new(vec![
                make_entity_obs("张三甲"),
                make_entity_obs("张三乙"),
                make_identity_merge_candidate_obs("张三甲", "张三乙", "仅名字相似"),
            ]),
            None,
            None,
            Some(&identity_judge),
        )
        .await
        .unwrap();
        let name_similarity_merges: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_merge_operations WHERE book_id = ? AND status = 'completed'",
        )
        .bind(name_similarity_book)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            name_similarity_merges.0, 0,
            "real identity judge must not merge solely because names are similar",
        );
    }

    #[tokio::test]
    async fn real_ai_smoke_test_admin_arcane_throne_first_chapters() {
        if std::env::var("RUN_REAL_AI_TESTS").unwrap_or_default() != "1" {
            return;
        }

        let pool = setup_pool().await;
        let book_id = "admin-arcane-throne-smoke";
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress(book_id).await.unwrap();
        let (ai_service, _tmp_dir) = create_smoke_test_ai_service()
            .await
            .expect("Failed to create AI service for Arcane Throne smoke test");
        let extractor = crate::service::v4::extractor::RealAiExtractor::new(ai_service.clone());
        let relationship_judge =
            crate::service::v4::relationship_judge::RealAiRelationshipJudge::new(
                ai_service.clone(),
                pool.clone(),
            );
        let identity_judge =
            crate::service::v4::identity_judge::RealAiIdentityJudge::new(ai_service, pool.clone());
        let chapters = read_arcane_throne_admin_chapters(3)
            .expect("admin Arcane Throne local TXT chapters should be readable");
        assert!(
            !chapters.is_empty(),
            "admin Arcane Throne local TXT fixture should include readable chapters"
        );

        for (chapter_index, title, raw_text) in &chapters {
            println!(
                "[ARCANE_SMOKE] processing chapter_index={} title={} chars={}",
                chapter_index,
                title,
                raw_text.chars().count()
            );
            process_chapter_with_identity_judge(
                book_id,
                *chapter_index,
                raw_text,
                &pool,
                &extractor,
                Some("real-ai-arcane-throne"),
                Some(&relationship_judge),
                Some(&identity_judge),
            )
            .await
            .unwrap_or_else(|err| panic!("Arcane Throne chapter {chapter_index} failed: {err}"));
        }

        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT claim_type, status, COUNT(*) FROM claims WHERE book_id = ? GROUP BY claim_type, status ORDER BY claim_type, status",
        )
        .bind(book_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        for (claim_type, status, count) in &claims {
            println!("[ARCANE_SMOKE] claim {claim_type}/{status}: {count}");
        }

        let identity_links = IdentityRepo::new(pool.clone())
            .list_identity_links_by_book(book_id, None)
            .await
            .unwrap();
        for link in &identity_links {
            println!(
                "[ARCANE_SMOKE] identity_link type={} status={} confidence={:.2} pair={}->{}",
                link.link_type, link.status, link.confidence, link.entity_a_id, link.entity_b_id
            );
        }

        let merge_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_merge_operations WHERE book_id = ? AND status = 'completed'",
        )
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let entity_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM entities WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let relationship_count = RelationshipRepo::new(pool.clone())
            .count_active_by_book(book_id)
            .await
            .unwrap();
        let identity_run_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM ai_runs WHERE book_id = ? AND run_type = 'identity_judge'",
        )
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let processed_titles: Vec<&str> = chapters
            .iter()
            .map(|(_, title, _)| title.as_str())
            .collect();
        let all_claims = claim_repo
            .list_claims_by_chapter(book_id, chapters[0].0)
            .await
            .unwrap();
        println!(
            "[ARCANE_SMOKE] processed_titles={:?} first_chapter_claims={} entities={} active_relationships={} merges={} identity_judge_runs={} identity_links={}",
            processed_titles,
            all_claims.len(),
            entity_count.0,
            relationship_count,
            merge_count.0,
            identity_run_count.0,
            identity_links.len()
        );

        assert!(
            entity_count.0 > 0,
            "real Arcane Throne source should produce canonical entities"
        );
        assert!(
            !claims.is_empty(),
            "real Arcane Throne source should produce claims"
        );
    }

    #[tokio::test]
    async fn real_ai_smoke_test_knowledge_phase4_arcane_throne() {
        if std::env::var("RUN_REAL_AI_TESTS").unwrap_or_default() != "1" {
            return;
        }

        let pool = setup_pool().await;
        let book_id = "admin-arcane-throne-knowledge-smoke";
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress(book_id).await.unwrap();
        let (ai_service, _tmp_dir) = create_smoke_test_ai_service()
            .await
            .expect("Failed to create AI service for Phase 4 knowledge smoke test");
        let extractor = crate::service::v4::extractor::RealAiExtractor::new(ai_service.clone());
        let relationship_judge =
            crate::service::v4::relationship_judge::RealAiRelationshipJudge::new(
                ai_service.clone(),
                pool.clone(),
            );
        let identity_judge = crate::service::v4::identity_judge::RealAiIdentityJudge::new(
            ai_service.clone(),
            pool.clone(),
        );
        let knowledge_judge =
            crate::service::v4::knowledge_judge::RealAiKnowledgeRevisionJudge::new(
                ai_service,
                pool.clone(),
            );

        let mut chapters: Vec<_> = read_arcane_throne_admin_chapters(1)
            .expect("admin Arcane Throne local TXT chapters should be readable");
        assert!(
            !chapters.is_empty(),
            "admin Arcane Throne local TXT fixture should include readable chapters"
        );
        for (_, title, raw_text) in &mut chapters {
            let excerpt: String = raw_text.chars().take(1800).collect();
            *raw_text = excerpt;
            title.push_str(" excerpt");
        }
        chapters.push((
            9001,
            "Phase 4 knowledge smoke fixture".to_string(),
            "奥术帝国的知识议会规定，所有正式奥术师都必须通过元素、力场和星相三类基础课程考核，考核记录由知识议会归档。年轻奥术师路西恩在旁听时得知，这条规则是长期制度，不是某个人的临时状态。随后路西恩只是整理书架，没有产生新的世界规则。".to_string(),
        ));

        for (chapter_index, title, raw_text) in &chapters {
            println!(
                "[KNOWLEDGE_SMOKE] processing chapter_index={} title={} chars={}",
                chapter_index,
                title,
                raw_text.chars().count()
            );
            process_chapter_with_knowledge_judge(
                book_id,
                *chapter_index,
                raw_text,
                &pool,
                &extractor,
                Some("real-ai-arcane-throne-knowledge"),
                Some(&relationship_judge),
                Some(&identity_judge),
                Some(&knowledge_judge),
            )
            .await
            .unwrap_or_else(|err| {
                panic!("Arcane Throne knowledge chapter {chapter_index} failed: {err}")
            });
        }

        let knowledge_repo =
            crate::storage::db::v4::knowledge_repo::KnowledgeRepo::new(pool.clone());
        let cards = knowledge_repo
            .list_cards(book_id, None, Some("active"))
            .await
            .unwrap();
        let assertions: Vec<_> = futures::future::try_join_all(
            cards
                .iter()
                .map(|card| knowledge_repo.list_assertions_for_card(&card.id)),
        )
        .await
        .unwrap()
        .into_iter()
        .flatten()
        .collect();
        let claim_counts = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT claim_type, status, COUNT(*) FROM claims WHERE book_id = ? GROUP BY claim_type, status ORDER BY claim_type, status",
        )
        .bind(book_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        for (claim_type, status, count) in &claim_counts {
            println!("[KNOWLEDGE_SMOKE] claim {claim_type}/{status}: {count}");
        }
        for card in &cards {
            println!(
                "[KNOWLEDGE_SMOKE] card category={} topic_key={} summary={:?}",
                card.category, card.topic_key, card.current_summary
            );
        }
        for assertion in &assertions {
            println!(
                "[KNOWLEDGE_SMOKE] assertion status={} text={}",
                assertion.status, assertion.assertion_text
            );
        }
        let knowledge_claim_payloads = sqlx::query_as::<_, (String, String, Option<String>)>(
            "SELECT id, status, value_json FROM claims WHERE book_id = ? AND claim_type = 'knowledge_assertion' ORDER BY chapter_index, id",
        )
        .bind(book_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        for (claim_id, status, value_json) in &knowledge_claim_payloads {
            println!(
                "[KNOWLEDGE_SMOKE] knowledge_claim id={} status={} value_json={}",
                claim_id,
                status,
                value_json.as_deref().unwrap_or("")
            );
        }
        let knowledge_runs = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
            "SELECT status, output_json, error FROM ai_runs WHERE book_id = ? AND run_type = 'knowledge_revision_judge' ORDER BY started_at, id",
        )
        .bind(book_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        for (status, output_json, error) in &knowledge_runs {
            println!(
                "[KNOWLEDGE_SMOKE] knowledge_judge_run status={} output={} error={}",
                status,
                output_json.as_deref().unwrap_or(""),
                error.as_deref().unwrap_or("")
            );
        }

        let knowledge_claim_count: i64 = claim_counts
            .iter()
            .filter(|(claim_type, _, _)| claim_type == "knowledge_assertion")
            .map(|(_, _, count)| *count)
            .sum();
        assert!(
            knowledge_claim_count > 0,
            "real Phase 4 knowledge smoke should produce knowledge_assertion claims"
        );
        assert!(
            !cards.is_empty(),
            "real Phase 4 knowledge smoke should produce at least one knowledge card"
        );
        assert!(
            assertions
                .iter()
                .any(|assertion| matches!(assertion.status.as_str(), "active" | "rumor" | "uncertain" | "false_in_world")),
            "real Phase 4 knowledge smoke should persist at least one canonical knowledge assertion"
        );
        assert!(
            assertions
                .iter()
                .all(|assertion| !assertion.assertion_text.contains("整理书架")),
            "non-knowledge distractor actions should not become canonical knowledge assertions"
        );

        let unresolved_refs: (i64,) = sqlx::query_as(
            "SELECT COUNT(*)
             FROM knowledge_assertion_entities kae
             LEFT JOIN entities e ON e.id = kae.entity_id
             WHERE kae.book_id = ? AND e.id IS NULL",
        )
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            unresolved_refs.0, 0,
            "knowledge assertion entity refs should resolve to canonical entities"
        );
    }

    #[tokio::test]
    async fn real_ai_smoke_test_knowledge_phase4_freeze_fixture() {
        if std::env::var("RUN_REAL_AI_TESTS").unwrap_or_default() != "1" {
            return;
        }

        let pool = setup_pool().await;
        let book_id = "phase4-knowledge-freeze-smoke";
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress(book_id).await.unwrap();
        let (ai_service, _tmp_dir) = create_smoke_test_ai_service()
            .await
            .expect("Failed to create AI service for Phase 4 freeze smoke test");
        let knowledge_judge =
            crate::service::v4::knowledge_judge::RealAiKnowledgeRevisionJudge::new(
                ai_service,
                pool.clone(),
            );
        let relationship_judge = mock_accept_judge();
        let identity_judge = crate::service::v4::identity_judge::MockIdentityJudge::new(
            crate::service::v4::identity_judge::IdentityJudgeOutput {
                decision: crate::service::v4::identity_judge::IdentityJudgeDecision::Merge,
                link_type: "same_identity".to_string(),
                survivor_hint: "entity_b".to_string(),
                confidence: 0.96,
                reason_code: "test_reveal".to_string(),
                explanation_for_log: "test reveal".to_string(),
                property_conflicts: vec![],
                relationship_migration_hint: "safe".to_string(),
            },
        );

        let chapters = vec![
            (
                1,
                "知识议会与路西恩登场。传闻知识议会已经废除了星相课程。路西恩和艾丽莎只是朋友关系。",
                MockExtractor::new(vec![
                    make_entity_obs("知识议会"),
                    make_entity_obs("路西恩"),
                    make_entity_obs("艾丽莎"),
                    make_rel_obs("路西恩", "艾丽莎", "friendship", "朋友", 0.9),
                    crate::service::v4::extractor::Observation::KnowledgeAssertion {
                        category: "faction_structure".to_string(),
                        topic: "正式奥术师基础课程要求".to_string(),
                        assertion_text: "传闻知识议会已经废除了星相课程。".to_string(),
                        confidence: 0.86,
                        importance_score: 0.78,
                        evidence_span_ids: vec![],
                        referenced_entity_mentions: vec![
                            crate::service::v4::extractor::KnowledgeEntityMention {
                                mention: "知识议会".to_string(),
                                entity_type_hint: Some("organization".to_string()),
                                role: "faction".to_string(),
                                confidence: 0.91,
                                resolved_entity_id: None,
                            },
                        ],
                        status_hint: Some("rumor".to_string()),
                        reason_hint: Some("test rumor with referenced entity".to_string()),
                    },
                ]),
            ),
            (
                2,
                "档案确认传闻错误：星相课程从未废除，正式奥术师仍必须通过元素、力场和星相三类基础课程考核。路西恩突破一环是个人状态，不是世界知识。",
                MockExtractor::new(vec![
                    crate::service::v4::extractor::Observation::PropertyUpdate {
                        subject_mention: "路西恩".to_string(),
                        dimension_key: "realm".to_string(),
                        value_text: Some("一环".to_string()),
                        value_json: None,
                        evidence_span_ids: vec![],
                        confidence: 0.9,
                    },
                    crate::service::v4::extractor::Observation::KnowledgeAssertion {
                        category: "faction_structure".to_string(),
                        topic: "正式奥术师基础课程要求".to_string(),
                        assertion_text: "星相课程从未废除，正式奥术师仍必须通过元素、力场和星相三类基础课程考核。".to_string(),
                        confidence: 0.94,
                        importance_score: 0.9,
                        evidence_span_ids: vec![],
                        referenced_entity_mentions: vec![
                            crate::service::v4::extractor::KnowledgeEntityMention {
                                mention: "知识议会".to_string(),
                                entity_type_hint: Some("organization".to_string()),
                                role: "faction".to_string(),
                                confidence: 0.9,
                                resolved_entity_id: None,
                            },
                        ],
                        status_hint: Some("fact".to_string()),
                        reason_hint: Some("test correction of prior rumor".to_string()),
                    },
                ]),
            ),
            (
                3,
                "黑衣导师摘下面具，众人才知道黑衣导师就是费尔南多。青云门在东域以北只是地图方向，不应写入 Phase 4 knowledge。",
                MockExtractor::new(vec![
                    make_entity_obs("黑衣导师"),
                    make_entity_obs("费尔南多"),
                    make_identity_reveal_obs("黑衣导师", "费尔南多"),
                ]),
            ),
            (
                4,
                "奥术师等级体系分为学徒、正式奥术师和高阶奥术师。知识议会继续管理基础课程档案。",
                MockExtractor::new(vec![make_knowledge_obs(
                    "power_system",
                    "奥术师等级体系",
                    "奥术师等级体系分为学徒、正式奥术师和高阶奥术师。",
                )]),
            ),
            (
                5,
                "坊间传闻，真理之钟每逢满月会自动改写所有魔法契约；档案尚未确认这条世界规则。",
                MockExtractor::new(vec![
                    crate::service::v4::extractor::Observation::KnowledgeAssertion {
                        category: "world_rule".to_string(),
                        topic: "真理之钟满月传闻".to_string(),
                        assertion_text: "传闻真理之钟每逢满月会自动改写所有魔法契约。".to_string(),
                        confidence: 0.84,
                        importance_score: 0.76,
                        evidence_span_ids: vec![],
                        referenced_entity_mentions: vec![],
                        status_hint: Some("rumor".to_string()),
                        reason_hint: Some("unconfirmed long-term world rule rumor".to_string()),
                    },
                ]),
            ),
        ];

        for (chapter_index, raw_text, extractor) in &chapters {
            process_chapter_with_knowledge_judge(
                book_id,
                *chapter_index,
                raw_text,
                &pool,
                extractor,
                Some("real-ai-phase4-freeze-fixture"),
                Some(&relationship_judge),
                Some(&identity_judge),
                Some(&knowledge_judge),
            )
            .await
            .unwrap_or_else(|err| {
                panic!("Phase 4 freeze fixture chapter {chapter_index} failed: {err}")
            });
        }

        let knowledge_repo =
            crate::storage::db::v4::knowledge_repo::KnowledgeRepo::new(pool.clone());
        let cards = knowledge_repo
            .list_cards(book_id, None, Some("active"))
            .await
            .unwrap();
        let mut assertions = Vec::new();
        for card in &cards {
            assertions.extend(
                knowledge_repo
                    .list_assertions_for_card(&card.id)
                    .await
                    .unwrap(),
            );
        }
        let statuses: std::collections::HashSet<_> = assertions
            .iter()
            .map(|assertion| assertion.status.as_str())
            .collect();
        let links: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM knowledge_assertion_links WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let knowledge_runs: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM ai_runs WHERE book_id = ? AND run_type = 'knowledge_revision_judge' AND status = 'success' AND output_json IS NOT NULL",
        )
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let knowledge_run_rows = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
            "SELECT status, output_json, error FROM ai_runs WHERE book_id = ? AND run_type = 'knowledge_revision_judge' ORDER BY started_at, id",
        )
        .bind(book_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        for (status, output_json, error) in &knowledge_run_rows {
            println!(
                "[FREEZE_SMOKE] knowledge_judge_run status={} output={} error={}",
                status,
                output_json.as_deref().unwrap_or(""),
                error.as_deref().unwrap_or("")
            );
        }
        let ref_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM knowledge_assertion_entities WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let claim_rows = sqlx::query_as::<_, (i64, String, String, Option<String>)>(
            "SELECT chapter_index, claim_type, status, value_json FROM claims WHERE book_id = ? ORDER BY chapter_index, claim_type, status",
        )
        .bind(book_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        for (chapter_index, claim_type, status, value_json) in &claim_rows {
            println!(
                "[FREEZE_SMOKE] claim chapter={} type={} status={} value={}",
                chapter_index,
                claim_type,
                status,
                value_json.as_deref().unwrap_or("")
            );
        }

        println!(
            "[FREEZE_SMOKE] cards={} assertions={} statuses={:?} links={} refs={} judge_runs={}",
            cards.len(),
            assertions.len(),
            statuses,
            links.0,
            ref_count.0,
            knowledge_runs.0
        );
        for assertion in &assertions {
            println!(
                "[FREEZE_SMOKE] assertion status={} text={}",
                assertion.status, assertion.assertion_text
            );
        }

        assert!(
            cards.iter().any(|card| card.category == "power_system"),
            "freeze fixture should create a power_system card"
        );
        assert!(
            cards
                .iter()
                .any(|card| card.category == "faction_structure"),
            "freeze fixture should create a faction_structure card"
        );
        assert!(
            cards.iter().any(|card| card.category == "world_rule"),
            "freeze fixture should create a world_rule card"
        );
        assert!(
            statuses.contains("rumor") || statuses.contains("false_in_world"),
            "freeze fixture should preserve rumor or false_in_world knowledge"
        );
        assert!(
            links.0 > 0 || statuses.contains("contradicted") || statuses.contains("revised"),
            "freeze fixture should exercise revision/contradiction handling"
        );
        assert!(
            ref_count.0 > 0,
            "freeze fixture should resolve referenced entities"
        );
        assert!(
            knowledge_runs.0 >= 3,
            "freeze fixture should call RealAiKnowledgeRevisionJudge"
        );
        assert!(
            assertions.iter().all(|assertion| {
                !assertion.assertion_text.contains("朋友")
                    && !assertion.assertion_text.contains("突破一环")
                    && !assertion.assertion_text.contains("东域以北")
                    && !assertion.assertion_text.contains("黑衣导师就是费尔南多")
            }),
            "relationship/property/map/identity distractors should not become canonical knowledge"
        );
    }

    struct SmokeMapJudge {
        real: crate::service::v4::map_conflict_judge::RealAiMapConflictJudge,
        real_calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        scripted_conflicts: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        real_errors: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    #[axum::async_trait]
    impl crate::service::v4::map_conflict_judge::MapConflictJudge for SmokeMapJudge {
        async fn judge(
            &self,
            input: &crate::service::v4::map_conflict_judge::MapConflictJudgeInput,
        ) -> anyhow::Result<crate::service::v4::map_conflict_judge::MapConflictJudgeOutput>
        {
            let call_index = self
                .real_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if call_index == 0 {
                if let Err(err) = self.real.judge(input).await {
                    self.real_errors.lock().unwrap().push(err.to_string());
                }
            }

            if input.edge_type == "north_of" {
                self.scripted_conflicts
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return Ok(crate::service::v4::map_conflict_judge::MapConflictJudgeOutput {
                    decision: crate::service::v4::map_conflict_judge::MapConflictDecision::Conflict,
                    normalized_edge_type: Some(input.edge_type.clone()),
                    normalized_direction_hint: Some("north".to_string()),
                    normalized_distance_hint: None,
                    conflict_type: Some("direction_conflict".to_string()),
                    reason_code: "phase5_smoke_direction_conflict".to_string(),
                    confidence: 0.93,
                    explanation_for_log: "smoke fixture forces one conflicting direction so the canonical conflict path is deterministic".to_string(),
                });
            }

            Ok(crate::service::v4::map_conflict_judge::MapConflictJudgeOutput {
                decision: crate::service::v4::map_conflict_judge::MapConflictDecision::Accept,
                normalized_edge_type: Some(input.edge_type.clone()),
                normalized_direction_hint: None,
                normalized_distance_hint: Some("smoke fixture distance".to_string()),
                conflict_type: None,
                reason_code: "phase5_smoke_accept".to_string(),
                confidence: 0.91,
                explanation_for_log: "smoke fixture accepts stable topology after real judge connectivity is exercised".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn real_ai_smoke_test_map_phase5_fixture() {
        if std::env::var("RUN_REAL_AI_TESTS").unwrap_or_default() != "1" {
            return;
        }

        let pool = setup_pool().await;
        let book_id = "phase5-map-smoke";
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress(book_id).await.unwrap();
        let (ai_service, _tmp_dir) = create_smoke_test_ai_service()
            .await
            .expect("Failed to create AI service for Phase 5 map smoke test");

        let real_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let scripted_conflicts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let real_errors = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let map_judge = SmokeMapJudge {
            real: crate::service::v4::map_conflict_judge::RealAiMapConflictJudge::new(
                ai_service,
                pool.clone(),
            ),
            real_calls: real_calls.clone(),
            scripted_conflicts: scripted_conflicts.clone(),
            real_errors: real_errors.clone(),
        };
        let relationship_judge = mock_accept_judge();
        let identity_judge = crate::service::v4::identity_judge::MockIdentityJudge::new(
            crate::service::v4::identity_judge::IdentityJudgeOutput {
                decision: crate::service::v4::identity_judge::IdentityJudgeDecision::Uncertain,
                link_type: "possible_same_identity".to_string(),
                survivor_hint: "none".to_string(),
                confidence: 0.5,
                reason_code: "phase5_smoke_identity_neutral".to_string(),
                explanation_for_log: "identity is out of scope for this map smoke".to_string(),
                property_conflicts: vec![],
                relationship_migration_hint: "not_applicable".to_string(),
            },
        );

        let organization_obs = crate::service::v4::extractor::Observation::EntityIntroduction {
            subject_mention: "青云门".to_string(),
            entity_type: "organization".to_string(),
            aliases: vec![],
            short_summary: "青云门是东域宗门组织。".to_string(),
            evidence_span_ids: vec![],
            confidence: 0.92,
        };
        let movement_distractor = crate::service::v4::extractor::Observation::PropertyUpdate {
            subject_mention: "林澈".to_string(),
            dimension_key: "location".to_string(),
            value_text: Some("丹房".to_string()),
            value_json: None,
            evidence_span_ids: vec![],
            confidence: 0.88,
        };

        let chapters = vec![
            (
                1,
                "东域包含青云门，青云门内有丹房。青云门到黑水渡有一条后山古道。青云门同时也是宗门组织名。林澈从丹房走到庭院只是人物移动；苏晚和林澈是同门朋友。东域多山只是地理知识摘要。",
                MockExtractor::new(vec![
                    make_entity_obs("林澈"),
                    make_entity_obs("苏晚"),
                    organization_obs,
                    make_rel_obs("林澈", "苏晚", "friendship", "同门朋友", 0.9),
                    movement_distractor,
                    make_knowledge_obs("geography", "东域地貌", "东域多山，宗门沿山势分布。"),
                    make_location_intro_obs("东域", "region"),
                    crate::service::v4::extractor::Observation::LocationIntroduction {
                        place_mention: "青云门".to_string(),
                        place_type: "sect_site".to_string(),
                        parent_place_mention: Some("东域".to_string()),
                        aliases: vec!["青云山门".to_string()],
                        description: Some("青云门在东域山中。".to_string()),
                        importance_score: 0.9,
                        map_visible_hint: Some(true),
                        evidence_span_ids: vec![],
                        confidence: 0.93,
                    },
                    crate::service::v4::extractor::Observation::LocationIntroduction {
                        place_mention: "丹房".to_string(),
                        place_type: "building".to_string(),
                        parent_place_mention: Some("青云门".to_string()),
                        aliases: vec![],
                        description: Some("丹房位于青云门内。".to_string()),
                        importance_score: 0.7,
                        map_visible_hint: Some(true),
                        evidence_span_ids: vec![],
                        confidence: 0.9,
                    },
                    make_location_intro_obs("后山古道", "route"),
                    make_location_intro_obs("黑水渡", "city"),
                    make_location_edge_obs("东域", "青云门", "contains"),
                    make_location_edge_obs("青云门", "丹房", "contains"),
                    make_location_edge_obs("青云门", "黑水渡", "route_to"),
                ]),
            ),
            (
                2,
                "后续传闻称丹房在青云门以北，这与丹房位于青云门内的拓扑说法冲突，应进入地图冲突记录而不是 active map。林澈回到丹房仍然只是人物当前位置。",
                MockExtractor::new(vec![
                    crate::service::v4::extractor::Observation::PropertyUpdate {
                        subject_mention: "林澈".to_string(),
                        dimension_key: "location".to_string(),
                        value_text: Some("丹房".to_string()),
                        value_json: None,
                        evidence_span_ids: vec![],
                        confidence: 0.88,
                    },
                    make_location_edge_obs("丹房", "青云门", "north_of"),
                ]),
            ),
            (
                3,
                "黑水渡在青云门以东。后来又有人说青云门在黑水渡以西，这是同一条方向关系的反向说法，不应生成重复边。伪书记载青云门包含东域，会造成东域包含青云门后的层级循环，应拒绝。后山古道仍是命名路线。",
                MockExtractor::new(vec![
                    make_location_edge_obs("黑水渡", "青云门", "east_of"),
                    make_location_edge_obs("青云门", "黑水渡", "west_of"),
                    make_location_edge_obs("青云门", "东域", "contains"),
                ]),
            ),
        ];
        assert_eq!(
            chapters.len(),
            3,
            "Phase 5 map smoke must cover a 3-chapter fixture"
        );

        for (chapter_index, raw_text, extractor) in &chapters {
            process_chapter_with_map_conflict_judge(
                book_id,
                *chapter_index,
                raw_text,
                &pool,
                extractor,
                Some("real-ai-phase5-map-fixture"),
                Some(&relationship_judge),
                Some(&identity_judge),
                None,
                Some(&map_judge),
            )
            .await
            .unwrap_or_else(|err| {
                panic!("Phase 5 map smoke chapter {chapter_index} failed: {err}")
            });
        }

        let overview = crate::service::v4::place_projection::project_map_overview(book_id, &pool)
            .await
            .unwrap();
        let places = crate::service::v4::place_projection::project_all_places(book_id, &pool)
            .await
            .unwrap();
        let graph = crate::service::v4::place_projection::project_map_graph(book_id, &pool)
            .await
            .unwrap();
        let layout =
            crate::service::v4::place_projection::rebuild_layout_snapshot(book_id, 3, &pool)
                .await
                .unwrap();
        let conflicts: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM place_edge_conflicts WHERE book_id = ? AND status = 'open'",
        )
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let map_judge_runs: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM ai_runs WHERE book_id = ? AND run_type = 'map_conflict_judge' AND status = 'success' AND output_json IS NOT NULL",
        )
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let false_positive_edges: (i64,) = sqlx::query_as(
            "SELECT COUNT(*)
             FROM place_edges e
             JOIN entities a ON a.id = e.from_place_id
             JOIN entities b ON b.id = e.to_place_id
             WHERE e.book_id = ?
               AND (a.display_name IN ('林澈', '苏晚') OR b.display_name IN ('林澈', '苏晚'))",
        )
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let relationship_count = RelationshipRepo::new(pool.clone())
            .count_active_by_book(book_id)
            .await
            .unwrap();
        let org_place_links: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_links WHERE book_id = ? AND link_type = 'organization_place_pair' AND status = 'active'",
        )
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let edge_sources: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM place_edge_sources WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let aliases_created: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM entity_aliases WHERE book_id = ?")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let inverse_duplicate_edges: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM place_edges WHERE book_id = ? AND edge_type = 'east_of' AND status = 'active'",
        )
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let rejected_cycle_claims: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM claims WHERE book_id = ? AND claim_type = 'location_edge' AND status = 'rejected'",
        )
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let character_cards: Vec<_> = EntityRepo::new(pool.clone())
            .list_by_book(book_id)
            .await
            .unwrap()
            .into_iter()
            .filter(|entity| entity.entity_type == "character")
            .collect();
        let first_card_ok = if let Some(entity) = character_cards.first() {
            crate::service::v4::projection::project_character_card(&entity.id, book_id, 2, &pool)
                .await
                .is_ok()
        } else {
            false
        };
        let map_run_rows = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
            "SELECT status, output_json, error FROM ai_runs WHERE book_id = ? AND run_type = 'map_conflict_judge' ORDER BY started_at, id",
        )
        .bind(book_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        for (status, output_json, error) in &map_run_rows {
            println!(
                "[MAP_SMOKE] map_judge_run status={} output={} error={}",
                status,
                output_json.as_deref().unwrap_or(""),
                error.as_deref().unwrap_or("")
            );
        }
        println!(
            "[MAP_SMOKE] places={} active_edges={} conflicts={} layout_nodes={} layout_edges={} real_calls={} scripted_conflicts={} relationships={} org_place_links={} aliases={} edge_sources={} inverse_duplicate_edges={} rejected_cycle_claims={} false_positive_edges={}",
            overview.place_count,
            overview.active_edge_count,
            conflicts.0,
            layout.nodes.len(),
            layout.edges.len(),
            real_calls.load(std::sync::atomic::Ordering::SeqCst),
            scripted_conflicts.load(std::sync::atomic::Ordering::SeqCst),
            relationship_count,
            org_place_links.0,
            aliases_created.0,
            edge_sources.0,
            inverse_duplicate_edges.0,
            rejected_cycle_claims.0,
            false_positive_edges.0
        );
        for place in &places {
            println!(
                "[MAP_SMOKE] place id={} name={} type={}",
                place.id, place.name, place.place_type
            );
        }
        for edge in &graph.edges {
            println!(
                "[MAP_SMOKE] edge {} {} -> {}",
                edge.edge_type, edge.from_place_id, edge.to_place_id
            );
        }

        let real_errors = real_errors.lock().unwrap().clone();
        assert!(
            real_errors.is_empty(),
            "RealAiMapConflictJudge failed: {}",
            real_errors.join("; ")
        );
        assert_eq!(
            map_judge_runs.0, 1,
            "RealAiMapConflictJudge should record a successful ai_run"
        );
        assert_eq!(
            overview.place_count, 5,
            "distractors must not create extra places"
        );
        assert_eq!(
            overview.active_edge_count, 4,
            "inverse duplicate should dedupe into one extra active direction edge"
        );
        assert_eq!(
            conflicts.0, 1,
            "conflicting map statement should be recorded"
        );
        assert!(
            real_calls.load(std::sync::atomic::Ordering::SeqCst) >= 1,
            "smoke wrapper should call the real map judge"
        );
        assert_eq!(
            scripted_conflicts.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "fixture should exercise deterministic conflict path"
        );
        assert_eq!(
            false_positive_edges.0, 0,
            "character movement distractor must not enter map"
        );
        assert_eq!(
            relationship_count, 1,
            "Phase 2 relationship regression should still pass"
        );
        assert_eq!(
            org_place_links.0, 1,
            "organization/place same-name link should be active"
        );
        assert!(aliases_created.0 >= 1, "place alias should be persisted");
        assert_eq!(
            edge_sources.0, 5,
            "inverse duplicate should preserve source evidence without duplicate active edge"
        );
        assert_eq!(
            inverse_duplicate_edges.0, 1,
            "east/west inverse pair should canonicalize to one east_of edge"
        );
        assert_eq!(
            rejected_cycle_claims.0, 1,
            "hierarchy cycle attempt should be rejected"
        );
        assert!(
            first_card_ok,
            "Phase 1 character card projection should still work"
        );
        assert!(
            !layout.nodes.is_empty(),
            "layout snapshot should include place nodes"
        );
        assert!(
            !layout.edges.is_empty(),
            "layout snapshot should include active edges"
        );
    }

    #[tokio::test]
    async fn real_ai_smoke_test_relationship_extraction() {
        // Gate: only run when explicitly enabled
        if std::env::var("RUN_REAL_AI_TESTS").unwrap_or_default() != "1" {
            return;
        }

        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let (ai_service, _tmp_dir) = create_smoke_test_ai_service()
            .await
            .expect("Failed to create AI service for smoke test");
        let extractor = crate::service::v4::extractor::RealAiExtractor::new(ai_service.clone());

        // Use RealAiRelationshipJudge for real AI smoke test
        let judge = crate::service::v4::relationship_judge::RealAiRelationshipJudge::new(
            ai_service.clone(),
            pool.clone(),
        );

        // Sample chapter text with characters and relationships
        let chapters = vec![
            (
                1i64,
                "张三是一个年轻的修士，他从小在青云门长大。他的师父是李真人，一位金丹期的高手。李真人视张三如己出，倾囊相授。张三还有一个师妹叫小红，两人从小一起长大，情同手足。",
            ),
            (
                2,
                "张三和李真人一起下山历练。途中遇到了王五，一个神秘的散修。王五和李真人曾经是同门师兄弟，后来因为一本秘籍反目成仇，从此势不两立。王五发誓要找李真人报仇。",
            ),
            (
                3,
                "张三突破到了筑基期。小红为他高兴，两人约定一起闯荡江湖。李真人对张三的进步很满意，决定将掌门之位传给他。王五暗中观察，等待报仇的机会。",
            ),
        ];

        for (idx, text) in &chapters {
            let mut last_err = None;
            for attempt in 0..3 {
                if attempt > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
                match process_chapter("b1", *idx, text, &pool, &extractor, None, Some(&judge)).await
                {
                    Ok(_) => {
                        last_err = None;
                        break;
                    }
                    Err(e) => {
                        eprintln!(
                            "[SMOKE] Chapter {} attempt {} failed: {}",
                            idx,
                            attempt + 1,
                            e
                        );
                        last_err = Some(e);
                    }
                }
            }
            if let Some(e) = last_err {
                panic!("Chapter {} failed after 3 attempts: {}", idx, e);
            }
        }

        // Count relationship_update claims across all chapters
        let claim_repo = ClaimRepo::new(pool.clone());
        let mut total_relationship_updates = 0;
        let mut accepted = 0;
        let mut rejected = 0;
        let mut uncertain = 0;
        let mut redirected = 0;
        let mut other_status = 0;

        for (idx, _) in &chapters {
            let claims = claim_repo.list_claims_by_chapter("b1", *idx).await.unwrap();
            for claim in &claims {
                if claim.claim_type == "relationship_update" {
                    total_relationship_updates += 1;
                    match claim.status.as_str() {
                        "accepted" => accepted += 1,
                        "rejected" => rejected += 1,
                        "uncertain" => uncertain += 1,
                        "redirected" => redirected += 1,
                        _ => other_status += 1,
                    }
                }
            }
        }

        // Count entities and relationships
        let entity_repo = EntityRepo::new(pool.clone());
        let entities = entity_repo.list_by_book("b1").await.unwrap();
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rels = rel_repo.list_by_book("b1", None, None).await.unwrap();
        let active_count = rel_repo.count_active_by_book("b1").await.unwrap();

        // Print summary (visible with `cargo test -- --nocapture`)
        // Print ai_run details for relationship_judge
        let judge_runs = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>)>(
            "SELECT id, model, status, output_json, error FROM ai_runs WHERE book_id = 'b1' AND run_type = 'relationship_judge' ORDER BY started_at"
        ).fetch_all(&pool).await.unwrap_or_default();
        for (id, model, status, output, error) in &judge_runs {
            println!(
                "[SMOKE] Judge ai_run {}: model={}, status={}",
                id, model, status
            );
            if let Some(ref o) = output {
                let preview: String = o.chars().take(500).collect();
                let preview = preview.as_str();
                println!("[SMOKE] Judge output: {}", preview);
            }
            if let Some(ref e) = error {
                println!("[SMOKE] Judge error: {}", e);
            }
        }

        println!("=== Real AI Smoke Test Results ===");
        println!("Entities created: {}", entities.len());
        for e in &entities {
            println!(
                "  - [{}] {} (importance: {})",
                e.entity_type, e.canonical_name, e.importance_score
            );
        }
        println!("Relationship update claims: {}", total_relationship_updates);
        println!("  accepted: {}", accepted);
        println!("  rejected: {}", rejected);
        println!("  uncertain: {}", uncertain);
        println!("  redirected: {}", redirected);
        println!("  other: {}", other_status);
        println!("Active relationships: {}", active_count);
        for r in &rels {
            println!(
                "  - {} -> {} [{}] {} (strength: {}, polarity: {})",
                r.subject_character_id,
                r.object_character_id,
                r.relation_group,
                r.relation_label,
                r.strength,
                r.polarity
            );
        }

        // Basic assertions
        assert!(
            !entities.is_empty(),
            "Real AI should extract at least some entities"
        );

        // Phase 1 character card should still work
        if let Some(entity) = entities.first() {
            let card =
                crate::service::v4::projection::project_character_card(&entity.id, "b1", 3, &pool)
                    .await;
            assert!(
                card.is_ok(),
                "Character card projection should work after real AI extraction"
            );
        }
    }
}
