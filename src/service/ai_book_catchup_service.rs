use crate::error::error::AppError;
use crate::model::ai_book::{AiBookChapterDigestV3, AiBookMemoryV3};
use crate::model::ai_book_catchup::{
    AiBookCatchupTaskStats, AiBookCatchupTaskStatus, AiBookCatchupTaskView,
};
use crate::model::ai_book_generation::{AiBookChapterDigestCandidateV3, AiBookKnowledgePatchV3};
#[cfg(test)]
use crate::model::ai_model::ResolvedAiModelEndpoint;
#[cfg(test)]
use crate::model::ai_proxy::build_ai_proxy_url;
use crate::service::ai_book_generation_service::AiBookWriteGuard;
use crate::service::ai_book_memory_v3::{
    merge_ai_book_memory_v3, normalize_knowledge_patch_v3, select_working_context_v3,
    sync_processed_chapter_from_digests, AiBookWorkingContextV3,
};
use crate::util::time::now_ts;
use futures::future::BoxFuture;
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[cfg(test)]
const DEFAULT_PROMPT: &str = r#"你是小说 AI资料后台补齐 agent。只允许基于当前已读章节和本次章节正文更新资料，不预测未读内容，不剧透目标章节之后内容。
输入会给你 currentMemory 和 chapter。不要输出 Markdown，不要输出解释，只输出严格 JSON 对象。
优先输出 {"memory": <只包含本章新增/更新字段的 AI memory 增量 JSON>}；不要回传未变化的大数组，后端会与 currentMemory 合并。
如果 currentMemory.schemaVersion 是 2，必须保留 V2 结构：summary.current/recentChanges/openQuestions、chapterDigests、worldFacts、characters、relationships、locations、mapState、renderArtifacts。
V2 中 worldFacts、characters、relationships、locations 里的每个有效条目都必须带 evidence 数组；evidence 至少 1 条，至少包含 chapterIndex、chapterTitle、note，quote 仅在必要时填写。
worldFacts 必须使用 category/title/content/confidence/importance；category 只能从 基础规则、势力制度、历史传说、技术/魔法、社会文化、地理环境、组织体系、未确认信息 中选择，禁止留空。
characters 使用 name/currentStatus/faction/locationName/description；relationships 使用 sourceName/targetName/targetKind/relationType/direction/currentStatus/description。
locations 必须使用 name/kind/scale/parentName/description/currentStatus；parentName 表示层级归属，例如 昆墟 > 昆墟第一层 > 嵩阳高中 > 学校食堂；无法确认父级才留空。
不要为了凑数输出 importance=low 的空洞条目；无法确认的信息标为“推断”或“未知”。
每次必须推进 processedChapterIndex/processedChapterTitle，并保留或更新已有有效资料。"#;

type SaveMemoryFn = Arc<dyn Fn(Value) -> BoxFuture<'static, Result<Value, AppError>> + Send + Sync>;
type FetchContentFn =
    Arc<dyn Fn(CatchupChapter) -> BoxFuture<'static, Result<String, AppError>> + Send + Sync>;
type GenerateDigestFn = Arc<
    dyn Fn(
            AiBookWorkingContextV3,
            CatchupChapter,
            String,
        ) -> BoxFuture<'static, Result<AiBookChapterDigestCandidateV3, AppError>>
        + Send
        + Sync,
>;
type GeneratePatchFn = Arc<
    dyn Fn(
            AiBookWorkingContextV3,
            CatchupChapter,
            String,
            AiBookChapterDigestCandidateV3,
        ) -> BoxFuture<'static, Result<AiBookKnowledgePatchV3, AppError>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct AiBookCatchupService {
    tasks: Arc<RwLock<HashMap<String, TaskState>>>,
    runner: Arc<dyn CatchupRunner>,
}

#[derive(Debug, Clone)]
struct TaskState {
    view: AiBookCatchupTaskView,
    cancel_requested: bool,
}

impl TaskState {
    fn new(user_ns: &str, book_url: &str, target_chapter_index: Option<i32>) -> Self {
        Self {
            view: AiBookCatchupTaskView {
                user_ns: user_ns.to_string(),
                book_url: book_url.to_string(),
                status: AiBookCatchupTaskStatus::Running.as_str().to_string(),
                start_chapter_index: None,
                target_chapter_index,
                total_chapters: 0,
                completed_chapters: 0,
                current_chapter_index: None,
                current_chapter_title: None,
                processed_chapter_index: None,
                processed_chapter_title: None,
                error: None,
                updated_at: now_ts() * 1000,
                current_stage: Some("fetching".to_string()),
                stats: Some(AiBookCatchupTaskStats::default()),
            },
            cancel_requested: false,
        }
    }

    fn snapshot(&self) -> AiBookCatchupTaskView {
        self.view.clone()
    }
}

#[derive(Debug, Clone)]
pub struct CatchupChapter {
    pub title: String,
    pub chapter_url: String,
    pub index: i32,
}

pub struct CatchupBookContext {
    pub chapters: Vec<CatchupChapter>,
    pub memory: Value,
    pub write_guard: Option<AiBookWriteGuard>,
    pub save_memory: SaveMemoryFn,
    pub fetch_content: FetchContentFn,
    pub generate_digest: GenerateDigestFn,
    pub generate_patch: GeneratePatchFn,
}

pub fn save_memory_fn<F, Fut>(f: F) -> SaveMemoryFn
where
    F: Fn(Value) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<Value, AppError>> + Send + 'static,
{
    Arc::new(move |memory| Box::pin(f(memory)))
}

pub fn fetch_content_fn<F, Fut>(f: F) -> FetchContentFn
where
    F: Fn(CatchupChapter) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<String, AppError>> + Send + 'static,
{
    Arc::new(move |chapter| Box::pin(f(chapter)))
}

pub fn generate_digest_fn<F, Fut>(f: F) -> GenerateDigestFn
where
    F: Fn(AiBookWorkingContextV3, CatchupChapter, String) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<AiBookChapterDigestCandidateV3, AppError>>
        + Send
        + 'static,
{
    Arc::new(move |memory, chapter, content| Box::pin(f(memory, chapter, content)))
}

pub fn generate_patch_fn<F, Fut>(f: F) -> GeneratePatchFn
where
    F: Fn(AiBookWorkingContextV3, CatchupChapter, String, AiBookChapterDigestCandidateV3) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: std::future::Future<Output = Result<AiBookKnowledgePatchV3, AppError>> + Send + 'static,
{
    Arc::new(move |memory, chapter, content, digest| Box::pin(f(memory, chapter, content, digest)))
}

