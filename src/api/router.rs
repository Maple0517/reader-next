use crate::api::{handlers, AppState};
use axum::{
    extract::DefaultBodyLimit,
    routing::{any, get, post},
    Router,
};
use std::path::PathBuf;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

const AI_BOOK_MEMORY_ROUTE: &str = "/reader3/ai/book/memory";
const AI_BOOK_CHAPTER_MEMORY_ROUTE: &str = "/reader3/ai/book/chapter-memory";
const AI_BOOK_MEMORY_RESET_ROUTE: &str = "/reader3/ai/book/memory/reset";
const AI_BOOK_ENABLED_ROUTE: &str = "/reader3/ai/book/enabled";
const AI_BOOK_CHAPTER_GENERATE_ROUTE: &str = "/reader3/ai/book/chapter-memory/generate";
const AI_BOOK_MAP_GENERATE_ROUTE: &str = "/reader3/ai/book/map/generate";
const AI_BOOK_CATCHUP_START_ROUTE: &str = "/reader3/ai/book/catchup/start";
const AI_BOOK_CATCHUP_STATUS_ROUTE: &str = "/reader3/ai/book/catchup/status";
const AI_BOOK_CATCHUP_CANCEL_ROUTE: &str = "/reader3/ai/book/catchup/cancel";
const AI_MODEL_CONFIG_ROUTE: &str = "/reader3/ai/model/config";
const AI_CHAPTER_SUMMARY_ROUTE: &str = "/reader3/ai/chapter-summary";
const AI_CHAPTER_SUMMARY_GENERATE_ROUTE: &str = "/reader3/ai/chapter-summary/generate";
const AI_CHAPTER_SUMMARY_CONFIG_ROUTE: &str = "/reader3/ai/chapter-summary/config";
const AI_PROXY_ROUTE: &str = "/reader3/ai/proxy";
const AI_PROXY_IMAGE_ROUTE: &str = "/reader3/ai/proxy/image";

// V4 API routes
const V4_MEMORY_ROUTE: &str = "/api/books/v4/memory";
const V4_CHARACTERS_ROUTE: &str = "/api/books/v4/characters";
const V4_CHARACTER_CARD_ROUTE: &str = "/api/books/v4/characters/:character_id";
const V4_CHAPTER_MEMORY_ROUTE: &str = "/api/books/v4/chapter-memory";
const V4_MEMORY_STATUS_ROUTE: &str = "/api/books/v4/memory/status";
const V4_MEMORY_RESET_ROUTE: &str = "/api/books/v4/memory/reset";
const V4_ENABLED_ROUTE: &str = "/api/books/v4/enabled";
const V4_CHAPTER_GENERATE_ROUTE: &str = "/api/books/v4/chapter-memory/generate";
const V4_CATCHUP_START_ROUTE: &str = "/api/books/v4/catchup/start";
const V4_CATCHUP_STATUS_ROUTE: &str = "/api/books/v4/catchup/status";
const V4_CATCHUP_CANCEL_ROUTE: &str = "/api/books/v4/catchup/cancel";
const V4_RELATIONSHIPS_ROUTE: &str = "/api/books/v4/relationships";
const V4_CHARACTER_RELATIONSHIPS_ROUTE: &str =
    "/api/books/v4/characters/:character_id/relationships";
const V4_IDENTITY_LINKS_ROUTE: &str = "/api/books/v4/identity-links";
const V4_CHARACTER_IDENTITY_ROUTE: &str = "/api/books/v4/characters/:character_id/identity";
const V4_MERGE_OPERATIONS_ROUTE: &str = "/api/books/v4/merge-operations";
const V4_KNOWLEDGE_ROUTE: &str = "/api/books/v4/knowledge";
const V4_KNOWLEDGE_CARD_ROUTE: &str = "/api/books/v4/knowledge/cards/:card_id";
const V4_KNOWLEDGE_CATEGORY_ROUTE: &str = "/api/books/v4/knowledge/categories/:category";
const V4_MAP_ROUTE: &str = "/api/books/v4/map";
const V4_MAP_PLACES_ROUTE: &str = "/api/books/v4/map/places";
const V4_MAP_PLACE_DETAIL_ROUTE: &str = "/api/books/v4/map/places/:place_id";
const V4_MAP_GRAPH_ROUTE: &str = "/api/books/v4/map/graph";
const V4_MAP_LAYOUT_ROUTE: &str = "/api/books/v4/map/layout";
const V4_MAP_CONFLICTS_ROUTE: &str = "/api/books/v4/map/conflicts";
const V4_QUALITY_ROUTE: &str = "/api/books/v4/quality";
const V4_QUALITY_QUARANTINE_ROUTE: &str = "/api/books/v4/quality/quarantine";
const V4_QUALITY_QUARANTINE_ACTION_ROUTE: &str =
    "/api/books/v4/quality/quarantine/:claim_id/action";
const V4_QUALITY_AUDIT_RUNS_ROUTE: &str = "/api/books/v4/quality/audit-runs";
const V4_QUALITY_AUDIT_FINDINGS_ROUTE: &str = "/api/books/v4/quality/audit-findings";
const V4_QUALITY_AUDIT_FINDING_ACTION_ROUTE: &str =
    "/api/books/v4/quality/audit-findings/:finding_id/action";
const V4_QUALITY_CORRECTIONS_ROUTE: &str = "/api/books/v4/quality/corrections";
const V4_QUALITY_CORRECTION_APPLY_ROUTE: &str =
    "/api/books/v4/quality/corrections/:correction_id/apply";
const V4_QUALITY_REPROCESS_JOBS_ROUTE: &str = "/api/books/v4/quality/reprocess-jobs";
const V4_QUALITY_REPROCESS_JOB_CANCEL_ROUTE: &str =
    "/api/books/v4/quality/reprocess-jobs/:job_id/cancel";
const V4_QUALITY_PROMPT_REGRESSION_RUNS_ROUTE: &str =
    "/api/books/v4/quality/prompt-regression-runs";
