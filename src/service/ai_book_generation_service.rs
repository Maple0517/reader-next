use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::error::error::AppError;
use crate::model::ai_book::{AiBookChapterMemoryViewResponse, AiBookMemoryV3};
use crate::model::ai_book_generation::{
    AiBookChapterDigestCandidateV3, AiBookCombinedChapterGenerationV3, AiBookKnowledgePatchV3,
};
use crate::model::book::Book;
use crate::service::ai_book_memory_v3::{
    merge_ai_book_memory_v3, normalize_knowledge_patch_v3, select_ai_book_chapter_view_v3,
    select_ai_book_display_memory_v3, select_working_context_v3, AiBookWorkingContextV3,
};
use crate::service::ai_book_service::AiBookService;
use crate::service::book_service::BookService;
use crate::service::book_source_service::BookSourceService;
use crate::service::local_txt_book::{is_local_txt_origin, LocalTxtBookService};
use crate::util::text::{normalize_source_url, repair_encoded_url};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiBookGenerationMode {
    Auto,
    Manual,
}

impl Default for AiBookGenerationMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Clone)]
pub struct AiBookGenerationService {
    ai_book_service: Arc<AiBookService>,
    book_service: Arc<BookService>,
    book_source_service: Arc<BookSourceService>,
    local_txt_book_service: Arc<LocalTxtBookService>,
    write_guards: Arc<Mutex<HashSet<String>>>,
    generator: Arc<dyn ChapterGenerationModel>,
}

impl AiBookGenerationService {
    pub fn new(
        ai_book_service: Arc<AiBookService>,
        book_service: Arc<BookService>,
        book_source_service: Arc<BookSourceService>,
        local_txt_book_service: Arc<LocalTxtBookService>,
    ) -> Self {
        Self::new_with_generator(
            ai_book_service,
            book_service,
            book_source_service,
            local_txt_book_service,
            Arc::new(DisabledChapterGenerationModel),
        )
    }

    pub fn new_with_generator(
        ai_book_service: Arc<AiBookService>,
        book_service: Arc<BookService>,
        book_source_service: Arc<BookSourceService>,
        local_txt_book_service: Arc<LocalTxtBookService>,
        generator: Arc<dyn ChapterGenerationModel>,
    ) -> Self {
        Self {
            ai_book_service,
            book_service,
            book_source_service,
            local_txt_book_service,
            write_guards: Arc::new(Mutex::new(HashSet::new())),
            generator,
        }
    }

    pub fn acquire_write_guard(
        &self,
        user_ns: &str,
        book_url: &str,
    ) -> Result<AiBookWriteGuard, AppError> {
        let key = guard_key(user_ns, book_url);
        let mut guards = self.write_guards.lock().unwrap();
        if !guards.insert(key.clone()) {
            return Err(AppError::BadRequest(format!("当前书籍正在生成中: {book_url}")));
        }
        Ok(AiBookWriteGuard {
            key,
            write_guards: Arc::clone(&self.write_guards),
        })
    }

    pub async fn generate_current_chapter(
        &self,
        user_ns: &str,
        shelf_book: &Book,
        chapter_index: i32,
        mode: AiBookGenerationMode,
    ) -> Result<AiBookChapterMemoryViewResponse, AppError> {
        let book_url = repair_encoded_url(&shelf_book.book_url);
        let _guard = self.acquire_write_guard(user_ns, &book_url)?;
        let loaded = self
            .load_generation_context(user_ns, shelf_book, chapter_index)
            .await?;
        let mut memory = self
            .ai_book_service
            .get_or_create_v3(
                user_ns,
                &book_url,
                Some(shelf_book.name.clone()),
                Some(shelf_book.author.clone()),
            )
            .await?;
        let generation = self
            .generate_combined_for_current_chapter(&loaded.chapter_text, &memory, &loaded.chapter, mode)
            .await?;
        apply_generation_result(
            &mut memory,
            &loaded.chapter,
            loaded.chapter_text.as_str(),
            generation,
        )?;
        let saved = self.ai_book_service.save_v3(user_ns, &book_url, memory).await?;
        Ok(project_chapter_response(&saved, chapter_index))
    }

