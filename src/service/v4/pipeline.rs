use crate::service::v4::claim_writer;
use crate::service::v4::context_builder::ContextBuilder;
use crate::service::v4::extractor::Extractor;
use crate::service::v4::projection;
use crate::service::v4::reducer;
use crate::service::v4::resolver;
use crate::storage::db::v4::ai_run_repo::AiRunRepo;
use crate::storage::db::v4::chapter_repo::{self, ChapterRepo};
use crate::storage::db::v4::claim_repo::ClaimRepo;
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::progress_repo::ProgressRepo;
use sqlx::SqlitePool;

const PROMPT_VERSION: &str = "v1";
const SCHEMA_VERSION: i64 = 1;
const DEFAULT_MODEL: &str = "unknown";

/// Process a single chapter through the full V4 pipeline.
///
/// Flow:
/// 1. Ensure chapter exists in DB
/// 2. Check idempotency (already processed with same params)
/// 3. Create chapter_processing_run
/// 4. Segment chapter text
/// 5. Upsert segments + source_spans
/// 6. For each segment: context → extract → resolve → claim_writer → reducer
/// 7. Invalidate + rebuild projection cache
/// 8. Update chapter_processing_runs to success
/// 9. Update processing_progress.max_processed_chapter
pub async fn process_chapter(
    book_id: &str,
    chapter_index: i64,
    raw_text: &str,
    pool: &SqlitePool,
    extractor: &dyn Extractor,
    model_name: Option<&str>,
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

    // 6. For each segment: context → extract → resolve → claim_writer → reducer
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

        // f. Reduce claims into canonical state
        let _reduction_result = reducer::reduce_claims(
            &claim_result.claims_created,
            book_id,
            pool,
        )
        .await?;

        // g. Mark AI run as success with output_json
        ai_run_repo
            .update_run_status(&ai_run.id, "success", output_json.as_deref(), None)
            .await?;
    }

    // 7. Invalidate + rebuild projection cache
    projection::invalidate_book_cache(book_id, pool).await?;

    // Force rebuild by projecting (populates cache)
    let _ = projection::project_character_list(book_id, chapter_index, pool).await;

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

    #[tokio::test]
    async fn process_chapter_creates_source_spans_and_ai_runs() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三走进了大殿，看到了李四。两人互相行礼。";
        let extractor = empty_extractor();

        process_chapter("b1", 1, raw_text, &pool, &extractor, None).await.unwrap();

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
        process_chapter("b1", 1, raw_text, &pool, &extractor, None).await.unwrap();

        // Get chapter to count runs
        let chapter_repo = ChapterRepo::new(pool.clone());
        let chapter = chapter_repo.get_chapter("b1", 1).await.unwrap().unwrap();
        let ai_run_repo = AiRunRepo::new(pool.clone());
        let runs_before = ai_run_repo.list_runs_by_chapter("b1", &chapter.id).await.unwrap();

        // Second run (same text → same hash → should skip)
        process_chapter("b1", 1, raw_text, &pool, &extractor, None).await.unwrap();
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

        process_chapter("b1", 1, "第一章内容。", &pool, &extractor, None)
            .await
            .unwrap();

        let progress = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(progress.max_processed_chapter, 1);

        process_chapter("b1", 2, "第二章内容。", &pool, &extractor, None)
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

        process_chapter("b1", 1, "测试内容。", &pool, &extractor, None)
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
        let result = process_chapter("b1", 1, "", &pool, &extractor, None).await;
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

        process_chapter("b1", 1, raw_text, &pool, &extractor, None)
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
        let accepted: Vec<&crate::storage::db::v4::claim_repo::ClaimRecord> = claims
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

        process_chapter("b1", 1, raw_text, &pool, &extractor, None)
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
}