pub trait CatchupRunner: Send + Sync {
    fn spawn_task(&self, fut: BoxFuture<'static, ()>);
}

#[derive(Clone, Default)]
struct TokioCatchupRunner;

impl CatchupRunner for TokioCatchupRunner {
    fn spawn_task(&self, fut: BoxFuture<'static, ()>) {
        tokio::spawn(fut);
    }
}

impl Default for AiBookCatchupService {
    fn default() -> Self {
        Self::new()
    }
}

impl AiBookCatchupService {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            runner: Arc::new(TokioCatchupRunner),
        }
    }

    #[cfg(test)]
    fn new_with_runner(runner: Arc<dyn CatchupRunner>) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            runner,
        }
    }

    pub async fn start_with<F, Fut>(
        &self,
        user_ns: String,
        book_url: String,
        target_chapter_index: Option<i32>,
        build_context: F,
    ) -> Result<AiBookCatchupTaskView, AppError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<CatchupBookContext, AppError>> + Send + 'static,
    {
        let key = task_key(&user_ns, &book_url);
        let mut tasks = self.tasks.write().await;
        if let Some(existing) = tasks.get(&key) {
            if matches_status(&existing.view.status, &["running", "canceling"]) {
                return Ok(existing.snapshot());
            }
        }
        let state = TaskState::new(&user_ns, &book_url, target_chapter_index);
        let view = state.snapshot();
        tasks.insert(key.clone(), state);
        drop(tasks);

        let service = self.clone();
        self.runner.spawn_task(Box::pin(async move {
            match build_context().await {
                Ok(context) => {
                    service
                        .run_task(user_ns, book_url, context, target_chapter_index)
                        .await
                }
                Err(err) => service.mark_failed(&key, err.to_string()).await,
            }
        }));
        Ok(view)
    }

    pub async fn get_status(&self, user_ns: &str, book_url: &str) -> Option<AiBookCatchupTaskView> {
        self.tasks
            .read()
            .await
            .get(&task_key(user_ns, book_url))
            .map(TaskState::snapshot)
    }

    pub async fn request_cancel(
        &self,
        user_ns: &str,
        book_url: &str,
    ) -> Result<AiBookCatchupTaskView, AppError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(&task_key(user_ns, book_url))
            .ok_or_else(|| AppError::BadRequest("任务不存在".to_string()))?;
        if matches_status(&task.view.status, &["completed", "failed", "canceled"]) {
            return Ok(task.snapshot());
        }
        task.cancel_requested = true;
        task.view.status = AiBookCatchupTaskStatus::Canceling.as_str().to_string();
        task.view.updated_at = now_ts() * 1000;
        Ok(task.snapshot())
    }

    async fn run_task(
        &self,
        user_ns: String,
        book_url: String,
        mut context: CatchupBookContext,
        requested_target: Option<i32>,
    ) {
        let _write_guard = context.write_guard.take();
        let key = task_key(&user_ns, &book_url);
        let start_index = next_catchup_start_index(&context.memory);
        let max_chapter_index = context.chapters.iter().map(|chapter| chapter.index).max();
        let target_index = requested_target
            .or(max_chapter_index)
            .unwrap_or(start_index - 1);
        let chapters = std::mem::take(&mut context.chapters)
            .into_iter()
            .filter(|chapter| chapter.index >= start_index && chapter.index <= target_index)
            .collect::<Vec<_>>();
        self.set_plan(&key, start_index, target_index, chapters.len() as i32)
            .await;

        if chapters.is_empty() {
            self.mark_completed(&key).await;
            return;
        }

        for chapter in chapters {
            if self.cancel_should_stop(&key).await {
                self.mark_canceled(&key).await;
                return;
            }
            self.set_current_chapter(&key, &chapter).await;
            let content_result = self
                .run_stage(&key, "fetching", (context.fetch_content)(chapter.clone()))
                .await;
            let content = match content_result {
                Ok(content) => content,
                Err(StageError::App(err)) => {
                    self.save_failure_memory(&context, &chapter, &err.to_string())
                        .await;
                    self.mark_failed(&key, err.to_string()).await;
                    return;
                }
                Err(StageError::Canceled) => {
                    self.mark_canceled(&key).await;
                    return;
                }
            };
            self.set_stage(&key, "digest").await;
            let memory_result = self
                .process_chapter(&key, &context.memory, &chapter, &content, &context)
                .await;
            let next_memory = match memory_result {
                Ok(memory) => memory,
                Err(StageError::App(err)) => {
                    self.save_failure_memory(&context, &chapter, &err.to_string())
                        .await;
                    self.mark_failed(&key, err.to_string()).await;
                    return;
                }
                Err(StageError::Canceled) => {
                    self.mark_canceled(&key).await;
                    return;
                }
            };

            match (context.save_memory)(next_memory).await {
                Ok(saved) => {
                    context.memory = saved;
                    self.mark_processed(&key, &chapter).await;
                    if self.cancel_should_stop(&key).await {
                        self.mark_canceled(&key).await;
                        return;
                    }
                }
                Err(err) => {
                    self.mark_failed(&key, err.to_string()).await;
                    return;
                }
            }
        }
        self.mark_completed(&key).await;
    }

    async fn save_failure_memory(
        &self,
        context: &CatchupBookContext,
        chapter: &CatchupChapter,
        error: &str,
    ) {
        let mut memory = context.memory.clone();
        mark_memory_failed(&mut memory, chapter, error);
        let _ = (context.save_memory)(memory).await;
    }

    async fn process_chapter(
        &self,
        key: &str,
        memory: &Value,
        chapter: &CatchupChapter,
        chapter_content: &str,
        context: &CatchupBookContext,
    ) -> Result<Value, StageError> {
        let mut memory_v3 =
            serde_json::from_value::<AiBookMemoryV3>(memory.clone()).map_err(|e| {
                StageError::App(AppError::BadRequest(format!(
                    "AI资料补齐内存格式不正确: {e}"
                )))
            })?;
        let digest_working_context = select_working_context_v3(&memory_v3, None, chapter_content);
        let digest_started = Instant::now();
        let mut digest = (context.generate_digest)(
            digest_working_context,
            chapter.clone(),
            chapter_content.to_string(),
        )
        .await
        .map_err(StageError::App)?;
        digest.chapter_index = chapter.index;
        digest.chapter_title = chapter.title.clone();
        self.bump_stats(key, |stats| {
            stats.total_model_calls += 1;
            stats.digest_calls += 1;
            record_stats(stats, chapter.index, digest_started.elapsed());
        })
        .await;
        upsert_digest_v3(&mut memory_v3, &digest);

        if self.cancel_should_stop(key).await {
            return Err(StageError::Canceled);
        }

        let should_patch = digest_requires_patch(&digest, chapter_content, &memory_v3);
        if should_patch {
            self.set_stage(key, "patch").await;
            let patch_working_context = select_working_context_v3(
                &memory_v3,
                Some(&digest_to_view(&digest)),
                chapter_content,
            );
            let patch_started = Instant::now();
            let mut patch = (context.generate_patch)(
                patch_working_context.clone(),
                chapter.clone(),
                chapter_content.to_string(),
                digest.clone(),
            )
            .await
            .map_err(StageError::App)?;
            patch.chapter_index = chapter.index;
            if patch
                .summary
                .as_deref()
                .is_none_or(|summary| summary.trim().is_empty())
            {
                patch.summary = Some(digest.summary.clone());
            }
            let working_context = patch_working_context;
            let normalized = normalize_knowledge_patch_v3(patch, &working_context);
            memory_v3 = merge_ai_book_memory_v3(memory_v3, normalized);
            self.bump_stats(key, |stats| {
                stats.total_model_calls += 1;
                stats.patch_calls += 1;
                record_stats(stats, chapter.index, patch_started.elapsed());
            })
            .await;
        } else {
            self.bump_stats(key, |stats| {
                stats.skipped_patch_chapters += 1;
                stats.updated_at = now_ts();
                stats.last_chapter_index = Some(chapter.index);
            })
            .await;
        }
        sync_processed_chapter_from_digests(&mut memory_v3);
        memory_v3.last_error = None;
        memory_v3.last_error_chapter_index = None;
        memory_v3.last_error_chapter_title = None;
        memory_v3.catchup_stats = self.current_stats(key).await;
        serde_json::to_value(memory_v3)
            .map_err(|e| StageError::App(AppError::BadRequest(e.to_string())))
    }

    async fn set_plan(&self, key: &str, start: i32, target: i32, total: i32) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(key) {
            task.view.start_chapter_index = Some(start);
            task.view.target_chapter_index = Some(target);
            task.view.total_chapters = total.max(0);
            task.view.completed_chapters = 0;
            task.view.updated_at = now_ts() * 1000;
        }
    }

    async fn run_stage<T, F>(&self, key: &str, stage: &str, future: F) -> Result<T, StageError>
    where
        F: Future<Output = Result<T, AppError>>,
    {
        self.set_stage(key, stage).await;
        let deadline = tokio::time::sleep(stage_timeout(stage));
        tokio::pin!(future);
        tokio::pin!(deadline);
        loop {
            if self.cancel_should_stop(key).await {
                return Err(StageError::Canceled);
            }
            tokio::select! {
                result = &mut future => {
                    let result = result.map_err(StageError::App);
                    if self.cancel_should_stop(key).await {
                        return Err(StageError::Canceled);
                    }
                    return result;
                }
                _ = &mut deadline => {
                    return Err(StageError::App(AppError::BadRequest(format!(
                        "AI资料补齐阶段超时: {}",
                        stage_label(stage),
                    ))));
                }
                _ = tokio::time::sleep(Duration::from_millis(200)) => {}
            }
        }
    }

    async fn set_stage(&self, key: &str, stage: &str) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(key) {
            task.view.current_stage = Some(stage.to_string());
            task.view.updated_at = now_ts() * 1000;
        }
    }

    async fn set_current_chapter(&self, key: &str, chapter: &CatchupChapter) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(key) {
            if !task.cancel_requested {
                task.view.status = AiBookCatchupTaskStatus::Running.as_str().to_string();
            }
            task.view.current_chapter_index = Some(chapter.index);
            task.view.current_chapter_title = Some(chapter.title.clone());
            task.view.updated_at = now_ts() * 1000;
        }
    }

    async fn mark_processed(&self, key: &str, chapter: &CatchupChapter) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(key) {
            task.view.processed_chapter_index = Some(chapter.index);
            task.view.processed_chapter_title = Some(chapter.title.clone());
            task.view.completed_chapters = task
                .view
                .start_chapter_index
                .map(|start| chapter.index.saturating_sub(start) + 1)
                .unwrap_or(task.view.completed_chapters + 1)
                .max(task.view.completed_chapters)
                .min(task.view.total_chapters.max(0));
            task.view.error = None;
            task.view.updated_at = now_ts() * 1000;
        }
    }

    async fn mark_completed(&self, key: &str) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(key) {
            task.view.status = AiBookCatchupTaskStatus::Completed.as_str().to_string();
            task.view.completed_chapters = task.view.total_chapters;
            task.view.current_chapter_index = None;
            task.view.current_chapter_title = None;
            task.view.updated_at = now_ts() * 1000;
        }
    }

    async fn mark_canceled(&self, key: &str) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(key) {
            task.view.status = AiBookCatchupTaskStatus::Canceled.as_str().to_string();
            task.view.current_stage = None;
            task.view.current_chapter_index = None;
            task.view.current_chapter_title = None;
            task.view.updated_at = now_ts() * 1000;
        }
    }

    async fn mark_failed(&self, key: &str, error: String) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(key) {
            task.view.status = AiBookCatchupTaskStatus::Failed.as_str().to_string();
            task.view.error = Some(error);
            task.view.updated_at = now_ts() * 1000;
        }
    }

    async fn cancel_should_stop(&self, key: &str) -> bool {
        self.tasks
            .read()
            .await
            .get(key)
            .map(|task| task.cancel_requested)
            .unwrap_or(false)
    }

    async fn bump_stats<F>(&self, key: &str, update: F)
    where
        F: FnOnce(&mut AiBookCatchupTaskStats),
    {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(key) {
            let stats = task
                .view
                .stats
                .get_or_insert_with(AiBookCatchupTaskStats::default);
            update(stats);
            task.view.updated_at = now_ts() * 1000;
        }
    }

    async fn current_stats(&self, key: &str) -> Option<AiBookCatchupTaskStats> {
        self.tasks
            .read()
            .await
            .get(key)
            .and_then(|task| task.view.stats.clone())
    }
}