const V4_QUALITY_PROMPT_REGRESSION_RESULTS_ROUTE: &str =
    "/api/books/v4/quality/prompt-regression-runs/:run_id/results";

pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(handlers::health))
        .route(
            "/reader3/getBookSource",
            get(handlers::get_book_source).post(handlers::get_book_source),
        )
        .route(
            "/reader3/getBookSources",
            get(handlers::get_book_sources).post(handlers::get_book_sources),
        )
        .route(
            "/reader3/getDefaultBookSourceOwner",
            get(handlers::get_default_book_source_owner),
        )
        .route(
            "/reader3/loginBookSource",
            post(handlers::login_book_source),
        )
        .route(
            "/reader3/getExploreKinds",
            post(handlers::get_explore_kinds),
        )
        .route(
            "/reader3/testBookSources",
            post(handlers::test_book_sources),
        )
        .route(
            "/reader3/deleteInvalidBookSources",
            post(handlers::delete_invalid_book_sources),
        )
        .route("/reader3/bookSourceProxy", any(handlers::book_source_proxy))
        .route(
            "/reader3/bookSourceClientLog",
            any(handlers::book_source_client_log),
        )
        .route("/reader3/saveBookSource", post(handlers::save_book_source))
        .route(
            "/reader3/saveBookSources",
            post(handlers::save_book_sources),
        )
        .route(
            "/reader3/deleteBookSource",
            post(handlers::delete_book_source),
        )
        .route(
            "/reader3/deleteBookSources",
            post(handlers::delete_book_sources),
        )
        .route(
            "/reader3/deleteAllBookSources",
            post(handlers::delete_all_book_sources),
        )
        .route(
            "/reader3/setAsDefaultBookSources",
            post(handlers::set_as_default_book_sources),
        )
        .route(
            "/reader3/readRemoteSourceFile",
            post(handlers::read_remote_source_file),
        )
        .route("/reader3/readSourceFile", post(handlers::read_source_file))
        .route(
            "/reader3/searchBook",
            get(handlers::search_book).post(handlers::search_book),
        )
        .route(
            "/reader3/exploreBook",
            get(handlers::explore_book).post(handlers::explore_book),
        )
        .route(
            "/reader3/exploreBookGlobal",
            post(handlers::explore_book_global),
        )
        .route(
            "/reader3/searchBookMulti",
            get(handlers::search_book_multi).post(handlers::search_book_multi),
        )
        .route("/reader3/getBookshelf", get(handlers::get_bookshelf))
        .route(
            "/reader3/getShelfBook",
            get(handlers::get_shelf_book).post(handlers::get_shelf_book),
        )
        .route(
            "/reader3/getShelfBookWithCacheInfo",
            get(handlers::get_shelf_book_with_cache_info),
        )
        .route(
            "/reader3/getBookGroups",
            get(handlers::get_book_groups).post(handlers::get_book_groups),
        )
        .route("/reader3/saveBookGroup", post(handlers::save_book_group))
        .route(
            "/reader3/saveBookGroupOrder",
            post(handlers::save_book_group_order),
        )
        .route(
            "/reader3/deleteBookGroup",
            post(handlers::delete_book_group),
        )
        .route(
            "/reader3/saveBookGroupId",
            post(handlers::save_book_group_id),
        )
        .route(
            "/reader3/addBookGroupMulti",
            post(handlers::add_book_group_multi),
        )
        .route(
            "/reader3/removeBookGroupMulti",
            post(handlers::remove_book_group_multi),
        )
        .route("/reader3/uploadTxtBook", post(handlers::upload_txt_book))
        .route("/reader3/uploadEpubBook", post(handlers::upload_epub_book))
        .route("/reader3/uploadMobiBook", post(handlers::upload_mobi_book))
        .route("/reader3/uploadPdfBook", post(handlers::upload_pdf_book))
        .route("/reader3/saveBook", post(handlers::save_book))
        .route("/reader3/saveBooks", post(handlers::save_books))
        .route("/reader3/setBookSource", post(handlers::set_book_source))
        .route("/reader3/deleteBook", post(handlers::delete_book))
        .route("/reader3/deleteBooks", post(handlers::delete_books))
        .route(
            "/reader3/saveBookProgress",
            post(handlers::save_book_progress),
        )
        .route(
            "/reader3/getBookInfo",
            get(handlers::get_book_info).post(handlers::get_book_info),
        )
        .route(
            "/reader3/getChapterList",
            get(handlers::get_chapter_list).post(handlers::get_chapter_list),
        )
        .route(
            "/reader3/getBookContent",
            get(handlers::get_book_content).post(handlers::get_book_content),
        )
        .route(
            "/reader3/deleteBookCache",
            post(handlers::delete_book_cache),
        )
        .route(
            "/reader3/getInvalidBookSources",
            post(handlers::get_invalid_book_sources),
        )
        .route(
            "/reader3/cacheBookSSE",
            get(handlers::cache_book_sse).post(handlers::cache_book_sse),
        )
        .route(
            "/reader3/searchBookMultiSSE",
            get(handlers::search_book_multi_sse),
        )
        .route(
            "/reader3/searchBookSourceSSE",
            get(handlers::search_book_source_sse),
        )
        .route(
            "/reader3/getAvailableBookSource",
            get(handlers::get_available_book_source).post(handlers::get_available_book_source),
        )
        .route(
            "/reader3/getAvailableBookSourceSSE",
            get(handlers::get_available_book_source_sse),
        )
        .route(
            "/reader3/bookSourceDebugSSE",
            get(handlers::book_source_debug_sse),
        )
        .route("/reader3/cover", get(handlers::get_book_cover))
        .route("/reader3/getRssSources", get(handlers::get_rss_sources))
        .route("/reader3/saveRssSource", post(handlers::save_rss_source))
        .route("/reader3/saveRssSources", post(handlers::save_rss_sources))
        .route(
            "/reader3/deleteRssSource",
            post(handlers::delete_rss_source),
        )
        .route(
            "/reader3/deleteRssSources",
            post(handlers::delete_rss_sources),
        )
        .route(
            "/reader3/readRemoteRssSourceFile",
            post(handlers::read_remote_rss_source_file),
        )
        .route(
            "/reader3/readRssSourceFile",
            post(handlers::read_rss_source_file),
        )
        .route(
            "/reader3/getRssArticles",
            get(handlers::get_rss_articles).post(handlers::get_rss_articles),
        )
        .route(
            "/reader3/getRssContent",
            get(handlers::get_rss_content).post(handlers::get_rss_content),
        )
        .route("/reader3/getBookmarks", get(handlers::get_bookmarks))
        .route("/reader3/saveBookmark", post(handlers::save_bookmark))
        .route("/reader3/saveBookmarks", post(handlers::save_bookmarks))
        .route("/reader3/deleteBookmark", post(handlers::delete_bookmark))
        .route("/reader3/deleteBookmarks", post(handlers::delete_bookmarks))
        .route(AI_BOOK_MEMORY_ROUTE, get(handlers::get_ai_book_memory))
        .route(
            AI_BOOK_CHAPTER_MEMORY_ROUTE,
            get(handlers::get_ai_book_chapter_memory),
        )
        .route(
            AI_BOOK_MEMORY_RESET_ROUTE,
            post(handlers::reset_ai_book_memory),
        )
        .route(AI_BOOK_ENABLED_ROUTE, post(handlers::set_ai_book_enabled))
        .route(
            AI_BOOK_CHAPTER_GENERATE_ROUTE,
            post(handlers::generate_ai_book_chapter_memory),
        )
        .route(
            AI_BOOK_MAP_GENERATE_ROUTE,
            post(handlers::generate_ai_book_map),
        )
        .route(
            AI_BOOK_CATCHUP_START_ROUTE,
            post(handlers::start_ai_book_catchup),
        )
        .route(
            AI_BOOK_CATCHUP_STATUS_ROUTE,
            get(handlers::get_ai_book_catchup_status),
        )
        .route(
            AI_BOOK_CATCHUP_CANCEL_ROUTE,
            post(handlers::cancel_ai_book_catchup),
        )
        .route(
            AI_MODEL_CONFIG_ROUTE,
            get(handlers::get_ai_model_config).post(handlers::save_ai_model_config),
        )
        .route(AI_CHAPTER_SUMMARY_ROUTE, get(handlers::get_chapter_summary))
        .route(
            AI_CHAPTER_SUMMARY_GENERATE_ROUTE,
            post(handlers::generate_chapter_summary),
        )
        .route(
            AI_CHAPTER_SUMMARY_CONFIG_ROUTE,
            get(handlers::get_chapter_summary_config).post(handlers::save_chapter_summary_config),
        )
        .route(AI_PROXY_ROUTE, post(handlers::ai_proxy))
        .route(AI_PROXY_IMAGE_ROUTE, post(handlers::ai_proxy_image))
        // V4 API routes
        .route(V4_MEMORY_ROUTE, get(handlers::get_v4_memory))
        .route(V4_CHARACTERS_ROUTE, get(handlers::get_v4_characters))
        .route(
            V4_CHARACTER_CARD_ROUTE,
            get(handlers::get_v4_character_card),
        )
        .route(
            V4_CHAPTER_MEMORY_ROUTE,
            get(handlers::get_v4_chapter_memory),
        )
        .route(V4_MEMORY_STATUS_ROUTE, get(handlers::get_v4_memory_status))
        .route(V4_MEMORY_RESET_ROUTE, post(handlers::post_v4_memory_reset))
        .route(V4_ENABLED_ROUTE, post(handlers::post_v4_enabled))
        .route(
            V4_CHAPTER_GENERATE_ROUTE,
            post(handlers::post_v4_chapter_generate),
        )
        .route(
            V4_CATCHUP_START_ROUTE,
            post(handlers::post_v4_catchup_start),
        )
        .route(
            V4_CATCHUP_STATUS_ROUTE,
            get(handlers::get_v4_catchup_status),
        )
        .route(
            V4_CATCHUP_CANCEL_ROUTE,
            post(handlers::post_v4_catchup_cancel),
        )
        .route(V4_RELATIONSHIPS_ROUTE, get(handlers::get_v4_relationships))
        .route(
            V4_CHARACTER_RELATIONSHIPS_ROUTE,
            get(handlers::get_v4_character_relationships),
        )
        .route(
            V4_IDENTITY_LINKS_ROUTE,
            get(handlers::get_v4_identity_links),
        )
        .route(
            V4_CHARACTER_IDENTITY_ROUTE,
            get(handlers::get_v4_character_identity),
        )
        .route(
            V4_MERGE_OPERATIONS_ROUTE,
            get(handlers::get_v4_merge_operations),
        )
        .route(V4_KNOWLEDGE_ROUTE, get(handlers::get_v4_knowledge))
        .route(
            V4_KNOWLEDGE_CARD_ROUTE,
            get(handlers::get_v4_knowledge_card),
        )
        .route(
            V4_KNOWLEDGE_CATEGORY_ROUTE,
            get(handlers::get_v4_knowledge_category),
        )
        .route(V4_MAP_ROUTE, get(handlers::get_v4_map))
        .route(V4_MAP_PLACES_ROUTE, get(handlers::get_v4_map_places))
        .route(
            V4_MAP_PLACE_DETAIL_ROUTE,
            get(handlers::get_v4_map_place_detail),
        )
        .route(V4_MAP_GRAPH_ROUTE, get(handlers::get_v4_map_graph))
        .route(V4_MAP_LAYOUT_ROUTE, get(handlers::get_v4_map_layout))
        .route(V4_MAP_CONFLICTS_ROUTE, get(handlers::get_v4_map_conflicts))
        .route(V4_QUALITY_ROUTE, get(handlers::get_v4_quality))
        .route(
            V4_QUALITY_QUARANTINE_ROUTE,
            get(handlers::get_v4_quality_quarantine),
        )
        .route(
            V4_QUALITY_QUARANTINE_ACTION_ROUTE,
            post(handlers::post_v4_quality_quarantine_action),
        )
        .route(
            V4_QUALITY_AUDIT_RUNS_ROUTE,
            get(handlers::get_v4_quality_audit_runs).post(handlers::post_v4_quality_audit_runs),
        )
        .route(
            V4_QUALITY_AUDIT_FINDINGS_ROUTE,
            get(handlers::get_v4_quality_audit_findings),
        )
        .route(
            V4_QUALITY_AUDIT_FINDING_ACTION_ROUTE,
            post(handlers::post_v4_quality_audit_finding_action),
        )
        .route(
            V4_QUALITY_CORRECTIONS_ROUTE,
            get(handlers::get_v4_quality_corrections).post(handlers::post_v4_quality_corrections),
        )
        .route(
            V4_QUALITY_CORRECTION_APPLY_ROUTE,
            post(handlers::post_v4_quality_correction_apply),
        )
        .route(
            V4_QUALITY_REPROCESS_JOBS_ROUTE,
            get(handlers::get_v4_quality_reprocess_jobs)
                .post(handlers::post_v4_quality_reprocess_jobs),
        )
        .route(
            V4_QUALITY_REPROCESS_JOB_CANCEL_ROUTE,
            post(handlers::post_v4_quality_reprocess_job_cancel),
        )
        .route(
            V4_QUALITY_PROMPT_REGRESSION_RUNS_ROUTE,
            get(handlers::get_v4_quality_prompt_regression_runs)
                .post(handlers::post_v4_quality_prompt_regression_runs),
        )
        .route(
            V4_QUALITY_PROMPT_REGRESSION_RESULTS_ROUTE,
            get(handlers::get_v4_quality_prompt_regression_results),
        )
        .route("/reader3/getReplaceRules", get(handlers::get_replace_rules))
        .route(
            "/reader3/saveReplaceRule",
            post(handlers::save_replace_rule),
        )
        .route(
            "/reader3/saveReplaceRules",
            post(handlers::save_replace_rules),
        )
        .route(
            "/reader3/deleteReplaceRule",
            post(handlers::delete_replace_rule),
        )
        .route(
            "/reader3/deleteReplaceRules",
            post(handlers::delete_replace_rules),
        )
        .route(
            "/reader3/getWebdavFileList",
            get(handlers::get_webdav_file_list),
        )
        .route("/reader3/getWebdavFile", get(handlers::get_webdav_file))
        .route(
            "/reader3/uploadFileToWebdav",
            post(handlers::upload_file_to_webdav),
        )
        .route(
            "/reader3/deleteWebdavFile",
            post(handlers::delete_webdav_file),
        )
        .route(
            "/reader3/deleteWebdavFileList",
            post(handlers::delete_webdav_file_list),
        )
        .route("/reader3/webdav/*path", any(handlers::webdav_handler))
        .route("/reader3/login", post(handlers::login))
        .route("/reader3/logout", post(handlers::logout))
        .route("/reader3/getUserInfo", get(handlers::get_user_info))
        .route(
            "/reader3/getVersionUpdate",
            get(handlers::get_version_update),
        )
        .route(
            "/reader3/dismissVersionUpdate",
            post(handlers::dismiss_version_update),
        )
        .route("/reader3/saveUserConfig", post(handlers::save_user_config))
        .route("/reader3/getUserConfig", get(handlers::get_user_config))
        .route("/reader3/getUserList", get(handlers::get_user_list))
        .route("/reader3/deleteUsers", post(handlers::delete_users))
        .route("/reader3/addUser", post(handlers::add_user))
        .route("/reader3/resetPassword", post(handlers::reset_password))
        .route("/reader3/changePassword", post(handlers::change_password))
        .route("/reader3/updateUser", post(handlers::update_user))
        .route("/reader3/uploadFile", post(handlers::upload_file))
        .route("/reader3/deleteFile", post(handlers::delete_file))
        .route("/reader3/getTxtTocRules", get(handlers::get_txt_toc_rules))
        .with_state(state.clone());

    let web_root = state.config.web_root.clone();
    let assets_root = state.config.assets_dir.clone();
    let web_assets_root = PathBuf::from(&web_root).join("assets");

    let static_web = Router::new()
        .nest_service(
            "/assets",
            ServeDir::new(web_assets_root).not_found_service(ServeDir::new(assets_root)),
        )
        .fallback_service(ServeDir::new(web_root));

    Router::new()
        .merge(api)
        .merge(static_web)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid::default()))
        .layer(CorsLayer::very_permissive())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::config::AppConfig;
    use crate::service::ai_book_catchup_service::AiBookCatchupService;
    use crate::service::ai_book_generation_service::AiBookGenerationService;
    use crate::service::ai_book_service::AiBookService;
    use crate::service::ai_model_service::AiModelService;
    use crate::service::book_group_service::BookGroupService;
    use crate::service::book_service::BookService;
    use crate::service::book_source_service::BookSourceService;
    use crate::service::chapter_summary_service::ChapterSummaryService;
    use crate::service::json_document_service::JsonDocumentService;
    use crate::service::local_epub_book::LocalEpubBookService;
    use crate::service::local_mobi_book::LocalMobiBookService;
    use crate::service::local_pdf_book::LocalPdfBookService;
    use crate::service::local_txt_book::LocalTxtBookService;
    use crate::service::update_service::UpdateService;
    use crate::service::user_service::UserService;
    use crate::storage::cache::file_cache::FileCache;
    use crate::storage::db;
    use crate::storage::db::repo::BookSourceRepo;
    use crate::util::crypto::random_string;
    use reqwest::{Client, Method, StatusCode};
    use std::path::PathBuf;
    use std::sync::Arc;

    async fn create_test_state() -> (AppState, PathBuf) {
        let dir = std::env::temp_dir().join(format!("reader-ai-book-router-{}", random_string(8)));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        let cfg = AppConfig {
            storage_dir: dir.to_string_lossy().to_string(),
            assets_dir: dir.join("assets").to_string_lossy().to_string(),
            web_root: dir.join("web").to_string_lossy().to_string(),
            database_url,
            ..AppConfig::default()
        };
        let http =
            crate::crawler::http_client::HttpClient::new(cfg.request_timeout_secs, None).unwrap();
        let parser = crate::parser::rule_engine::RuleEngine::new().unwrap();
        let cache = FileCache::new(format!("{}/cache", cfg.storage_dir));
        let book_service = Arc::new(BookService::new(http, parser, cache, &cfg.storage_dir));
        let book_source_service = Arc::new(BookSourceService::new(
            BookSourceRepo::new(pool.clone()),
            &cfg.storage_dir,
        ));
        let local_txt_book_service = Arc::new(LocalTxtBookService::new(&cfg.storage_dir));
        let local_epub_book_service = Arc::new(LocalEpubBookService::new(&cfg.storage_dir));
        let local_mobi_book_service = Arc::new(LocalMobiBookService::new(&cfg.storage_dir));
        let local_pdf_book_service = Arc::new(LocalPdfBookService::new(&cfg.storage_dir));
        let json_document_service =
            Arc::new(JsonDocumentService::new(pool.clone(), &cfg.storage_dir));
        let user_service = Arc::new(UserService::new(cfg.clone(), pool.clone()));
        user_service.migrate_legacy_users_from_json().await.unwrap();
        let book_group_service = Arc::new(BookGroupService::new(json_document_service.clone()));
        let ai_model_service = Arc::new(AiModelService::new(
            json_document_service.clone(),
            &cfg.storage_dir,
        ));
        let ai_book_service = Arc::new(AiBookService::new(pool.clone(), &cfg.storage_dir));
        let ai_book_generation_service =
            Arc::new(AiBookGenerationService::new_with_ai_model_service(
                ai_book_service.clone(),
                book_service.clone(),
                book_source_service.clone(),
                local_txt_book_service.clone(),
                ai_model_service.clone(),
            ));
        let ai_book_catchup_service = Arc::new(AiBookCatchupService::new());
        let chapter_summary_service =
            Arc::new(ChapterSummaryService::new(json_document_service.clone()));
        let update_service = Arc::new(
            UpdateService::new(
                json_document_service.clone(),
                cfg.request_timeout_secs,
                format!("v{}", env!("CARGO_PKG_VERSION")),
            )
            .unwrap(),
        );
        let state = AppState {
            config: cfg,
            book_service,
            book_source_service,
            user_service,
            book_group_service,
            local_txt_book_service,
            local_epub_book_service,
            local_mobi_book_service,
            local_pdf_book_service,
            json_document_service,
            ai_book_service,
            ai_book_generation_service,
            ai_book_catchup_service,
            ai_model_service,
            chapter_summary_service,
            update_service,
            pool: pool.clone(),
        };
        (state, dir)
    }

    #[tokio::test]
    async fn ai_book_v3_routes_are_registered_without_legacy_aliases() {
        let (state, dir) = create_test_state().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(state)).await.unwrap();
        });
        let client = Client::new();
        let base_url = format!("http://{}", addr);

        for (method, path) in [
            (Method::POST, "/reader3/saveAiBookMemory"),
            (Method::GET, "/reader3/getAiBookMemory"),
            (Method::POST, "/reader3/aiBookCatchup/pause"),
        ] {
            let response = client
                .request(method.clone(), format!("{base_url}{path}"))
                .send()
                .await
                .unwrap();
            assert!(matches!(
                response.status(),
                StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
            ));
        }

        for (method, path) in [
            (Method::GET, AI_BOOK_MEMORY_ROUTE),
            (Method::GET, AI_BOOK_CHAPTER_MEMORY_ROUTE),
            (Method::POST, AI_BOOK_MEMORY_RESET_ROUTE),
            (Method::POST, AI_BOOK_ENABLED_ROUTE),
            (Method::POST, AI_BOOK_CHAPTER_GENERATE_ROUTE),
            (Method::POST, AI_BOOK_MAP_GENERATE_ROUTE),
            (Method::POST, AI_BOOK_CATCHUP_START_ROUTE),
            (Method::GET, AI_BOOK_CATCHUP_STATUS_ROUTE),
            (Method::POST, AI_BOOK_CATCHUP_CANCEL_ROUTE),
        ] {
            let response = client
                .request(method.clone(), format!("{base_url}{path}"))
                .send()
                .await
                .unwrap();
            assert_ne!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
            assert_ne!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} {path}"
            );
        }

        let wrong_method_memory = client
            .request(Method::POST, format!("{base_url}{AI_BOOK_MEMORY_ROUTE}"))
            .send()
            .await
            .unwrap();
        assert_eq!(wrong_method_memory.status(), StatusCode::METHOD_NOT_ALLOWED);

        let wrong_method_enabled = client
            .request(Method::GET, format!("{base_url}{AI_BOOK_ENABLED_ROUTE}"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            wrong_method_enabled.status(),
            StatusCode::METHOD_NOT_ALLOWED
        );

        server.abort();
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn v4_routes_are_registered() {
        let (state, dir) = create_test_state().await;
        // Initialize V4 schema
        db::v4::init_v4(&state.pool).await.unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(state)).await.unwrap();
        });
        let client = Client::new();
        let base_url = format!("http://{}", addr);

        // All V4 GET routes should respond (not 404)
        // Note: parameterized routes use concrete values in the URL
        for (method, path) in [
            (Method::GET, V4_MEMORY_ROUTE.to_string()),
            (Method::GET, V4_CHARACTERS_ROUTE.to_string()),
            (Method::GET, V4_CHAPTER_MEMORY_ROUTE.to_string()),
            (Method::GET, V4_MEMORY_STATUS_ROUTE.to_string()),
            (Method::GET, V4_CATCHUP_STATUS_ROUTE.to_string()),
            (Method::GET, V4_RELATIONSHIPS_ROUTE.to_string()),
            (
                Method::GET,
                V4_CHARACTER_RELATIONSHIPS_ROUTE.replace(":character_id", "test-char"),
            ),
            (Method::GET, "/api/books/v4/identity-links".to_string()),
            (
                Method::GET,
                "/api/books/v4/characters/test-char/identity".to_string(),
            ),
            (Method::GET, "/api/books/v4/merge-operations".to_string()),
            (Method::GET, "/api/books/v4/knowledge".to_string()),
            (
                Method::GET,
                "/api/books/v4/knowledge/cards/test-card".to_string(),
            ),
            (
                Method::GET,
                "/api/books/v4/knowledge/categories/history".to_string(),
            ),
            (Method::GET, V4_MAP_ROUTE.to_string()),
            (Method::GET, V4_MAP_PLACES_ROUTE.to_string()),
            (
                Method::GET,
                V4_MAP_PLACE_DETAIL_ROUTE.replace(":place_id", "test-place"),
            ),
            (Method::GET, V4_MAP_GRAPH_ROUTE.to_string()),
            (Method::GET, V4_MAP_LAYOUT_ROUTE.to_string()),
            (Method::GET, V4_MAP_CONFLICTS_ROUTE.to_string()),
            (Method::GET, "/api/books/v4/quality".to_string()),
            (Method::GET, "/api/books/v4/quality/quarantine".to_string()),
            (Method::GET, "/api/books/v4/quality/audit-runs".to_string()),
            (
                Method::GET,
                "/api/books/v4/quality/audit-findings".to_string(),
            ),
            (Method::GET, "/api/books/v4/quality/corrections".to_string()),
            (
                Method::GET,
                "/api/books/v4/quality/reprocess-jobs".to_string(),
            ),
            (
                Method::GET,
                "/api/books/v4/quality/prompt-regression-runs".to_string(),
            ),
            (
                Method::GET,
                "/api/books/v4/quality/prompt-regression-runs/run-quality/results".to_string(),
            ),
        ] {
            let response = client
                .request(method.clone(), format!("{base_url}{path}"))
                .send()
                .await
                .unwrap();
            assert_ne!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
            assert_ne!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} {path}"
            );
        }

        // All V4 POST routes should respond (not 404)
        for (method, path) in [
            (Method::POST, V4_MEMORY_RESET_ROUTE),
            (Method::POST, V4_ENABLED_ROUTE),
            (Method::POST, V4_CHAPTER_GENERATE_ROUTE),
            (Method::POST, V4_CATCHUP_START_ROUTE),
            (Method::POST, V4_CATCHUP_CANCEL_ROUTE),
            (
                Method::POST,
                "/api/books/v4/quality/quarantine/workflow-quality/action",
            ),
            (Method::POST, "/api/books/v4/quality/audit-runs"),
            (
                Method::POST,
                "/api/books/v4/quality/audit-findings/finding-quality/action",
            ),
            (Method::POST, "/api/books/v4/quality/corrections"),
            (
                Method::POST,
                "/api/books/v4/quality/corrections/correction-quality/apply",
            ),
            (Method::POST, "/api/books/v4/quality/reprocess-jobs"),
            (
                Method::POST,
                "/api/books/v4/quality/reprocess-jobs/job-quality/cancel",
            ),
            (Method::POST, "/api/books/v4/quality/prompt-regression-runs"),
        ] {
            let response = client
                .request(method.clone(), format!("{base_url}{path}"))
                .send()
                .await
                .unwrap();
            assert_ne!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
            assert_ne!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} {path}"
            );
        }

        // Wrong method checks
        let wrong_method_memory = client
            .request(Method::POST, format!("{base_url}{V4_MEMORY_ROUTE}"))
            .send()
            .await
            .unwrap();
        assert_eq!(wrong_method_memory.status(), StatusCode::METHOD_NOT_ALLOWED);

        let wrong_method_reset = client
            .request(Method::GET, format!("{base_url}{V4_MEMORY_RESET_ROUTE}"))
            .send()
            .await
            .unwrap();
        assert_eq!(wrong_method_reset.status(), StatusCode::METHOD_NOT_ALLOWED);

        server.abort();
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn v4_quality_api_returns_overview_and_group_lists() {
        let (state, dir) = create_test_state().await;
        db::v4::init_v4(&state.pool).await.unwrap();
        let book_url = "quality-book";
        let book_id = crate::util::hash::md5_hex(book_url);

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('chapter-quality-1', ?, 1, 'quality text', 'hash-quality', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('segment-quality-1', ?, 'chapter-quality-1', 'hash-quality', 0, datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span-quality-1', ?, 'chapter-quality-1', 'hash-quality', 'segment-quality-1', 0, 0, 12, 'quality evidence', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run-quality-1', ?, 'chapter-quality-1', 'extract', 'test-model', 'prompt-v1', 1, 'input', 'completed', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, subject_mention, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('claim-quality-1', ?, 1, 'property_update', '张三', 'realm', 'span-quality-1', 'run-quality-1', 0.4, 'high', 'quarantined', datetime('now'), datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO quarantined_claims (id, book_id, claim_id, reason_code, reason_text, suggested_action, status, priority, created_at, updated_at) VALUES ('workflow-quality', ?, 'claim-quality-1', 'low_confidence', 'needs review', 'accept', 'open', 5, datetime('now'), datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO quality_audit_runs (id, book_id, audit_type, scope_json, status, started_at, finished_at, summary_json) VALUES ('audit-quality', ?, 'duplicate_entities', '{}', 'completed', datetime('now'), datetime('now'), '{\"findingCount\":1}')")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO quality_audit_findings (id, book_id, audit_run_id, finding_type, severity, target_type, target_id, reason_code, reason_text, evidence_json, suggested_action, status, created_at) VALUES ('finding-quality', ?, 'audit-quality', 'duplicate_entity_candidate', 'medium', 'entity', 'entity-quality-1', 'alias_overlap', 'same alias', '{\"score\":0.8}', 'merge_entities', 'open', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO user_corrections (id, book_id, target_type, target_id, correction_type, correction_json, status, source, source_claim_id, source_span_id, created_by, created_at) VALUES ('correction-quality', ?, 'claim', 'claim-quality-1', 'reject_claim', '{}', 'validated', 'test', 'claim-quality-1', 'span-quality-1', 'api-test', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO quality_metrics (id, book_id, metric_type, metric_value, metric_json, measured_at) VALUES ('metric-quality-1', ?, 'quarantined_claim_count', 1, NULL, datetime('now')), ('metric-quality-2', ?, 'duplicate_entity_candidate_count', 1, NULL, datetime('now'))")
            .bind(&book_id)
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO reprocess_jobs (id, book_id, scope_type, scope_json, mode, status, requested_by, reason, dry_run, prompt_version, schema_version) VALUES ('job-quality', ?, 'claim', '{\"claimId\":\"claim-quality-1\"}', 'dry_run_compare', 'queued', 'api-test', 'quality smoke', 1, 'prompt-v1', '1')")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO prompt_regression_runs (id, book_id, prompt_version, schema_version, model, fixture_set, status, started_at, finished_at, summary_json) VALUES ('run-quality', ?, 'prompt-v1', '1', 'test-model', 'phase6', 'completed', datetime('now'), datetime('now'), '{\"total\":1,\"passed\":0}')")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO prompt_regression_results (id, run_id, case_id, case_name, domain, expected_json, actual_json, pass, diff_json, created_at) VALUES ('result-quality', 'run-quality', 'case-1', 'quality regression', 'quality', '{}', '{\"missing\":true}', 0, '{\"missing\":true}', datetime('now'))")
            .execute(&state.pool)
            .await
            .unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(state)).await.unwrap();
        });
        let client = Client::new();
        let base_url = format!("http://{}", addr);

        let overview: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/quality?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(overview["data"]["bookUrl"], book_url);
        assert_eq!(
            overview["data"]["qualityMetrics"]["quarantinedClaimCount"],
            1.0
        );
        assert_eq!(overview["data"]["auditRuns"].as_array().unwrap().len(), 1);
        assert_eq!(overview["data"]["findings"].as_array().unwrap().len(), 1);
        assert_eq!(
            overview["data"]["reprocessJobs"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            overview["data"]["promptRegressionRuns"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let quarantine: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/quality/quarantine?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(quarantine["data"]["total"], 1);
        assert_eq!(
            quarantine["data"]["items"][0]["workflow"]["reasonCode"],
            "low_confidence"
        );
        assert_eq!(
            quarantine["data"]["items"][0]["claim"]["primarySourceSpanId"],
            "span-quality-1"
        );

        let audit_runs: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/quality/audit-runs?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            audit_runs["data"]["auditRuns"][0]["auditType"],
            "duplicate_entities"
        );

        let findings: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/quality/audit-findings?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            findings["data"]["findings"][0]["suggestedAction"],
            "merge_entities"
        );

        let corrections: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/quality/corrections?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            corrections["data"]["corrections"][0]["correctionType"],
            "reject_claim"
        );

        let reprocess_jobs: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/quality/reprocess-jobs?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(reprocess_jobs["data"]["reprocessJobs"][0]["dryRun"], true);

        let prompt_runs: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/quality/prompt-regression-runs?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            prompt_runs["data"]["promptRegressionRuns"][0]["fixtureSet"],
            "phase6"
        );

        let prompt_results: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/quality/prompt-regression-runs/run-quality/results?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(prompt_results["data"]["results"][0]["caseId"], "case-1");
        assert_eq!(prompt_results["data"]["results"][0]["pass"], false);

        let apply_correction: serde_json::Value = client
            .post(format!(
                "{base_url}/api/books/v4/quality/corrections/correction-quality/apply"
            ))
            .json(&serde_json::json!({ "bookUrl": book_url }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(apply_correction["data"]["status"], "applied");

        let updated_overview: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/quality?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            updated_overview["data"]["qualityMetrics"]["rejectedClaimRate"],
            1.0
        );

        server.abort();
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn v4_quality_quarantine_action_accepts_workflow_id_path() {
        let (state, dir) = create_test_state().await;
        db::v4::init_v4(&state.pool).await.unwrap();
        let book_url = "quality-action-book";
        let book_id = crate::util::hash::md5_hex(book_url);

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('chapter-quality-action-1', ?, 1, 'quality text', 'hash-quality-action', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('segment-quality-action-1', ?, 'chapter-quality-action-1', 'hash-quality-action', 0, datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span-quality-action-1', ?, 'chapter-quality-action-1', 'hash-quality-action', 'segment-quality-action-1', 0, 0, 12, 'quality evidence', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run-quality-action-1', ?, 'chapter-quality-action-1', 'extract', 'test-model', 'prompt-v1', 1, 'input', 'completed', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, subject_mention, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('claim-quality-action-1', ?, 1, 'property_update', '张三', 'realm', 'span-quality-action-1', 'run-quality-action-1', 0.4, 'high', 'quarantined', datetime('now'), datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO quarantined_claims (id, book_id, claim_id, reason_code, reason_text, suggested_action, status, priority, created_at, updated_at) VALUES ('workflow-quality-action-1', ?, 'claim-quality-action-1', 'low_confidence', 'needs review', 'accept', 'open', 5, datetime('now'), datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(state)).await.unwrap();
        });
        let client = Client::new();
        let base_url = format!("http://{}", addr);

        let response: serde_json::Value = client
            .post(format!(
                "{base_url}/api/books/v4/quality/quarantine/workflow-quality-action-1/action"
            ))
            .json(&serde_json::json!({
                "bookUrl": book_url,
                "action": "ignore",
                "actor": "api-test"
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(response["data"]["kind"], "workflowUpdated");
        assert_eq!(response["data"]["workflow"]["status"], "ignored");

        server.abort();
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn v4_knowledge_api_returns_overview_detail_category_and_memory_count() {
        let (state, dir) = create_test_state().await;
        db::v4::init_v4(&state.pool).await.unwrap();
        let book_url = "knowledge-book";
        let book_id = crate::util::hash::md5_hex(book_url);
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('chapter1', ?, 1, 'text', 'hash', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('segment1', ?, 'chapter1', 'hash', 0, datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span1', ?, 'chapter1', 'hash', 'segment1', 0, 0, 10, 'evidence', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', ?, 'chapter1', 'extract', 'test', 'v1', 1, 'input', 'completed', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('claim1', ?, 1, 'knowledge_assertion', 'knowledge', 'span1', 'run1', 0.9, 'high', 'accepted', datetime('now'), datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        let knowledge_repo =
            crate::storage::db::v4::knowledge_repo::KnowledgeRepo::new(state.pool.clone());
        let card = knowledge_repo
            .find_or_create_card(
                &book_id,
                "history",
                "old-war",
                "Old War",
                Some("War summary"),
                0.9,
                0.8,
                1,
            )
            .await
            .unwrap();
        knowledge_repo
            .find_or_create_assertion(
                &book_id,
                &card.id,
                "claim1",
                "Old war happened.",
                "active",
                0.9,
                0.8,
                1,
            )
            .await
            .unwrap();
        knowledge_repo
            .find_or_create_assertion(
                &book_id,
                &card.id,
                "claim1",
                "Old rumor.",
                "rumor",
                0.6,
                0.4,
                1,
            )
            .await
            .unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(state)).await.unwrap();
        });
        let client = Client::new();
        let base_url = format!("http://{}", addr);

        let overview: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/knowledge?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(overview["data"]["total"], 1);
        assert_eq!(overview["data"]["cards"][0]["topicDisplay"], "Old War");

        let category: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/knowledge/categories/history?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(category["data"]["category"], "history");
        assert_eq!(category["data"]["total"], 1);

        let detail: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/knowledge/cards/{}?bookUrl={book_url}",
                card.id
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            detail["data"]["assertionsByStatus"]["active"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            detail["data"]["assertionsByStatus"]["rumor"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let memory: serde_json::Value = client
            .get(format!("{base_url}/api/books/v4/memory?bookUrl={book_url}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(memory["data"]["knowledgeCount"], 1);

        server.abort();
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn v4_map_api_returns_overview_places_graph_layout_conflicts() {
        let (state, dir) = create_test_state().await;
        db::v4::init_v4(&state.pool).await.unwrap();
        let book_url = "map-book";
        let book_id = crate::util::hash::md5_hex(book_url);
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('chapter-map-1', ?, 1, 'map text', 'hash-map', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('segment-map-1', ?, 'chapter-map-1', 'hash-map', 0, datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span-map-1', ?, 'chapter-map-1', 'hash-map', 'segment-map-1', 0, 0, 10, '青云城到黑风谷', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run-map-1', ?, 'chapter-map-1', 'extract', 'test', 'v1', 1, 'input', 'completed', datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, subject_mention, object_mention, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('claim-map-1', ?, 1, 'location_edge', '青云城', '黑风谷', 'location_edge', 'span-map-1', 'run-map-1', 0.9, 'medium', 'accepted', datetime('now'), datetime('now'))")
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('place-map-a', ?, 'place', '青云城', '青云城', 0.9, 1, 1, 'active', datetime('now'), datetime('now')), ('place-map-b', ?, 'place', '黑风谷', '黑风谷', 0.7, 1, 1, 'active', datetime('now'), datetime('now'))")
            .bind(&book_id)
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO place_details (entity_id, book_id, place_type, parent_place_id, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('place-map-a', ?, 'city', NULL, 0.9, 1, 1, 'active', datetime('now'), datetime('now')), ('place-map-b', ?, 'dungeon', 'place-map-a', 0.7, 1, 1, 'active', datetime('now'), datetime('now'))")
            .bind(&book_id)
            .bind(&book_id)
            .execute(&state.pool)
            .await
            .unwrap();
        for idx in 0..11 {
            let place_id = format!("place-map-extra-{idx}");
            let name = format!("外域据点{idx}");
            sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES (?, ?, 'place', ?, ?, 0.1, 1, 1, 'active', datetime('now'), datetime('now'))")
                .bind(&place_id)
                .bind(&book_id)
                .bind(&name)
                .bind(&name)
                .execute(&state.pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO place_details (entity_id, book_id, place_type, parent_place_id, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES (?, ?, 'building', NULL, 0.1, 1, 1, 'active', datetime('now'), datetime('now'))")
                .bind(&place_id)
                .bind(&book_id)
                .execute(&state.pool)
                .await
                .unwrap();
        }
        let place_repo = crate::storage::db::v4::place_repo::PlaceRepo::new(state.pool.clone());
        place_repo
            .find_or_create_edge(
                &book_id,
                "place-map-a",
                "place-map-b",
                "route_to",
                None,
                Some("三日路程"),
                0.9,
                "claim-map-1",
                1,
            )
            .await
            .unwrap();
        place_repo
            .insert_conflict(
                &book_id,
                "claim-map-1",
                None,
                "duplicate_conflicting_direction",
                "opposite_direction",
                None,
            )
            .await
            .unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(state)).await.unwrap();
        });
        let client = Client::new();
        let base_url = format!("http://{}", addr);

        let overview: serde_json::Value = client
            .get(format!("{base_url}/api/books/v4/map?bookUrl={book_url}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(overview["data"]["placeCount"], 13);
        assert_eq!(overview["data"]["activeEdgeCount"], 1);

        let places: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/map/places?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let hierarchy_names = places["data"]["hierarchy"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["name"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert!(hierarchy_names.contains(&"青云城"));
        assert_eq!(places["data"]["total"], 13);
        assert_eq!(places["data"]["places"].as_array().unwrap().len(), 13);

        let detail: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/map/places/place-map-a?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(detail["data"]["name"], "青云城");

        let graph: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/map/graph?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(graph["data"]["edges"][0]["edgeType"], "route_to");

        let layout: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/map/layout?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(layout["data"]["nodes"].as_array().unwrap().len(), 13);

        let conflicts: serde_json::Value = client
            .get(format!(
                "{base_url}/api/books/v4/map/conflicts?bookUrl={book_url}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(conflicts["data"]["total"], 1);

        server.abort();
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
}