    pub async fn generate_digest(
        &self,
        chapter_text: &str,
        memory: &AiBookWorkingContextV3,
        chapter: &LoadedChapter,
        mode: AiBookGenerationMode,
    ) -> Result<AiBookChapterDigestCandidateV3, AppError> {
        self.generator
            .generate_digest(chapter_text, memory, chapter, mode)
            .await
    }

    pub async fn generate_patch_for_catchup(
        &self,
        chapter_text: &str,
        memory: &AiBookWorkingContextV3,
        chapter: &LoadedChapter,
        digest: &AiBookChapterDigestCandidateV3,
        mode: AiBookGenerationMode,
    ) -> Result<AiBookKnowledgePatchV3, AppError> {
        self.generator
            .generate_patch_for_catchup(chapter_text, memory, chapter, digest, mode)
            .await
    }

    async fn generate_combined_for_current_chapter(
        &self,
        chapter_text: &str,
        memory: &AiBookMemoryV3,
        chapter: &LoadedChapter,
        mode: AiBookGenerationMode,
    ) -> Result<AiBookCombinedChapterGenerationV3, AppError> {
        if let Some(combined) = self
            .generator
            .generate_combined(chapter_text, memory, chapter, mode)
            .await?
        {
            return Ok(combined);
        }
        let digest_context = select_working_context_v3(memory, None, chapter_text);
        let digest = self
            .generate_digest(chapter_text, &digest_context, chapter, mode)
            .await?;
        let patch_context = select_working_context_v3(memory, Some(&crate::model::ai_book::AiBookChapterDigestV3 {
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
        }), chapter_text);
        let patch = self
            .generate_patch_for_catchup(chapter_text, &patch_context, chapter, &digest, mode)
            .await?;
        Ok(AiBookCombinedChapterGenerationV3 {
            chapter_digest: digest,
            patch,
        })
    }

    async fn load_generation_context(
        &self,
        user_ns: &str,
        shelf_book: &Book,
        chapter_index: i32,
    ) -> Result<LoadedGenerationContext, AppError> {
        let chapter = self
            .load_chapter_identity(user_ns, shelf_book, chapter_index)
            .await?;
        let chapter_text = self.load_chapter_text(user_ns, shelf_book, &chapter).await?;
        if chapter_text.trim().is_empty() {
            return Err(AppError::BadRequest("章节内容为空".to_string()));
        }
        Ok(LoadedGenerationContext { chapter, chapter_text })
    }

    async fn load_chapter_identity(
        &self,
        user_ns: &str,
        shelf_book: &Book,
        chapter_index: i32,
    ) -> Result<LoadedChapter, AppError> {
        let book_url = repair_encoded_url(&shelf_book.book_url);
        if is_local_txt_origin(&shelf_book.origin) || book_url.starts_with("local-txt:") {
            let chapters = self
                .local_txt_book_service
                .get_chapter_list(user_ns, &book_url)
                .await?;
            let chapter = chapters
                .into_iter()
                .find(|chapter| chapter.index == chapter_index)
                .ok_or_else(|| AppError::BadRequest("章节不存在".to_string()))?;
            return Ok(LoadedChapter {
                index: chapter.index,
                title: chapter.title,
                chapter_url: chapter.url,
            });
        }

        let source = self
            .resolve_book_source(user_ns, shelf_book, &book_url)
            .await?;
        let toc_url = shelf_book.toc_url.as_deref().unwrap_or(&book_url);
        let chapters = self
            .book_service
            .get_chapter_list_with_cache(user_ns, &source, toc_url, false)
            .await?;
        let chapter = chapters
            .into_iter()
            .find(|chapter| chapter.index == chapter_index)
            .ok_or_else(|| AppError::BadRequest("章节不存在".to_string()))?;
        Ok(LoadedChapter {
            index: chapter.index,
            title: chapter.title,
            chapter_url: chapter.url,
        })
    }