fn task_key(user_ns: &str, book_url: &str) -> String {
    format!("{user_ns}::{book_url}")
}

fn stage_timeout(stage: &str) -> Duration {
    match stage {
        "fetching" => Duration::from_secs(45),
        _ => Duration::from_secs(180),
    }
}

fn stage_label(stage: &str) -> &str {
    match stage {
        "fetching" => "获取章节正文",
        "digest" => "生成章节摘要",
        "patch" => "更新AI资料",
        _ => stage,
    }
}

fn matches_status(current: &str, candidates: &[&str]) -> bool {
    candidates.iter().any(|item| current == *item)
}

#[derive(Debug)]
enum StageError {
    Canceled,
    App(AppError),
}

fn record_stats(
    stats: &mut AiBookCatchupTaskStats,
    chapter_index: i32,
    elapsed: std::time::Duration,
) {
    let latency_ms = elapsed.as_millis() as i64;
    stats.last_call_latency_ms = Some(latency_ms);
    stats.last_chapter_index = Some(chapter_index);
    stats.updated_at = now_ts();
    stats.average_call_latency_ms = Some(match stats.average_call_latency_ms {
        Some(avg) if stats.total_model_calls > 0 => {
            ((avg * (stats.total_model_calls as i64 - 1)) + latency_ms)
                / stats.total_model_calls as i64
        }
        _ => latency_ms,
    });
}

fn digest_to_view(digest: &AiBookChapterDigestCandidateV3) -> AiBookChapterDigestV3 {
    AiBookChapterDigestV3 {
        chapter_index: digest.chapter_index,
        chapter_title: digest.chapter_title.clone(),
        summary: digest.summary.clone(),
        key_points: digest.key_points.clone(),
        characters: Vec::new(),
        character_states: Vec::new(),
        character_relations: Vec::new(),
        knowledge_facts: Vec::new(),
        locations: Vec::new(),
        location_edges: Vec::new(),
    }
}

fn upsert_digest_v3(memory: &mut AiBookMemoryV3, digest: &AiBookChapterDigestCandidateV3) {
    let digest_v3 = digest_to_view(digest);
    if let Some(existing) = memory
        .chapter_digests
        .iter_mut()
        .find(|item| item.chapter_index == digest.chapter_index)
    {
        *existing = digest_v3;
    } else {
        memory.chapter_digests.push(digest_v3);
        memory
            .chapter_digests
            .sort_by_key(|item| item.chapter_index);
    }
}

fn digest_requires_patch(
    digest: &AiBookChapterDigestCandidateV3,
    chapter_text: &str,
    memory: &AiBookMemoryV3,
) -> bool {
    if digest.has_important_changes {
        return true;
    }
    if chapter_text.trim().is_empty() {
        return false;
    }
    if digest.summary.trim().is_empty() {
        return true;
    }
    let important = digest
        .key_points
        .iter()
        .any(|item| contains_patch_trigger(item))
        || contains_patch_trigger(&digest.summary);
    if important {
        return true;
    }
    backend_patch_guard(chapter_text, memory)
}

fn backend_patch_guard(chapter_text: &str, memory: &AiBookMemoryV3) -> bool {
    let text = chapter_text.trim();
    if text.is_empty() {
        return false;
    }
    if text.chars().any(|ch| ch == '→' || ch == '→') {
        return true;
    }
    if [
        "加入", "背叛", "救下", "欠", "债", "师父", "同盟", "冲突", "能力", "突破", "搬到", "来到",
        "离开",
    ]
    .iter()
    .any(|keyword| text.contains(keyword))
    {
        return true;
    }
    text.split(|ch: char| ch.is_whitespace() || "，。！？；：、“”‘’（）()《》<>-".contains(ch))
        .filter(|token| token.chars().count() >= 2)
        .any(|token| {
            let known_character = memory
                .characters
                .iter()
                .any(|item| item.name == token || item.aliases.iter().any(|alias| alias == token));
            let known_location = memory.locations.iter().any(|item| item.name == token);
            !known_character && !known_location && token.chars().all(|ch| !ch.is_ascii_digit())
        })
}

fn contains_patch_trigger(text: &str) -> bool {
    [
        "关系", "势力", "地点", "设定", "能力", "突破", "冲突", "身份", "加入", "离开",
    ]
    .iter()
    .any(|keyword| text.contains(keyword))
}

fn read_i32(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map(|value| value as i32)
}

pub fn next_catchup_start_index(memory: &Value) -> i32 {
    let present = memory
        .get("chapterDigests")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| digest_has_content(item))
        .filter_map(|item| read_i32(item, "chapterIndex"))
        .collect::<HashSet<_>>();
    let mut index = 0;
    while present.contains(&index) {
        index += 1;
    }
    index
}

fn digest_has_content(item: &Value) -> bool {
    item.get("summary")
        .and_then(Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
        || item
            .get("keyPoints")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
}

#[cfg(test)]
fn build_target_url(endpoint: &ResolvedAiModelEndpoint) -> Result<reqwest::Url, AppError> {
    build_ai_proxy_url(&endpoint.base_url, &endpoint.path, endpoint.use_full_url)
        .map_err(AppError::BadRequest)
}

#[cfg(test)]
fn build_model_body(path: &str, model: &str, prompt: String) -> Value {
    if is_gemini_path(path) {
        return json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": format!("{}\n\n{}", DEFAULT_PROMPT, prompt) }]
            }],
            "generationConfig": {
                "temperature": 0.2,
                "maxOutputTokens": 8192,
                "responseMimeType": "application/json"
            }
        });
    }
    if is_anthropic_path(path) {
        return json!({
            "model": model,
            "max_tokens": 8192,
            "temperature": 0.2,
            "system": DEFAULT_PROMPT,
            "messages": [{ "role": "user", "content": prompt }]
        });
    }
    if is_responses_path(path) {
        return json!({
            "model": model,
            "temperature": 0.2,
            "max_output_tokens": 8192,
            "stream": false,
            "text": { "format": { "type": "json_object" } },
            "input": [
                { "role": "system", "content": DEFAULT_PROMPT },
                { "role": "user", "content": prompt }
            ]
        });
    }
    json!({
        "model": model,
        "temperature": 0.2,
        "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": DEFAULT_PROMPT },
            { "role": "user", "content": prompt }
        ]
    })
}

#[cfg(test)]
fn is_gemini_path(path: &str) -> bool {
    path.contains("generateContent")
}

#[cfg(test)]
fn is_anthropic_path(path: &str) -> bool {
    path.contains("/v1/messages") || path.ends_with("/messages")
}

