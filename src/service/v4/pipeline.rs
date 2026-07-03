use crate::service::v4::claim_writer;
use crate::service::v4::context_builder::ContextBuilder;
use crate::service::v4::extractor::Extractor;
use crate::service::v4::projection;
use crate::service::v4::reducer;
use crate::service::v4::relationship_judge::{self, GateResult, JudgeDecision, JudgeOutput};
use crate::service::v4::relationship_projection;
use crate::service::v4::resolver;
use crate::storage::db::v4::ai_run_repo::AiRunRepo;
use crate::storage::db::v4::chapter_repo::{self, ChapterRepo};
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::progress_repo::ProgressRepo;
use sqlx::SqlitePool;

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
        .is_already_processed(book_id, chapter_index, &text_hash, PROMPT_VERSION, SCHEMA_VERSION)
        .await?
    {
        return Ok(());
    }

    // 3. Create chapter_processing_run
    let processing_run = progress_repo
        .create_run(book_id, chapter_index, &text_hash, PROMPT_VERSION, SCHEMA_VERSION)
        .await?;

    // 4. Segment chapter
    let segments_data = chapter_repo::segment_chapter(book_id, &chapter.id, &text_hash, raw_text);

    // Check if segments already exist for this chapter (active or stale with same hash)
    let existing_segments = chapter_repo.list_active_segments(&chapter.id).await?;
    let has_matching_segments = !existing_segments.is_empty()
        && existing_segments.iter().any(|s| s.chapter_hash == text_hash);

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
            let segment_text: String = spans.iter().map(|s| s.text_excerpt.as_str()).collect::<Vec<_>>().join("\n\n");
            segment_info.push((segment.id.clone(), segment_text, span_ids));
        }
    } else {
        // Create new segments and spans
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
            for (span_index, (start, end, excerpt)) in spans_data.iter().enumerate() {
                let span = claim_repo
                    .create_span(
                        book_id,
                        &chapter.id,
                        &text_hash,
                        &segment.id,
                        span_index as i64,
                        *start,
                        *end,
                        excerpt,
                    )
                    .await?;
                span_ids.push(span.id);
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

        // f. Phase 2 pipeline: separate relationship claims from other claims
        let (relationship_claims, other_claims): (Vec<_>, Vec<_>) = claim_result
            .claims_created
            .iter()
            .cloned()
            .partition(|c| c.claim_type == "relationship_update");

        // g. Phase 1 reducer FIRST (entity + property) — ensures new characters enter entities
        let _reduction_result = reducer::reduce_claims(
            &other_claims,
            book_id,
            pool,
        )
        .await?;

        // h. Phase 2: process relationship_update claims
        if !relationship_claims.is_empty() {
            let default_judge = DefaultJudge::new();
            let judge_ref = judge.unwrap_or(&default_judge);

            // Re-resolve relationship claims using newly created entities.
            // After Phase 1 reduction, new characters may have been added to entities.
            let mut re_resolved_claims = Vec::new();
            for rel_claim in &relationship_claims {
                let mut claim = rel_claim.clone();
                if let Some(ref subject_mention) = claim.subject_mention {
                    if let Ok(Some(entity)) =
                        entity_repo.find_entity_by_alias(book_id, subject_mention).await
                    {
                        claim.subject_entity_id = Some(entity.id);
                    }
                }
                if let Some(ref object_mention) = claim.object_mention {
                    if let Ok(Some(entity)) =
                        entity_repo.find_entity_by_alias(book_id, object_mention).await
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

                let gate_result =
                    relationship_judge::structural_gate(claim, subject_entity.as_ref(), object_entity.as_ref());

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
                        tracing::debug!(
                            "Relationship claim {} uncertain: {}",
                            claim.id,
                            reason
                        );
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
            let _rel_reduction = reducer::reduce_relationship_claims(
                &judge_accepted,
                book_id,
                pool,
            )
            .await?;

            // Second-pass property_reducer for derived redirect property_update claims
            if !redirect_claims.is_empty() {
                let _redirect_reduction = reducer::reduce_claims(
                    &redirect_claims,
                    book_id,
                    pool,
                )
                .await?;
            }
        }

        // g. Mark AI run as success with output_json
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

    // --- Existing Phase 1 tests (updated for new signature with judge parameter) ---

    #[tokio::test]
    async fn process_chapter_creates_source_spans_and_ai_runs() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三走进了大殿，看到了李四。两人互相行礼。";
        let extractor = empty_extractor();

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, None).await.unwrap();

        // Verify chapter exists
        let chapter_repo = ChapterRepo::new(pool.clone());
        let chapter = chapter_repo.get_chapter("b1", 1).await.unwrap();
        assert!(chapter.is_some(), "chapter should exist");

        // Verify source spans were created
        let chapter = chapter.unwrap();
        let segments = chapter_repo.list_active_segments(&chapter.id).await.unwrap();
        assert!(!segments.is_empty(), "should have segments");

        let claim_repo = ClaimRepo::new(pool.clone());
        for seg in &segments {
            let spans = claim_repo.list_spans_by_segment(&seg.id).await.unwrap();
            assert!(!spans.is_empty(), "segment should have spans");
        }

        // Verify ai_runs were created
        let ai_run_repo = AiRunRepo::new(pool.clone());
        let runs = ai_run_repo.list_runs_by_chapter("b1", &chapter.id).await.unwrap();
        assert!(!runs.is_empty(), "should have ai_runs");
        assert_eq!(runs[0].status, "success", "ai_run should be success");
        // Verify segment_id is populated
        assert!(runs[0].segment_id.is_some(), "ai_run.segment_id should be populated");
        // Verify segment_id points to a valid segment
        let seg_id = runs[0].segment_id.as_ref().unwrap();
        let seg = chapter_repo.get_segment(seg_id).await.unwrap();
        assert!(seg.is_some(), "ai_run.segment_id should point to a valid segment");
    }

    #[tokio::test]
    async fn process_chapter_idempotent_skip() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "第一章内容。";
        let extractor = empty_extractor();

        // First run
        process_chapter("b1", 1, raw_text, &pool, &extractor, None, None).await.unwrap();

        // Get chapter to count runs
        let chapter_repo = ChapterRepo::new(pool.clone());
        let chapter = chapter_repo.get_chapter("b1", 1).await.unwrap().unwrap();
        let ai_run_repo = AiRunRepo::new(pool.clone());
        let runs_before = ai_run_repo.list_runs_by_chapter("b1", &chapter.id).await.unwrap();

        // Second run (same text -> same hash -> should skip)
        process_chapter("b1", 1, raw_text, &pool, &extractor, None, None).await.unwrap();
        let runs_after = ai_run_repo.list_runs_by_chapter("b1", &chapter.id).await.unwrap();

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
        let property_repo =
            crate::storage::db::v4::property_repo::PropertyRepo::new(pool.clone());
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
        let summary = chapter_repo
            .get_chapter_summary("b1", 1)
            .await
            .unwrap();
        assert!(summary.is_some(), "chapter summary should exist");
        assert_eq!(
            summary.unwrap().summary,
            "本章介绍张三突破金丹"
        );

        // --- Verify claims ---
        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo
            .list_claims_by_chapter("b1", 1)
            .await
            .unwrap();
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
        let accepted: Vec<&ClaimRecord> = claims
            .iter()
            .filter(|c| c.status == "accepted")
            .collect();
        assert_eq!(
            accepted.len(),
            3,
            "entity_introduction, alias, property_update should all be accepted"
        );

        // --- Verify view_model_cache ---
        let cache_repo =
            crate::storage::db::v4::cache_repo::CacheRepo::new(pool.clone());
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
        let card = crate::service::v4::projection::project_character_card(
            &entity.id, "b1", 1, &pool,
        )
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
        assert!(entity.is_some(), "entity should be created from low-risk introduction");
        let entity = entity.unwrap();

        // --- Verify claims ---
        let claim_repo = ClaimRepo::new(pool.clone());
        let claims = claim_repo
            .list_claims_by_chapter("b1", 1)
            .await
            .unwrap();

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
        let property_repo =
            crate::storage::db::v4::property_repo::PropertyRepo::new(pool.clone());
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
        let e1 = entity_repo.find_entity_by_alias("b1", "张三").await.unwrap();
        let e2 = entity_repo.find_entity_by_alias("b1", "李四").await.unwrap();
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
        assert_eq!(rels.len(), 0, "no relationship should be created on gate reject");

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
        assert_eq!(rels.len(), 0, "no relationship should be created (redirect)");

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
        let e1 = entity_repo.find_entity_by_alias("b1", "张三").await.unwrap();
        let e2 = entity_repo.find_entity_by_alias("b1", "李四").await.unwrap();
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
        let entity = entity_repo.find_entity_by_alias("b1", "张三").await.unwrap().unwrap();
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
        assert_eq!(accepted.len(), 3, "Phase 1: entity_introduction, alias, property_update accepted");
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
        let judge = mock_accept_judge();  // mock returns normalized_group = "friendship"

        process_chapter("b1", 1, raw_text, &pool, &extractor, None, Some(&judge))
            .await
            .unwrap();

        // Verify relationships_in_chapter via repo (same as ChapterMemoryView uses)
        let rel_repo = RelationshipRepo::new(pool.clone());
        let chapter_rels = rel_repo.list_relationships_by_chapter("b1", 1).await.unwrap();
        assert_eq!(chapter_rels.len(), 1, "should have 1 relationship event in chapter 1");

        let (event, relationship) = &chapter_rels[0];
        // Mock judge normalizes to "friendship"
        assert_eq!(event.relation_group, "friendship");
        assert_eq!(relationship.relation_group, "friendship");
        assert!(!relationship.subject_character_id.is_empty());
        assert!(!relationship.object_character_id.is_empty());

        // Chapter 2 should have no relationship events
        let ch2_rels = rel_repo.list_relationships_by_chapter("b1", 2).await.unwrap();
        assert_eq!(ch2_rels.len(), 0, "chapter 2 should have no relationship events");
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

        let chapter_rels = rel_repo.list_relationships_by_chapter("b1", 1).await.unwrap();
        assert_eq!(chapter_rels.len(), 2, "should have 2 relationship events in chapter 1");
    }

    // --- Real AI Smoke Test (gated by RUN_REAL_AI_TESTS=1) ---

    /// Helper: create AiModelService with config from env vars for smoke test.
    async fn create_smoke_test_ai_service() -> anyhow::Result<(
        std::sync::Arc<crate::service::ai_model_service::AiModelService>,
        std::path::PathBuf,
    )> {
        use crate::service::ai_model_service::AiModelService;
        use crate::service::json_document_service::JsonDocumentService;
        use crate::model::ai_model::{AiModelConfig, AiModelEndpointConfig};

        let base_url = std::env::var("AI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com".to_string());
        let api_key = std::env::var("AI_API_KEY")
            .expect("AI_API_KEY must be set for real AI smoke test");
        let model = std::env::var("AI_MODEL")
            .unwrap_or_else(|_| "gpt-4o-mini".to_string());

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
        service.save(config).await.map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok((std::sync::Arc::new(service), dir))
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
            (1i64, "张三是一个年轻的修士，他从小在青云门长大。他的师父是李真人，一位金丹期的高手。李真人视张三如己出，倾囊相授。张三还有一个师妹叫小红，两人从小一起长大，情同手足。"),
            (2, "张三和李真人一起下山历练。途中遇到了王五，一个神秘的散修。王五和李真人曾经是同门师兄弟，后来因为一本秘籍反目成仇，从此势不两立。王五发誓要找李真人报仇。"),
            (3, "张三突破到了筑基期。小红为他高兴，两人约定一起闯荡江湖。李真人对张三的进步很满意，决定将掌门之位传给他。王五暗中观察，等待报仇的机会。"),
        ];

        for (idx, text) in &chapters {
            let mut last_err = None;
            for attempt in 0..3 {
                if attempt > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
                match process_chapter("b1", *idx, text, &pool, &extractor, None, Some(&judge)).await {
                    Ok(_) => { last_err = None; break; }
                    Err(e) => {
                        eprintln!("[SMOKE] Chapter {} attempt {} failed: {}", idx, attempt + 1, e);
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
            println!("[SMOKE] Judge ai_run {}: model={}, status={}", id, model, status);
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
            println!("  - [{}] {} (importance: {})", e.entity_type, e.canonical_name, e.importance_score);
        }
        println!("Relationship update claims: {}", total_relationship_updates);
        println!("  accepted: {}", accepted);
        println!("  rejected: {}", rejected);
        println!("  uncertain: {}", uncertain);
        println!("  redirected: {}", redirected);
        println!("  other: {}", other_status);
        println!("Active relationships: {}", active_count);
        for r in &rels {
            println!("  - {} -> {} [{}] {} (strength: {}, polarity: {})",
                r.subject_character_id, r.object_character_id,
                r.relation_group, r.relation_label, r.strength, r.polarity);
        }

        // Basic assertions
        assert!(!entities.is_empty(), "Real AI should extract at least some entities");

        // Phase 1 character card should still work
        if let Some(entity) = entities.first() {
            let card = crate::service::v4::projection::project_character_card(
                &entity.id, "b1", 3, &pool,
            ).await;
            assert!(card.is_ok(), "Character card projection should work after real AI extraction");
        }
    }
}
