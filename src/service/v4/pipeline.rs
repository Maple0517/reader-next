use crate::service::v4::claim_writer;
use crate::service::v4::context_builder::ContextBuilder;
use crate::service::v4::extractor::Observation;
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
const MODEL: &str = "placeholder";

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
) -> anyhow::Result<()> {
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

    // Mark old segments/spans stale if they exist (hash changed)
    chapter_repo.mark_segments_stale(&chapter.id).await?;
    chapter_repo.mark_spans_stale(&chapter.id).await?;

    // 5. Upsert segments + source_spans, collect (segment_id, segment_text, span_ids) for processing
    let mut segment_info: Vec<(String, String, Vec<String>)> = Vec::new();

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
                MODEL,
                PROMPT_VERSION,
                SCHEMA_VERSION,
                &input_hash,
            )
            .await?;

        // b. Build context
        let context = context_builder
            .build_context(book_id, chapter_index, segment_text)
            .await?;

        // c. Extract observations (placeholder - returns empty for now)
        let observations = extract_observations(&context, span_ids);

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

        // g. Mark AI run as success
        ai_run_repo
            .update_run_status(&ai_run.id, "success", None, None)
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

/// Placeholder extractor: returns empty observations for now.
/// TODO: Integrate actual AI extraction when AI service is available.
fn extract_observations(context: &str, span_ids: &[String]) -> Vec<Observation> {
    let _ = context; // Used for future AI call
    let _ = span_ids; // Will be referenced in observations
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("v4-pipeline-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        db::v4::init_v4(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn process_chapter_creates_source_spans_and_ai_runs() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "张三走进了大殿，看到了李四。两人互相行礼。";

        process_chapter("b1", 1, raw_text, &pool).await.unwrap();

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
    }

    #[tokio::test]
    async fn process_chapter_idempotent_skip() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        let raw_text = "第一章内容。";

        // First run
        process_chapter("b1", 1, raw_text, &pool).await.unwrap();

        // Get chapter to count runs
        let chapter_repo = ChapterRepo::new(pool.clone());
        let chapter = chapter_repo.get_chapter("b1", 1).await.unwrap().unwrap();
        let ai_run_repo = AiRunRepo::new(pool.clone());
        let runs_before = ai_run_repo.list_runs_by_chapter("b1", &chapter.id).await.unwrap();

        // Second run (same text → same hash → should skip)
        process_chapter("b1", 1, raw_text, &pool).await.unwrap();
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

        process_chapter("b1", 1, "第一章内容。", &pool)
            .await
            .unwrap();

        let progress = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(progress.max_processed_chapter, 1);

        process_chapter("b1", 2, "第二章内容。", &pool)
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

        process_chapter("b1", 1, "测试内容。", &pool)
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

        // Empty text should still succeed (produces 1 segment with empty span)
        let result = process_chapter("b1", 1, "", &pool).await;
        assert!(result.is_ok(), "empty text should succeed");

        let progress = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(progress.max_processed_chapter, 1);
    }
}