#[cfg(test)]
fn is_responses_path(path: &str) -> bool {
    path.contains("/v1/responses") || path.ends_with("/responses")
}

#[cfg(test)]
fn extract_model_content(path: &str, value: &Value) -> Result<String, AppError> {
    if is_gemini_path(path) {
        let text = value
            .get("candidates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|candidate| {
                candidate
                    .pointer("/content/parts")
                    .and_then(Value::as_array)
            })
            .flatten()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return Ok(text);
        }
    }
    if is_anthropic_path(path) {
        let text = value
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return Ok(text);
        }
    }
    if is_responses_path(path) {
        if let Some(text) = value.get("output_text").and_then(Value::as_str) {
            let text = text.trim();
            if !text.is_empty() {
                return Ok(text.to_string());
            }
        }
        let text = value
            .get("output")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("content").and_then(Value::as_array))
            .flatten()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return Ok(text);
        }
    }
    value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::BadRequest("AI资料补齐返回内容为空".to_string()))
}

#[cfg(test)]
fn parse_memory_update(
    text: &str,
    current: &Value,
    book_url: &str,
    book_name: &str,
    author: &str,
    chapter: &CatchupChapter,
) -> Result<Value, AppError> {
    let parsed = parse_json_content(text)?;
    let candidate = parsed.get("memory").cloned().unwrap_or(parsed);
    let mut next = merge_patch(current.clone(), candidate, chapter);
    normalize_memory(&mut next, book_url, book_name, author, chapter)?;
    if !has_semantic_content(&next) {
        return Err(AppError::BadRequest("AI资料补齐返回内容为空".to_string()));
    }
    Ok(next)
}

#[cfg(test)]
fn parse_json_content(text: &str) -> Result<Value, AppError> {
    let trimmed = text.trim();
    let json_text = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };
    serde_json::from_str::<Value>(json_text)
        .or_else(|_| {
            extract_first_json_object(json_text)
                .ok_or(())
                .and_then(|json| serde_json::from_str::<Value>(json).map_err(|_| ()))
        })
        .map_err(|_| AppError::BadRequest("AI资料补齐返回 JSON 格式不正确".to_string()))
}