    async fn load_chapter_text(
        &self,
        user_ns: &str,
        shelf_book: &Book,
        chapter: &LoadedChapter,
    ) -> Result<String, AppError> {
        let book_url = repair_encoded_url(&shelf_book.book_url);
        if is_local_txt_origin(&shelf_book.origin) || book_url.starts_with("local-txt:") {
            return self
                .local_txt_book_service
                .get_content(user_ns, &chapter.chapter_url)
                .await;
        }
        let source = self
            .resolve_book_source(user_ns, shelf_book, &book_url)
            .await?;
        self.book_service
            .get_content(user_ns, &book_url, &source, &chapter.chapter_url)
            .await
    }

    async fn resolve_book_source(
        &self,
        user_ns: &str,
        shelf_book: &Book,
        book_url: &str,
    ) -> Result<crate::model::book_source::BookSource, AppError> {
        let origin = normalize_source_url(&shelf_book.origin);
        if let Some(source) = self.book_source_service.get(user_ns, &origin).await? {
            return Ok(source);
        }
        let sources = self.book_source_service.list(user_ns).await?;
        if let Some(source) = sources
            .into_iter()
            .find(|source| normalize_source_url(&source.book_source_url) == origin)
        {
            return Ok(source);
        }
        if let Some(source) = self.book_source_service.get(user_ns, book_url).await? {
            return Ok(source);
        }
        Err(AppError::NotFound("bookSource not found".to_string()))
    }
}

fn apply_generation_result(
    memory: &mut AiBookMemoryV3,
    chapter: &LoadedChapter,
    chapter_text: &str,
    mut generation: AiBookCombinedChapterGenerationV3,
) -> Result<(), AppError> {
    generation.chapter_digest.chapter_index = chapter.index;
    generation.chapter_digest.chapter_title = chapter.title.clone();
    if generation.chapter_digest.chapter_title.trim().is_empty() {
        return Err(AppError::BadRequest("chapter digest title required".to_string()));
    }
    let digest_for_context = crate::model::ai_book::AiBookChapterDigestV3 {
        chapter_index: generation.chapter_digest.chapter_index,
        chapter_title: generation.chapter_digest.chapter_title.clone(),
        summary: generation.chapter_digest.summary.clone(),
        key_points: generation.chapter_digest.key_points.clone(),
        characters: Vec::new(),
        character_states: Vec::new(),
        character_relations: Vec::new(),
        knowledge_facts: Vec::new(),
        locations: Vec::new(),
        location_edges: Vec::new(),
    };
    let mut patch = generation.patch;
    patch.chapter_index = chapter.index;
    if patch.summary.as_deref().is_none_or(|summary| summary.trim().is_empty()) {
        patch.summary = Some(generation.chapter_digest.summary.clone());
    }
    let working_context = select_working_context_v3(memory, Some(&digest_for_context), chapter_text);
    let normalized_patch = normalize_knowledge_patch_v3(patch, &working_context);
    let next = merge_ai_book_memory_v3(memory.clone(), normalized_patch);
    *memory = upsert_digest(next, generation.chapter_digest);
    memory.processed_chapter_title = Some(chapter.title.clone());
    memory.last_error = None;
    memory.last_error_chapter_index = None;
    memory.last_error_chapter_title = None;
    Ok(())
}

fn upsert_digest(mut memory: AiBookMemoryV3, digest: AiBookChapterDigestCandidateV3) -> AiBookMemoryV3 {
    let digest_v3 = crate::model::ai_book::AiBookChapterDigestV3 {
        chapter_index: digest.chapter_index,
        chapter_title: digest.chapter_title,
        summary: digest.summary,
        key_points: digest.key_points,
        characters: Vec::new(),
        character_states: Vec::new(),
        character_relations: Vec::new(),
        knowledge_facts: Vec::new(),
        locations: Vec::new(),
        location_edges: Vec::new(),
    };
    let mut replaced = false;
    for existing in &mut memory.chapter_digests {
        if existing.chapter_index == digest_v3.chapter_index {
            *existing = digest_v3.clone();
            replaced = true;
            break;
        }
    }
    if !replaced {
        memory.chapter_digests.push(digest_v3);
        memory.chapter_digests.sort_by_key(|item| item.chapter_index);
    }
    memory
}