#[cfg(test)]
fn extract_first_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in text[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' => {
                if stack.pop()? != ch {
                    return None;
                }
                if stack.is_empty() {
                    let end = start + offset + ch.len_utf8();
                    return Some(&text[start..end]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
fn merge_patch(mut current: Value, patch: Value, chapter: &CatchupChapter) -> Value {
    let Some(object) = current.as_object_mut() else {
        return patch;
    };
    let is_v2 = object.get("schemaVersion").and_then(Value::as_i64) == Some(2);
    if is_v2 {
        merge_v2_patch(object, patch, chapter);
    } else {
        merge_legacy_patch(object, patch);
    }
    current
}

#[cfg(test)]
fn merge_v2_patch(
    object: &mut serde_json::Map<String, Value>,
    patch: Value,
    chapter: &CatchupChapter,
) {
    if let Some(summary) = patch.get("summary") {
        if summary.is_object() {
            merge_summary_object(object, summary);
        } else if let Some(text) = summary.as_str().filter(|text| !text.trim().is_empty()) {
            let summary_object = object.entry("summary").or_insert_with(
                || json!({ "current": "", "recentChanges": [], "openQuestions": [] }),
            );
            if let Some(summary_map) = summary_object.as_object_mut() {
                summary_map.insert("current".to_string(), Value::String(text.to_string()));
            }
        }
    }
    for (source, target) in [
        ("worldFacts", "worldFacts"),
        ("facts", "worldFacts"),
        ("worldview", "worldFacts"),
        ("characters", "characters"),
        ("relationships", "relationships"),
        ("locations", "locations"),
    ] {
        merge_non_empty_array_by_identity(object, &patch, source, target);
    }
    if let Some(map_state) = patch.get("mapState").filter(|value| value.is_object()) {
        merge_object_fields(object, "mapState", map_state);
    }
    if let Some(render_artifacts) = patch
        .get("renderArtifacts")
        .filter(|value| value.is_object())
    {
        merge_object_fields(object, "renderArtifacts", render_artifacts);
    }
    let digest = patch
        .pointer("/chapterDigest/digest")
        .and_then(Value::as_str)
        .or_else(|| patch.get("summary").and_then(Value::as_str))
        .or_else(|| patch.pointer("/summary/current").and_then(Value::as_str))
        .unwrap_or("已补齐本章 AI 资料");
    let digest_entry = json!({
        "chapterIndex": chapter.index,
        "chapterTitle": chapter.title,
        "digest": digest,
        "keyEvents": [],
        "touchedEntityIds": [],
        "createdAt": now_ts() * 1000,
    });
    let digests = object
        .entry("chapterDigests")
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Some(items) = digests.as_array_mut() {
        items.retain(|item| read_i32(item, "chapterIndex") != Some(chapter.index));
        items.push(digest_entry);
    }
}

#[cfg(test)]
fn merge_summary_object(object: &mut serde_json::Map<String, Value>, summary: &Value) {
    let target = object
        .entry("summary")
        .or_insert_with(|| json!({ "current": "", "recentChanges": [], "openQuestions": [] }));
    let Some(target_map) = target.as_object_mut() else {
        object.insert("summary".to_string(), summary.clone());
        return;
    };
    let Some(source_map) = summary.as_object() else {
        return;
    };
    for key in ["current", "recentChanges", "openQuestions"] {
        if let Some(value) = source_map.get(key) {
            let empty_string = value.as_str().map(str::trim).is_some_and(str::is_empty);
            let empty_array = value.as_array().is_some_and(Vec::is_empty);
            if !value.is_null() && !empty_string && !empty_array {
                target_map.insert(key.to_string(), value.clone());
            }
        }
    }
}

#[cfg(test)]
fn merge_non_empty_array_by_identity(
    object: &mut serde_json::Map<String, Value>,
    patch: &Value,
    source: &str,
    target: &str,
) {
    if let Some(items) = patch
        .get(source)
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
    {
        let target_value = object
            .entry(target.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(existing) = target_value.as_array_mut() else {
            object.insert(target.to_string(), Value::Array(items.clone()));
            return;
        };
        for item in items {
            if let Some(identity) = item_identity(target, item) {
                if let Some(old) = existing
                    .iter_mut()
                    .find(|old| item_identity(target, old).as_deref() == Some(identity.as_str()))
                {
                    merge_value_fields(old, item);
                    continue;
                }
            }
            existing.push(item.clone());
        }
    }
}

#[cfg(test)]
fn item_identity(kind: &str, item: &Value) -> Option<String> {
    if kind == "relationships" {
        return relationship_identity(item);
    }
    ["id", "name", "title"]
        .iter()
        .find_map(|key| item.get(*key).and_then(Value::as_str))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
fn relationship_identity(item: &Value) -> Option<String> {
    let source = item
        .get("sourceCharacterId")
        .or_else(|| item.get("sourceName"))
        .or_else(|| item.get("source"))
        .and_then(Value::as_str)?
        .trim();
    let target = item
        .get("targetEntityId")
        .or_else(|| item.get("targetName"))
        .or_else(|| item.get("target"))
        .and_then(Value::as_str)?
        .trim();
    let relation = item
        .get("relationType")
        .or_else(|| item.get("relation"))
        .and_then(Value::as_str)?
        .trim();
    if source.is_empty() || target.is_empty() || relation.is_empty() {
        return None;
    }
    let mut pair = [source, target];
    pair.sort_unstable();
    Some(format!("{}::{}::{}", pair[0], pair[1], relation))
}

#[cfg(test)]
fn merge_value_fields(target: &mut Value, patch: &Value) {
    let (Some(target_map), Some(patch_map)) = (target.as_object_mut(), patch.as_object()) else {
        *target = patch.clone();
        return;
    };
    for (key, value) in patch_map {
        if value.is_null() {
            continue;
        }
        let empty_string = value.as_str().map(str::trim).is_some_and(str::is_empty);
        let empty_array = value.as_array().is_some_and(Vec::is_empty);
        let empty_object = value.as_object().is_some_and(serde_json::Map::is_empty);
        if empty_string || empty_array || empty_object {
            continue;
        }
        if let Some(next_items) = value.as_array() {
            let target_value = target_map
                .entry(key.clone())
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Some(existing_items) = target_value.as_array_mut() {
                for next in next_items {
                    if !existing_items.iter().any(|old| old == next) {
                        existing_items.push(next.clone());
                    }
                }
                continue;
            }
        }
        target_map.insert(key.clone(), value.clone());
    }
}

#[cfg(test)]
fn merge_object_fields(object: &mut serde_json::Map<String, Value>, key: &str, patch: &Value) {
    let target = object.entry(key.to_string()).or_insert_with(|| json!({}));
    let Some(target_map) = target.as_object_mut() else {
        object.insert(key.to_string(), patch.clone());
        return;
    };
    let Some(patch_map) = patch.as_object() else {
        return;
    };
    for (field, value) in patch_map {
        let empty_string = value.as_str().map(str::trim).is_some_and(str::is_empty);
        let empty_array = value.as_array().is_some_and(Vec::is_empty);
        let empty_object = value.as_object().is_some_and(serde_json::Map::is_empty);
        if !value.is_null() && !empty_string && !empty_array && !empty_object {
            target_map.insert(field.clone(), value.clone());
        }
    }
}

#[cfg(test)]
fn merge_legacy_patch(object: &mut serde_json::Map<String, Value>, patch: Value) {
    for key in [
        "summary",
        "worldview",
        "characters",
        "relationships",
        "locations",
    ] {
        if let Some(value) = patch.get(key) {
            object.insert(key.to_string(), value.clone());
        }
    }
}

#[cfg(test)]
fn normalize_memory(
    value: &mut Value,
    book_url: &str,
    book_name: &str,
    author: &str,
    chapter: &CatchupChapter,
) -> Result<(), AppError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| AppError::BadRequest("AI资料补齐结果不是 JSON 对象".to_string()))?;
    object.insert("bookUrl".to_string(), Value::String(book_url.to_string()));
    set_string_if_empty(object, "bookName", book_name);
    set_string_if_empty(object, "author", author);
    object.insert("enabled".to_string(), Value::Bool(true));
    object.insert(
        "processedChapterIndex".to_string(),
        Value::Number(chapter.index.into()),
    );
    object.insert(
        "processedChapterTitle".to_string(),
        Value::String(chapter.title.clone()),
    );
    object.insert(
        "updatedAt".to_string(),
        Value::Number((now_ts() * 1000).into()),
    );
    object.remove("lastError");
    object.remove("lastErrorChapterIndex");
    object.remove("lastErrorChapterTitle");

    if object.get("schemaVersion").and_then(Value::as_i64) == Some(2) {
        ensure_v2_defaults(object);
    }
    Ok(())
}

fn mark_memory_failed(value: &mut Value, chapter: &CatchupChapter, error: &str) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert("lastError".to_string(), Value::String(error.to_string()));
    object.insert(
        "lastErrorChapterIndex".to_string(),
        Value::Number(chapter.index.into()),
    );
    object.insert(
        "lastErrorChapterTitle".to_string(),
        Value::String(chapter.title.clone()),
    );
    object.insert(
        "updatedAt".to_string(),
        Value::Number((now_ts() * 1000).into()),
    );
}

#[cfg(test)]
fn set_string_if_empty(object: &mut serde_json::Map<String, Value>, key: &str, fallback: &str) {
    if object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        object.insert(key.to_string(), Value::String(fallback.to_string()));
    }
}

#[cfg(test)]
fn ensure_v2_defaults(object: &mut serde_json::Map<String, Value>) {
    object
        .entry("summary")
        .or_insert_with(|| json!({ "current": "", "recentChanges": [], "openQuestions": [] }));
    for key in [
        "chapterDigests",
        "arcs",
        "worldFacts",
        "characters",
        "relationships",
        "locations",
    ] {
        object
            .entry(key.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
    }
    object
        .entry("mapState".to_string())
        .or_insert_with(|| json!({ "dirty": false, "nodes": [], "edges": [] }));
    object
        .entry("renderArtifacts".to_string())
        .or_insert_with(|| json!({}));
}

#[cfg(test)]
fn has_semantic_content(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .is_some()
    {
        return true;
    }
    if let Some(summary) = object.get("summary").and_then(Value::as_object) {
        if summary
            .get("current")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .is_some()
            || summary
                .get("recentChanges")
                .and_then(Value::as_array)
                .is_some_and(|v| !v.is_empty())
            || summary
                .get("openQuestions")
                .and_then(Value::as_array)
                .is_some_and(|v| !v.is_empty())
        {
            return true;
        }
    }
    [
        "worldview",
        "worldFacts",
        "characters",
        "relationships",
        "locations",
        "chapterDigests",
    ]
    .iter()
    .any(|key| {
        object
            .get(*key)
            .and_then(Value::as_array)
            .is_some_and(|v| !v.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawler::http_client::HttpClient;
    use crate::parser::rule_engine::RuleEngine;
    use crate::service::ai_book_generation_service::AiBookGenerationService;
    use crate::service::ai_book_memory_v3::create_empty_ai_book_memory_v3;
    use crate::service::ai_book_service::AiBookService;
    use crate::service::book_source_service::BookSourceService;
    use crate::service::local_txt_book::LocalTxtBookService;
    use crate::storage::cache::file_cache::FileCache;
    use crate::storage::db;
    use crate::storage::db::repo::BookSourceRepo;
    use crate::util::crypto::random_string;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct TestRunner {
        tasks: Arc<Mutex<Vec<BoxFuture<'static, ()>>>>,
    }

    impl CatchupRunner for TestRunner {
        fn spawn_task(&self, fut: BoxFuture<'static, ()>) {
            self.tasks.lock().unwrap().push(fut);
        }
    }

    async fn create_generation_service_for_guard() -> (AiBookGenerationService, PathBuf) {
        let dir = std::env::temp_dir().join(format!("reader-ai-book-catchup-{}", random_string(8)));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        let repo = BookSourceRepo::new(pool.clone());
        let book_service = Arc::new(crate::service::book_service::BookService::new(
            HttpClient::new(5, None).unwrap(),
            RuleEngine::new().unwrap(),
            FileCache::new(dir.join("cache")),
            dir.to_str().unwrap(),
        ));
        let book_source_service = Arc::new(BookSourceService::new(repo, dir.to_str().unwrap()));
        let local_txt_book_service = Arc::new(LocalTxtBookService::new(&dir));
        let ai_book_service = Arc::new(AiBookService::new(pool, dir.to_str().unwrap()));
        (
            AiBookGenerationService::new_disabled(
                ai_book_service,
                book_service,
                book_source_service,
                local_txt_book_service,
            ),
            dir,
        )
    }

    fn sample_context() -> CatchupBookContext {
        CatchupBookContext {
            chapters: vec![
                CatchupChapter {
                    title: "第1章".to_string(),
                    chapter_url: "c1".to_string(),
                    index: 0,
                },
                CatchupChapter {
                    title: "第2章".to_string(),
                    chapter_url: "c2".to_string(),
                    index: 1,
                },
            ],
            memory: json!({
                "schemaVersion": 2,
                "bookUrl": "book-a",
                "bookName": "书A",
                "enabled": true,
                "summary": { "current": "", "recentChanges": [], "openQuestions": [] },
                "chapterDigests": [],
                "arcs": [],
                "worldFacts": [],
                "characters": [{ "id": "hero", "name": "旧角色", "aliases": [], "importance": "high", "currentStatus": "已登场", "statusHistory": [], "evidence": [] }],
                "relationships": [],
                "locations": [],
                "mapState": { "dirty": true, "reason": "旧地图", "nodes": [{ "id": "n1", "locationId": "l1", "label": "旧地点", "scale": "site" }], "edges": [] },
                "renderArtifacts": { "mapImageUrl": "/old-map.png" },
            }),
            write_guard: None,
            save_memory: save_memory_fn(|memory| async move { Ok(memory) }),
            fetch_content: fetch_content_fn(|chapter| async move {
                Ok(format!("正文{}", chapter.index + 1))
            }),
            generate_digest: generate_digest_fn(|_memory, chapter, _content| async move {
                Ok(AiBookChapterDigestCandidateV3 {
                    chapter_index: chapter.index,
                    chapter_title: chapter.title,
                    summary: format!("第{}章摘要", chapter.index + 1),
                    key_points: vec![],
                    has_important_changes: false,
                })
            }),
            generate_patch: generate_patch_fn(|_memory, chapter, _content, _digest| async move {
                Ok(AiBookKnowledgePatchV3 {
                    chapter_index: chapter.index,
                    ..Default::default()
                })
            }),
        }
    }

    #[test]
    fn catchup_start_uses_first_missing_digest_not_stale_processed_index() {
        let stale_manual_memory = json!({
            "processedChapterIndex": 6,
            "chapterDigests": [{ "chapterIndex": 6, "chapterTitle": "第7章" }]
        });
        assert_eq!(next_catchup_start_index(&stale_manual_memory), 0);

        let contiguous_memory = json!({
            "processedChapterIndex": 6,
            "chapterDigests": [
                { "chapterIndex": 0, "summary": "第1章摘要" },
                { "chapterIndex": 1, "summary": "第2章摘要" },
                { "chapterIndex": 2, "summary": "第3章摘要" },
                { "chapterIndex": 3, "summary": "第4章摘要" },
                { "chapterIndex": 4, "summary": "第5章摘要" },
                { "chapterIndex": 5, "summary": "第6章摘要" },
                { "chapterIndex": 6, "summary": "第7章摘要" }
            ]
        });
        assert_eq!(next_catchup_start_index(&contiguous_memory), 7);

        let empty_placeholder_memory = json!({
            "processedChapterIndex": 8,
            "chapterDigests": [
                { "chapterIndex": 0, "chapterTitle": "第1章", "summary": "", "keyPoints": [] },
                { "chapterIndex": 1, "chapterTitle": "第2章", "summary": "", "keyPoints": [] }
            ]
        });
        assert_eq!(next_catchup_start_index(&empty_placeholder_memory), 0);
    }

    #[tokio::test]
    async fn task_status_round_trip_for_same_book() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());

        let started = service
            .start_with("u1".to_string(), "book-a".to_string(), Some(3), || async {
                Ok(sample_context())
            })
            .await
            .unwrap();

        assert_eq!(started.status, "running");
        assert_eq!(started.target_chapter_index, Some(3));
        let status = service.get_status("u1", "book-a").await.unwrap();
        assert_eq!(status.status, "running");
        assert_eq!(status.book_url, "book-a");
        assert_eq!(runner.tasks.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn ai_book_v3_cancel_moves_to_canceling_then_canceled() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());

        service
            .start_with("u1".to_string(), "book-a".to_string(), None, || async {
                Ok(sample_context())
            })
            .await
            .unwrap();

        let canceled = service.request_cancel("u1", "book-a").await.unwrap();
        assert_eq!(canceled.status, "canceling");

        let key = task_key("u1", "book-a");
        let chapter = CatchupChapter {
            title: "第1章".to_string(),
            chapter_url: "c1".to_string(),
            index: 0,
        };
        service.set_plan(&key, 0, 1, 2).await;
        service.mark_processed(&key, &chapter).await;
        assert!(service.cancel_should_stop(&key).await);
        service.mark_canceled(&key).await;

        let final_status = service.get_status("u1", "book-a").await.unwrap();
        assert_eq!(final_status.status, "canceled");
        assert_eq!(final_status.processed_chapter_index, Some(0));
        assert_eq!(final_status.completed_chapters, 1);
    }

    #[tokio::test]
    async fn cancel_interrupts_blocked_chapter_fetch() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());

        service
            .start_with("u1".to_string(), "book-a".to_string(), None, || async {
                let mut context = sample_context();
                context.chapters.truncate(1);
                context.fetch_content = fetch_content_fn(|_chapter| async move {
                    std::future::pending::<()>().await;
                    Ok(String::new())
                });
                Ok(context)
            })
            .await
            .unwrap();

        let fut = runner.tasks.lock().unwrap().pop().unwrap();
        let handle = tokio::spawn(fut);
        tokio::task::yield_now().await;

        let canceled = service.request_cancel("u1", "book-a").await.unwrap();
        assert_eq!(canceled.status, "canceling");

        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .unwrap()
            .unwrap();
        let final_status = service.get_status("u1", "book-a").await.unwrap();
        assert_eq!(final_status.status, "canceled");
        assert_eq!(final_status.completed_chapters, 0);
    }

    #[tokio::test]
    async fn cancel_interrupts_blocked_chapter_fetch_keeps_task_queryable() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());

        service
            .start_with("u1".to_string(), "book-a".to_string(), None, || async {
                let mut context = sample_context();
                context.chapters.truncate(1);
                context.fetch_content = fetch_content_fn(|_chapter| async move {
                    std::future::pending::<()>().await;
                    Ok(String::new())
                });
                Ok(context)
            })
            .await
            .unwrap();

        let fut = runner.tasks.lock().unwrap().pop().unwrap();
        let handle = tokio::spawn(fut);
        tokio::task::yield_now().await;

        let canceled = service.request_cancel("u1", "book-a").await.unwrap();
        assert_eq!(canceled.status, "canceling");

        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .unwrap()
            .unwrap();

        let final_status = service.get_status("u1", "book-a").await.unwrap();
        assert_eq!(final_status.status, "canceled");
    }

    #[test]
    fn direct_memory_update_preserves_v2_and_advances_chapter() {
        let current = sample_context().memory;
        let chapter = CatchupChapter {
            title: "第3章".to_string(),
            chapter_url: "c3".to_string(),
            index: 2,
        };
        let next = parse_memory_update(
            r#"{"memory":{"schemaVersion":2,"summary":{"current":"局势变化","recentChanges":["主角入城"],"openQuestions":[]},"chapterDigests":[],"arcs":[],"worldFacts":[],"characters":[],"relationships":[],"locations":[],"mapState":{"dirty":false,"nodes":[],"edges":[]},"renderArtifacts":{}}}"#,
            &current,
            "book-a",
            "书A",
            "作者A",
            &chapter,
        )
        .unwrap();
        assert_eq!(
            next.get("processedChapterIndex").and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            next.pointer("/summary/current").and_then(Value::as_str),
            Some("局势变化")
        );
        assert_eq!(
            next.get("characters")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            next.pointer("/mapState/nodes")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            next.pointer("/renderArtifacts/mapImageUrl")
                .and_then(Value::as_str),
            Some("/old-map.png")
        );
    }

    #[test]
    fn target_url_does_not_duplicate_openai_v1_prefix() {
        let endpoint = ResolvedAiModelEndpoint {
            enabled: true,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "sk-test".to_string(),
            model: "gpt-4o-mini".to_string(),
            path: "/v1/chat/completions".to_string(),
            use_full_url: false,
            image_size: None,
            voice: None,
            response_format: None,
        };

        let target = build_target_url(&endpoint).unwrap();

        assert_eq!(
            target.as_str(),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn v2_patch_appends_entities_without_dropping_existing_arrays() {
        let current = sample_context().memory;
        let chapter = CatchupChapter {
            title: "第4章".to_string(),
            chapter_url: "c4".to_string(),
            index: 3,
        };
        let next = parse_memory_update(
            r#"{"memory":{"schemaVersion":2,"summary":{"current":"新角色出现"},"worldFacts":[{"id":"fact-new","title":"新设定","content":"新内容"}],"characters":[{"id":"villain","name":"新角色","aliases":[],"importance":"medium","currentStatus":"已登场","statusHistory":[],"evidence":[]}],"relationships":[],"locations":[{"id":"loc-new","name":"新地点","kind":"地点","importance":"medium","description":"新地点","evidence":[]}]}}"#,
            &current,
            "book-a",
            "书A",
            "作者A",
            &chapter,
        )
        .unwrap();

        let characters = next.get("characters").and_then(Value::as_array).unwrap();
        assert!(characters
            .iter()
            .any(|item| item.get("id").and_then(Value::as_str) == Some("hero")));
        assert!(characters
            .iter()
            .any(|item| item.get("id").and_then(Value::as_str) == Some("villain")));
        let locations = next.get("locations").and_then(Value::as_array).unwrap();
        assert!(locations
            .iter()
            .any(|item| item.get("id").and_then(Value::as_str) == Some("loc-new")));
    }

    #[test]
    fn v2_patch_merges_same_identity_entity_without_dropping_old_fields() {
        let mut current = sample_context().memory;
        current["characters"][0]["aliases"] = json!(["旧名"]);
        current["characters"][0]["statusHistory"] =
            json!([{ "chapterIndex": 0, "status": "已登场" }]);
        current["characters"][0]["evidence"] = json!(["旧证据"]);
        let chapter = CatchupChapter {
            title: "第5章".to_string(),
            chapter_url: "c5".to_string(),
            index: 4,
        };

        let next = parse_memory_update(
            r#"{"memory":{"schemaVersion":2,"summary":{"current":"旧角色状态更新"},"characters":[{"id":"hero","currentStatus":"受伤"}]}}"#,
            &current,
            "book-a",
            "书A",
            "作者A",
            &chapter,
        )
        .unwrap();

        let hero = next
            .get("characters")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some("hero"))
            .unwrap();
        assert_eq!(hero.get("name").and_then(Value::as_str), Some("旧角色"));
        assert_eq!(
            hero.get("currentStatus").and_then(Value::as_str),
            Some("受伤")
        );
        assert_eq!(
            hero.get("aliases").and_then(Value::as_array).map(Vec::len),
            Some(1)
        );
        assert_eq!(
            hero.get("statusHistory")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            hero.get("evidence").and_then(Value::as_array).map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn v2_patch_merges_same_name_based_relationship() {
        let mut current = sample_context().memory;
        current["relationships"] = json!([{
            "sourceName": "张羽",
            "targetName": "张羽的姐姐",
            "targetKind": "character",
            "relationType": "姐弟",
            "description": "旧描述",
            "evidence": []
        }]);
        let chapter = CatchupChapter {
            title: "第6章".to_string(),
            chapter_url: "c6".to_string(),
            index: 5,
        };

        let next = parse_memory_update(
            r#"{"memory":{"schemaVersion":2,"summary":{"current":"姐姐转账"},"relationships":[{"sourceName":"张羽","targetName":"张羽的姐姐","targetKind":"character","relationType":"姐弟","currentStatus":"转账500元","evidence":[]}]}}"#,
            &current,
            "book-a",
            "书A",
            "作者A",
            &chapter,
        )
        .unwrap();

        let relationships = next.get("relationships").and_then(Value::as_array).unwrap();
        assert_eq!(relationships.len(), 1);
        assert_eq!(
            relationships[0]
                .get("currentStatus")
                .and_then(Value::as_str),
            Some("转账500元")
        );
    }

    #[tokio::test]
    async fn chapter_failure_persists_last_error_to_memory() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());
        let saved = Arc::new(Mutex::new(Vec::<Value>::new()));
        let saved_for_context = saved.clone();

        service
            .start_with("u1".to_string(), "book-a".to_string(), Some(0), move || {
                let saved = saved_for_context.clone();
                async move {
                    let mut context = sample_context();
                    context.chapters.truncate(1);
                    context.save_memory = save_memory_fn(move |memory| {
                        let saved = saved.clone();
                        async move {
                            saved.lock().unwrap().push(memory.clone());
                            Ok(memory)
                        }
                    });
                    context.generate_digest =
                        generate_digest_fn(|_memory, _chapter, _content| async move {
                            Err(AppError::BadRequest("后端文本模型返回错误".to_string()))
                        });
                    Ok(context)
                }
            })
            .await
            .unwrap();

        let fut = runner.tasks.lock().unwrap().pop().unwrap();
        fut.await;

        let status = service.get_status("u1", "book-a").await.unwrap();
        assert_eq!(status.status, "failed");
        let saved = saved.lock().unwrap();
        assert_eq!(saved.len(), 1);
        assert!(saved[0]
            .get("lastError")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("后端文本模型")));
        assert_eq!(
            saved[0]
                .get("lastErrorChapterIndex")
                .and_then(Value::as_i64),
            Some(0)
        );
    }

    #[tokio::test]
    async fn ai_book_v3_catchup_uses_working_context_contract() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());

        service
            .start_with("u1".to_string(), "book-a".to_string(), Some(0), || async {
                let mut context = sample_context();
                context.chapters.truncate(1);
                context.generate_digest =
                    generate_digest_fn(|memory, chapter, _content| async move {
                        assert!(memory.relevant_characters.len() <= 20);
                        assert!(memory.recent_chapter_digests.len() <= 8);
                        assert_eq!(memory.current_chapter_index, None);
                        Ok(AiBookChapterDigestCandidateV3 {
                            chapter_index: chapter.index,
                            chapter_title: chapter.title,
                            summary: "关键变化".to_string(),
                            key_points: vec!["关系变化".to_string()],
                            has_important_changes: true,
                        })
                    });
                context.generate_patch =
                    generate_patch_fn(|memory, chapter, _content, _digest| async move {
                        assert_eq!(memory.current_chapter_index, Some(chapter.index));
                        assert_eq!(
                            memory.current_chapter_title.as_deref(),
                            Some(chapter.title.as_str())
                        );
                        Ok(AiBookKnowledgePatchV3 {
                            chapter_index: chapter.index,
                            summary: Some("关键变化".to_string()),
                            ..Default::default()
                        })
                    });
                Ok(context)
            })
            .await
            .unwrap();

        let fut = runner.tasks.lock().unwrap().pop().unwrap();
        fut.await;

        let status = service.get_status("u1", "book-a").await.unwrap();
        assert_eq!(status.status, "completed");
    }

    #[tokio::test]
    async fn ai_book_v3_cancel_during_digest_prevents_patch_call() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());
        let patch_calls = Arc::new(Mutex::new(0));
        let digest_started = Arc::new(tokio::sync::Notify::new());
        let digest_release = Arc::new(tokio::sync::Notify::new());

        service
            .start_with("u1".to_string(), "book-a".to_string(), Some(0), {
                let patch_calls = patch_calls.clone();
                let digest_started = digest_started.clone();
                let digest_release = digest_release.clone();
                move || {
                    let patch_calls = patch_calls.clone();
                    let digest_started = digest_started.clone();
                    let digest_release = digest_release.clone();
                    async move {
                        let mut context = sample_context();
                        context.chapters.truncate(1);
                        context.generate_digest =
                            generate_digest_fn(move |_memory, chapter, _content| {
                                let digest_started = digest_started.clone();
                                let digest_release = digest_release.clone();
                                async move {
                                    digest_started.notify_one();
                                    digest_release.notified().await;
                                    Ok(AiBookChapterDigestCandidateV3 {
                                        chapter_index: chapter.index,
                                        chapter_title: chapter.title,
                                        summary: "关键变化".to_string(),
                                        key_points: vec!["关系变化".to_string()],
                                        has_important_changes: true,
                                    })
                                }
                            });
                        context.generate_patch =
                            generate_patch_fn(move |_memory, chapter, _content, _digest| {
                                let patch_calls = patch_calls.clone();
                                async move {
                                    *patch_calls.lock().unwrap() += 1;
                                    Ok(AiBookKnowledgePatchV3 {
                                        chapter_index: chapter.index,
                                        summary: Some("不该执行".to_string()),
                                        ..Default::default()
                                    })
                                }
                            });
                        Ok(context)
                    }
                }
            })
            .await
            .unwrap();

        let fut = runner.tasks.lock().unwrap().pop().unwrap();
        let handle = tokio::spawn(fut);
        digest_started.notified().await;

        let canceled = service.request_cancel("u1", "book-a").await.unwrap();
        assert_eq!(canceled.status, "canceling");

        digest_release.notify_one();
        handle.await.unwrap();

        assert_eq!(*patch_calls.lock().unwrap(), 0);
        let final_status = service.get_status("u1", "book-a").await.unwrap();
        assert_eq!(final_status.status, "canceled");
    }

    #[tokio::test]
    async fn ai_book_v3_digest_only_skips_patch() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());
        let saved = Arc::new(Mutex::new(Vec::<Value>::new()));
        let saved_for_context = saved.clone();

        service
            .start_with("u1".to_string(), "book-a".to_string(), Some(0), move || {
                let saved = saved_for_context.clone();
                async move {
                    let mut context = sample_context();
                    context.chapters.truncate(1);
                    context.save_memory = save_memory_fn(move |memory| {
                        let saved = saved.clone();
                        async move {
                            saved.lock().unwrap().push(memory.clone());
                            Ok(memory)
                        }
                    });
                    context.generate_digest =
                        generate_digest_fn(|memory, chapter, _content| async move {
                            assert!(memory.recent_chapter_digests.is_empty());
                            Ok(AiBookChapterDigestCandidateV3 {
                                chapter_index: chapter.index,
                                chapter_title: chapter.title,
                                summary: "仅摘要".to_string(),
                                key_points: vec!["日常推进".to_string()],
                                has_important_changes: false,
                            })
                        });
                    context.generate_patch =
                        generate_patch_fn(|_memory, _chapter, _content, _digest| async move {
                            Err(AppError::BadRequest("unexpected patch".to_string()))
                        });
                    Ok(context)
                }
            })
            .await
            .unwrap();

        let fut = runner.tasks.lock().unwrap().pop().unwrap();
        fut.await;

        let status = service.get_status("u1", "book-a").await.unwrap();
        assert_eq!(status.status, "completed");
        assert_eq!(
            status
                .stats
                .as_ref()
                .and_then(|s| Some(s.skipped_patch_chapters)),
            Some(1)
        );
        let saved = saved.lock().unwrap();
        let saved_memory: AiBookMemoryV3 =
            serde_json::from_value(saved.last().cloned().unwrap()).unwrap();
        assert_eq!(saved_memory.chapter_digests.len(), 1);
        assert_eq!(
            saved_memory
                .catchup_stats
                .as_ref()
                .map(|s| s.skipped_patch_chapters),
            Some(1)
        );
    }

    #[tokio::test]
    async fn ai_book_v3_digest_guard_forces_patch_when_needed() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());
        let patch_calls = Arc::new(Mutex::new(0));
        let patch_calls_for_context = patch_calls.clone();

        service
            .start_with("u1".to_string(), "book-a".to_string(), Some(0), move || {
                let patch_calls = patch_calls_for_context.clone();
                async move {
                    let mut context = sample_context();
                    context.chapters.truncate(1);
                    context.fetch_content = fetch_content_fn(|chapter| async move {
                        Ok(format!("正文{} 新角色加入宗门", chapter.index + 1))
                    });
                    context.generate_digest =
                        generate_digest_fn(|_memory, chapter, _content| async move {
                            Ok(AiBookChapterDigestCandidateV3 {
                                chapter_index: chapter.index,
                                chapter_title: chapter.title,
                                summary: "普通摘要".to_string(),
                                key_points: vec!["轻微变化".to_string()],
                                has_important_changes: false,
                            })
                        });
                    context.generate_patch =
                        generate_patch_fn(move |_memory, chapter, _content, _digest| {
                            let patch_calls = patch_calls.clone();
                            async move {
                                *patch_calls.lock().unwrap() += 1;
                                Ok(AiBookKnowledgePatchV3 {
                                    chapter_index: chapter.index,
                                    summary: Some("普通摘要".to_string()),
                                    characters: vec![
                                        crate::model::ai_book_generation::AiBookCharacterPatchV3 {
                                            name: "新角色".to_string(),
                                            aliases: vec![],
                                            status: Some("加入宗门".to_string()),
                                            faction: Some("宗门".to_string()),
                                            location: None,
                                            description: Some("首次登场".to_string()),
                                            last_seen_chapter: None,
                                        },
                                    ],
                                    ..Default::default()
                                })
                            }
                        });
                    Ok(context)
                }
            })
            .await
            .unwrap();

        let fut = runner.tasks.lock().unwrap().pop().unwrap();
        fut.await;

        assert_eq!(*patch_calls.lock().unwrap(), 1);
        let status = service.get_status("u1", "book-a").await.unwrap();
        assert_eq!(
            status.stats.as_ref().and_then(|s| Some(s.patch_calls)),
            Some(1)
        );
    }

    #[tokio::test]
    async fn ai_book_v3_catchup_stats_increment() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());

        service
            .start_with("u1".to_string(), "book-a".to_string(), Some(0), || async {
                let mut context = sample_context();
                context.chapters.truncate(1);
                context.generate_digest =
                    generate_digest_fn(|_memory, chapter, _content| async move {
                        Ok(AiBookChapterDigestCandidateV3 {
                            chapter_index: chapter.index,
                            chapter_title: chapter.title,
                            summary: "关键变化".to_string(),
                            key_points: vec!["关系变化".to_string()],
                            has_important_changes: true,
                        })
                    });
                context.generate_patch =
                    generate_patch_fn(|_memory, chapter, _content, _digest| async move {
                        Ok(AiBookKnowledgePatchV3 {
                            chapter_index: chapter.index,
                            summary: Some("关键变化".to_string()),
                            ..Default::default()
                        })
                    });
                Ok(context)
            })
            .await
            .unwrap();

        let fut = runner.tasks.lock().unwrap().pop().unwrap();
        fut.await;

        let status = service.get_status("u1", "book-a").await.unwrap();
        let stats = status.stats.unwrap();
        assert_eq!(stats.total_model_calls, 2);
        assert_eq!(stats.digest_calls, 1);
        assert_eq!(stats.patch_calls, 1);
        assert!(stats.last_call_latency_ms.is_some());
        assert!(stats.average_call_latency_ms.is_some());
    }

    #[tokio::test]
    async fn ai_book_v3_catchup_holds_write_guard_until_task_finishes() {
        let runner = Arc::new(TestRunner::default());
        let service = AiBookCatchupService::new_with_runner(runner.clone());
        let (generation_service, dir) = create_generation_service_for_guard().await;
        let guard = generation_service
            .acquire_write_guard("u1", "book-a")
            .unwrap();
        let fetch_started = Arc::new(tokio::sync::Notify::new());
        let fetch_release = Arc::new(tokio::sync::Notify::new());

        service
            .start_with("u1".to_string(), "book-a".to_string(), Some(0), {
                let fetch_started = fetch_started.clone();
                let fetch_release = fetch_release.clone();
                move || {
                    let fetch_started = fetch_started.clone();
                    let fetch_release = fetch_release.clone();
                    let guard = guard;
                    async move {
                        let mut context = sample_context();
                        context.chapters.truncate(1);
                        context.memory = serde_json::to_value(create_empty_ai_book_memory_v3(
                            "book-a",
                            Some("书A".to_string()),
                            Some("作者A".to_string()),
                        ))
                        .unwrap();
                        context.write_guard = Some(guard);
                        context.fetch_content = fetch_content_fn(move |chapter| {
                            let fetch_started = fetch_started.clone();
                            let fetch_release = fetch_release.clone();
                            async move {
                                fetch_started.notify_one();
                                fetch_release.notified().await;
                                Ok(format!("正文{}", chapter.index + 1))
                            }
                        });
                        context.generate_digest =
                            generate_digest_fn(|_memory, chapter, _content| async move {
                                Ok(AiBookChapterDigestCandidateV3 {
                                    chapter_index: chapter.index,
                                    chapter_title: chapter.title,
                                    summary: "摘要".to_string(),
                                    key_points: vec!["要点".to_string()],
                                    has_important_changes: false,
                                })
                            });
                        Ok(context)
                    }
                }
            })
            .await
            .unwrap();

        let fut = runner.tasks.lock().unwrap().pop().unwrap();
        let handle = tokio::spawn(fut);
        fetch_started.notified().await;

        let err = generation_service
            .acquire_write_guard("u1", "book-a")
            .unwrap_err();
        assert!(err.to_string().contains("正在生成中"));

        fetch_release.notify_one();
        handle.await.unwrap();

        assert!(generation_service
            .acquire_write_guard("u1", "book-a")
            .is_ok());
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[test]
    fn patch_update_adds_v2_digest() {
        let current = sample_context().memory;
        let chapter = CatchupChapter {
            title: "第1章".to_string(),
            chapter_url: "c1".to_string(),
            index: 0,
        };
        let next = parse_memory_update(
            r#"{"summary":"第一章发生变化","worldFacts":[],"characters":[],"relationships":[],"locations":[]}"#,
            &current,
            "book-a",
            "书A",
            "作者A",
            &chapter,
        )
        .unwrap();
        assert_eq!(
            next.pointer("/summary/current").and_then(Value::as_str),
            Some("第一章发生变化")
        );
        assert_eq!(
            next.get("chapterDigests")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn json_content_parser_extracts_wrapped_object() {
        let parsed = parse_json_content("说明：{\"summary\":\"ok\"}").unwrap();
        assert_eq!(parsed.get("summary").and_then(Value::as_str), Some("ok"));
    }

    #[test]
    fn responses_endpoint_uses_output_text() {
        let body = build_model_body("/v1/responses", "test-model", "hi".to_string());
        assert!(body.get("messages").is_none());
        assert_eq!(
            body.pointer("/text/format/type").and_then(Value::as_str),
            Some("json_object")
        );
        assert_eq!(body.get("stream").and_then(Value::as_bool), Some(false));

        let content = extract_model_content(
            "/v1/responses",
            &json!({ "output_text": "{\"summary\":\"ok\"}" }),
        )
        .unwrap();
        assert!(content.contains("\"summary\":\"ok\""));
    }

    #[test]
    fn gemini_endpoint_uses_generate_content_shape() {
        let body = build_model_body(
            "/v1beta/models/gemini:generateContent",
            "gemini-2.5-flash",
            "hi".to_string(),
        );
        assert!(body.get("contents").is_some());
        assert!(body.get("messages").is_none());
    }

    #[test]
    fn anthropic_endpoint_uses_messages_shape() {
        let body = build_model_body("/v1/messages", "claude-sonnet", "hi".to_string());
        assert!(body.get("system").is_some());
        assert!(body.get("messages").is_some());
    }
}