fn project_chapter_response(
    memory: &AiBookMemoryV3,
    chapter_index: i32,
) -> AiBookChapterMemoryViewResponse {
    AiBookChapterMemoryViewResponse {
        chapter: select_ai_book_chapter_view_v3(memory, chapter_index),
        memory: select_ai_book_display_memory_v3(memory),
    }
}

fn guard_key(user_ns: &str, book_url: &str) -> String {
    format!("{}::{}", user_ns.trim(), repair_encoded_url(book_url))
}

#[derive(Debug)]
pub struct AiBookWriteGuard {
    key: String,
    write_guards: Arc<Mutex<HashSet<String>>>,
}

impl Drop for AiBookWriteGuard {
    fn drop(&mut self) {
        self.write_guards.lock().unwrap().remove(&self.key);
    }
}

#[derive(Debug, Clone)]
pub struct LoadedChapter {
    pub index: i32,
    pub title: String,
    pub chapter_url: String,
}

struct LoadedGenerationContext {
    chapter: LoadedChapter,
    chapter_text: String,
}

pub trait ChapterGenerationModel: Send + Sync {
    fn generate_combined<'a>(
        &'a self,
        chapter_text: &'a str,
        memory: &'a AiBookMemoryV3,
        chapter: &'a LoadedChapter,
        mode: AiBookGenerationMode,
    ) -> futures::future::BoxFuture<'a, Result<Option<AiBookCombinedChapterGenerationV3>, AppError>>;

    fn generate_digest<'a>(
        &'a self,
        chapter_text: &'a str,
        memory: &'a AiBookWorkingContextV3,
        chapter: &'a LoadedChapter,
        mode: AiBookGenerationMode,
    ) -> futures::future::BoxFuture<'a, Result<AiBookChapterDigestCandidateV3, AppError>>;

    fn generate_patch_for_catchup<'a>(
        &'a self,
        chapter_text: &'a str,
        memory: &'a AiBookWorkingContextV3,
        chapter: &'a LoadedChapter,
        digest: &'a AiBookChapterDigestCandidateV3,
        mode: AiBookGenerationMode,
    ) -> futures::future::BoxFuture<'a, Result<AiBookKnowledgePatchV3, AppError>>;
}

struct DisabledChapterGenerationModel;

impl ChapterGenerationModel for DisabledChapterGenerationModel {
    fn generate_combined<'a>(
        &'a self,
        _chapter_text: &'a str,
        _memory: &'a AiBookMemoryV3,
        _chapter: &'a LoadedChapter,
        _mode: AiBookGenerationMode,
    ) -> futures::future::BoxFuture<'a, Result<Option<AiBookCombinedChapterGenerationV3>, AppError>> {
        Box::pin(async { Ok(None) })
    }

    fn generate_digest<'a>(
        &'a self,
        _chapter_text: &'a str,
        _memory: &'a AiBookWorkingContextV3,
        _chapter: &'a LoadedChapter,
        _mode: AiBookGenerationMode,
    ) -> futures::future::BoxFuture<'a, Result<AiBookChapterDigestCandidateV3, AppError>> {
        Box::pin(async {
            Err(AppError::BadRequest(
                "AI资料生成功能尚未接入模型".to_string(),
            ))
        })
    }

    fn generate_patch_for_catchup<'a>(
        &'a self,
        _chapter_text: &'a str,
        _memory: &'a AiBookWorkingContextV3,
        _chapter: &'a LoadedChapter,
        _digest: &'a AiBookChapterDigestCandidateV3,
        _mode: AiBookGenerationMode,
    ) -> futures::future::BoxFuture<'a, Result<AiBookKnowledgePatchV3, AppError>> {
        Box::pin(async {
            Err(AppError::BadRequest(
                "AI资料生成功能尚未接入模型".to_string(),
            ))
        })
    }
}

pub fn parse_model_json_value(text: &str) -> Result<Value, AppError> {
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
        .map_err(|_| AppError::BadRequest("AI资料生成返回 JSON 格式不正确".to_string()))
}

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
mod tests {
    use super::*;
    use crate::storage::cache::file_cache::FileCache;
    use crate::storage::db;
    use crate::storage::db::repo::BookSourceRepo;
    use crate::util::crypto::random_string;
    use crate::{crawler::http_client::HttpClient, parser::rule_engine::RuleEngine};
    use futures::future::BoxFuture;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tokio::fs;

    #[derive(Clone, Default)]
    struct FakeChapterGenerationModel {
        combined: Arc<Mutex<HashMap<i32, AiBookCombinedChapterGenerationV3>>>,
    }

    impl FakeChapterGenerationModel {
        fn with_combined(chapter_index: i32, combined: AiBookCombinedChapterGenerationV3) -> Self {
            let mut map = HashMap::new();
            map.insert(chapter_index, combined);
            Self {
                combined: Arc::new(Mutex::new(map)),
            }
        }
    }

    impl ChapterGenerationModel for FakeChapterGenerationModel {
        fn generate_combined<'a>(
            &'a self,
            _chapter_text: &'a str,
            _memory: &'a AiBookMemoryV3,
            chapter: &'a LoadedChapter,
            _mode: AiBookGenerationMode,
        ) -> BoxFuture<'a, Result<Option<AiBookCombinedChapterGenerationV3>, AppError>> {
            let value = self.combined.lock().unwrap().get(&chapter.index).cloned();
            Box::pin(async move { Ok(value) })
        }

        fn generate_digest<'a>(
            &'a self,
            _chapter_text: &'a str,
            _memory: &'a AiBookWorkingContextV3,
            _chapter: &'a LoadedChapter,
            _mode: AiBookGenerationMode,
        ) -> BoxFuture<'a, Result<AiBookChapterDigestCandidateV3, AppError>> {
            Box::pin(async { Err(AppError::BadRequest("unexpected digest call".to_string())) })
        }

        fn generate_patch_for_catchup<'a>(
            &'a self,
            _chapter_text: &'a str,
            _memory: &'a AiBookWorkingContextV3,
            _chapter: &'a LoadedChapter,
            _digest: &'a AiBookChapterDigestCandidateV3,
            _mode: AiBookGenerationMode,
        ) -> BoxFuture<'a, Result<AiBookKnowledgePatchV3, AppError>> {
            Box::pin(async { Err(AppError::BadRequest("unexpected patch call".to_string())) })
        }
    }

    async fn create_services() -> (AiBookGenerationService, PathBuf) {
        let dir = std::env::temp_dir().join(format!("reader-ai-book-generation-{}", random_string(8)));
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
            AiBookGenerationService::new(
                ai_book_service,
                book_service,
                book_source_service,
                local_txt_book_service,
            ),
            dir,
        )
    }

    async fn create_services_with_model(
        model: Arc<dyn ChapterGenerationModel>,
    ) -> (AiBookGenerationService, PathBuf) {
        let (service, dir) = create_services().await;
        let service = AiBookGenerationService::new_with_generator(
            service.ai_book_service.clone(),
            service.book_service.clone(),
            service.book_source_service.clone(),
            service.local_txt_book_service.clone(),
            model,
        );
        (service, dir)
    }

    async fn save_local_txt_book(
        service: &AiBookGenerationService,
        user_ns: &str,
        file_name: &str,
        content: &str,
    ) -> Book {
        let book = service
            .local_txt_book_service
            .import_txt_book(user_ns, file_name, content.as_bytes())
            .await
            .unwrap();
        service
            .book_service
            .save_book(user_ns, book.clone())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn ai_book_v3_generate_loads_chapter_for_current_user_only() {
        let model = Arc::new(FakeChapterGenerationModel::with_combined(
            0,
            AiBookCombinedChapterGenerationV3 {
                chapter_digest: AiBookChapterDigestCandidateV3 {
                    chapter_index: 0,
                    chapter_title: "第一章 开场".to_string(),
                    summary: "甲用户章节摘要".to_string(),
                    key_points: vec!["甲用户内容".to_string()],
                    has_important_changes: true,
                },
                patch: AiBookKnowledgePatchV3 {
                    chapter_index: 0,
                    summary: Some("甲用户章节摘要".to_string()),
                    ..Default::default()
                },
            },
        ));
        let (service, dir) = create_services_with_model(model).await;
        let book = save_local_txt_book(
            &service,
            "reader-a",
            "chapter.txt",
            "第一章 开场\n甲用户正文\n\n第二章 尾声\n收尾",
        )
        .await;
        let _other = save_local_txt_book(
            &service,
            "reader-b",
            "chapter.txt",
            "第一章 开场\n乙用户正文",
        )
        .await;

        let response = service
            .generate_current_chapter("reader-a", &book, 0, AiBookGenerationMode::Manual)
            .await
            .unwrap();
        assert_eq!(response.chapter.chapter_title.as_deref(), Some("第一章 开场"));
        assert_eq!(response.memory.summary.current, "甲用户章节摘要");

        let result = service
            .generate_current_chapter("reader-b", &book, 0, AiBookGenerationMode::Manual)
            .await;
        assert!(result.is_err());

        let loaded_other = service
            .ai_book_service
            .get_or_create_v3("reader-b", &book.book_url, None, None)
            .await
            .unwrap();
        assert!(loaded_other.summary.current.is_empty());
        assert!(loaded_other.chapter_digests.is_empty());

        let _ = fs::remove_dir_all(dir).await;
    }

    #[test]
    fn ai_book_v3_per_book_write_guard_rejects_concurrent_generation() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (service, dir) = runtime.block_on(create_services());
        let guard = service.acquire_write_guard("reader-a", "book://same").unwrap();
        let err = service.acquire_write_guard("reader-a", "book://same").unwrap_err();
        assert!(err.to_string().contains("正在生成中"));
        drop(guard);
        assert!(service.acquire_write_guard("reader-a", "book://same").is_ok());
        runtime.block_on(async { let _ = fs::remove_dir_all(dir).await; });
    }

    #[tokio::test]
    async fn ai_book_v3_combined_current_chapter_rewrites_model_chapter_identity() {
        let model = Arc::new(FakeChapterGenerationModel::with_combined(
            0,
            AiBookCombinedChapterGenerationV3 {
                chapter_digest: AiBookChapterDigestCandidateV3 {
                    chapter_index: 99,
                    chapter_title: "模型乱写标题".to_string(),
                    summary: "错误索引也要落到当前章".to_string(),
                    key_points: vec!["当前章关键点".to_string()],
                    has_important_changes: true,
                },
                patch: AiBookKnowledgePatchV3 {
                    chapter_index: 77,
                    summary: Some("错误索引也要落到当前章".to_string()),
                    knowledge_facts: vec![crate::model::ai_book_generation::AiBookKnowledgeFactPatchV3 {
                        title: "当前章事实".to_string(),
                        content: "必须写到第0章".to_string(),
                        category: "geography".to_string(),
                        confidence: "high".to_string(),
                        importance: "high".to_string(),
                    }],
                    ..Default::default()
                },
            },
        ));
        let (service, dir) = create_services_with_model(model).await;
        let book = save_local_txt_book(
            &service,
            "reader-a",
            "chapter.txt",
            "第一章 开场
当前章节正文。",
        )
        .await;

        let response = service
            .generate_current_chapter("reader-a", &book, 0, AiBookGenerationMode::Auto)
            .await
            .unwrap();

        assert_eq!(response.chapter.chapter_index, 0);
        assert_eq!(response.chapter.chapter_title.as_deref(), Some("第一章 开场"));
        assert_eq!(response.chapter.digest.as_ref().map(|d| d.chapter_index), Some(0));
        assert_eq!(response.chapter.digest.as_ref().map(|d| d.chapter_title.as_str()), Some("第一章 开场"));

        let saved = service
            .ai_book_service
            .get_or_create_v3("reader-a", &book.book_url, None, None)
            .await
            .unwrap();
        assert_eq!(saved.processed_chapter_index, Some(0));
        assert_eq!(saved.processed_chapter_title.as_deref(), Some("第一章 开场"));
        assert_eq!(saved.chapter_digests.len(), 1);
        assert_eq!(saved.chapter_digests[0].chapter_index, 0);
        assert_eq!(saved.chapter_digests[0].chapter_title, "第一章 开场");
        assert!(saved.knowledge_facts.iter().any(|fact| fact.title == "当前章事实"));

        let _ = fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn ai_book_v3_combined_current_chapter_saves_digest_and_patch() {
        let model = Arc::new(FakeChapterGenerationModel::with_combined(
            0,
            AiBookCombinedChapterGenerationV3 {
                chapter_digest: AiBookChapterDigestCandidateV3 {
                    chapter_index: 0,
                    chapter_title: "第一章 开场".to_string(),
                    summary: "局势变化".to_string(),
                    key_points: vec!["主角进城".to_string()],
                    has_important_changes: true,
                },
                patch: AiBookKnowledgePatchV3 {
                    chapter_index: 0,
                    summary: Some("局势变化".to_string()),
                    knowledge_facts: vec![crate::model::ai_book_generation::AiBookKnowledgeFactPatchV3 {
                        title: "城门已封锁".to_string(),
                        content: "全城戒严".to_string(),
                        category: "geography".to_string(),
                        confidence: "high".to_string(),
                        importance: "high".to_string(),
                    }],
                    characters: vec![crate::model::ai_book_generation::AiBookCharacterPatchV3 {
                        name: "张羽".to_string(),
                        aliases: vec![],
                        status: Some("入城".to_string()),
                        faction: None,
                        location: None,
                        description: Some("主角现身".to_string()),
                        last_seen_chapter: None,
                    }],
                    ..Default::default()
                },
            },
        ));
        let (service, dir) = create_services_with_model(model).await;
        let book = save_local_txt_book(
            &service,
            "reader-a",
            "chapter.txt",
            "第一章 开场\n张羽进入城门，发现全城戒严。",
        )
        .await;

        let response = service
            .generate_current_chapter("reader-a", &book, 0, AiBookGenerationMode::Auto)
            .await
            .unwrap();

        assert_eq!(response.memory.summary.current, "局势变化");
        assert_eq!(response.chapter.generation_status, "cached");
        assert_eq!(response.chapter.digest.as_ref().map(|d| d.summary.as_str()), Some("局势变化"));
        assert_eq!(response.chapter.chapter_title.as_deref(), Some("第一章 开场"));
        assert!(response.memory.knowledge_facts.iter().any(|fact| fact.title == "城门已封锁"));

        let saved = service
            .ai_book_service
            .get_or_create_v3("reader-a", &book.book_url, None, None)
            .await
            .unwrap();
        assert_eq!(saved.processed_chapter_index, Some(0));
        assert_eq!(saved.processed_chapter_title.as_deref(), Some("第一章 开场"));
        assert_eq!(saved.chapter_digests.len(), 1);
        assert_eq!(saved.chapter_digests[0].summary, "局势变化");
        assert!(saved.characters.iter().any(|character| character.name == "张羽"));
        assert!(saved.knowledge_facts.iter().any(|fact| fact.title == "城门已封锁"));

        let _ = fs::remove_dir_all(dir).await;
    }
}
