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

const AI_BOOK_MEMORY_ROUTE: &str = "/reader3/aiBook/memory";
const AI_BOOK_CHAPTER_MEMORY_ROUTE: &str = "/reader3/aiBook/chapterMemory";
const AI_BOOK_MEMORY_RESET_ROUTE: &str = "/reader3/aiBook/memory/reset";
const AI_BOOK_ENABLED_ROUTE: &str = "/reader3/aiBook/enabled";
const AI_BOOK_CHAPTER_GENERATE_ROUTE: &str = "/reader3/aiBook/chapterMemory/generate";
const AI_BOOK_MAP_GENERATE_ROUTE: &str = "/reader3/aiBook/map/generate";
const AI_BOOK_CATCHUP_START_ROUTE: &str = "/reader3/aiBook/catchup/start";
const AI_BOOK_CATCHUP_STATUS_ROUTE: &str = "/reader3/aiBook/catchup/status";
const AI_BOOK_CATCHUP_CANCEL_ROUTE: &str = "/reader3/aiBook/catchup/cancel";

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
        .route(
            AI_BOOK_ENABLED_ROUTE,
            post(handlers::set_ai_book_enabled),
        )
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
            "/reader3/getAiModelConfig",
            get(handlers::get_ai_model_config),
        )
        .route(
            "/reader3/saveAiModelConfig",
            post(handlers::save_ai_model_config),
        )
        .route(
            "/reader3/chapterSummary",
            get(handlers::get_chapter_summary),
        )
        .route(
            "/reader3/chapterSummary/generate",
            post(handlers::generate_chapter_summary),
        )
        .route(
            "/reader3/chapterSummary/config",
            get(handlers::get_chapter_summary_config).post(handlers::save_chapter_summary_config),
        )
        .route("/reader3/aiProxy", post(handlers::ai_proxy))
        .route("/reader3/aiProxyImage", post(handlers::ai_proxy_image))
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
    use crate::service::local_txt_book::LocalTxtBookService;
    use crate::service::update_service::UpdateService;
    use crate::service::user_service::UserService;
    use crate::storage::cache::file_cache::FileCache;
    use crate::storage::db;
    use crate::storage::db::repo::BookSourceRepo;
    use crate::util::crypto::random_string;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tower::util::ServiceExt;

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
        let http = crate::crawler::http_client::HttpClient::new(cfg.request_timeout_secs, None).unwrap();
        let parser = crate::parser::rule_engine::RuleEngine::new().unwrap();
        let cache = FileCache::new(format!("{}/cache", cfg.storage_dir));
        let book_service = Arc::new(BookService::new(http, parser, cache, &cfg.storage_dir));
        let book_source_service = Arc::new(BookSourceService::new(BookSourceRepo::new(pool.clone()), &cfg.storage_dir));
        let local_txt_book_service = Arc::new(LocalTxtBookService::new(&cfg.storage_dir));
        let json_document_service = Arc::new(JsonDocumentService::new(pool.clone(), &cfg.storage_dir));
        let user_service = Arc::new(UserService::new(cfg.clone(), pool.clone()));
        user_service.migrate_legacy_users_from_json().await.unwrap();
        let book_group_service = Arc::new(BookGroupService::new(json_document_service.clone()));
        let ai_book_service = Arc::new(AiBookService::new(pool.clone(), &cfg.storage_dir));
        let ai_book_generation_service = Arc::new(AiBookGenerationService::new(
            ai_book_service.clone(),
            book_service.clone(),
            book_source_service.clone(),
            local_txt_book_service.clone(),
        ));
        let ai_book_catchup_service = Arc::new(AiBookCatchupService::new());
        let ai_model_service = Arc::new(AiModelService::new(json_document_service.clone(), &cfg.storage_dir));
        let chapter_summary_service = Arc::new(ChapterSummaryService::new(json_document_service.clone()));
        let update_service = Arc::new(UpdateService::new(
            json_document_service.clone(),
            cfg.request_timeout_secs,
            format!("v{}", env!("CARGO_PKG_VERSION")),
        ).unwrap());
        let state = AppState {
            config: cfg,
            book_service,
            book_source_service,
            user_service,
            book_group_service,
            local_txt_book_service,
            json_document_service,
            ai_book_service,
            ai_book_generation_service,
            ai_book_catchup_service,
            ai_model_service,
            chapter_summary_service,
            update_service,
        };
        (state, dir)
    }

    #[tokio::test]
    async fn ai_book_v3_routes_are_registered_without_legacy_aliases() {
        let (state, dir) = create_test_state().await;
        let mut app = build_router(state);

        for (method, path) in [
            (Method::POST, "/reader3/saveAiBookMemory"),
            (Method::GET, "/reader3/getAiBookMemory"),
            (Method::POST, "/reader3/aiBookCatchup/pause"),
        ] {
            let response = app
                .as_service::<Body>()
                .oneshot(
                    Request::builder()
                        .method(method.clone())
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
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
            let response = app
                .as_service::<Body>()
                .oneshot(
                    Request::builder()
                        .method(method.clone())
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_ne!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
            assert_ne!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} {path}"
            );
        }

        let wrong_method_memory = app
            .as_service::<Body>()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(AI_BOOK_MEMORY_ROUTE)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(wrong_method_memory.status(), StatusCode::METHOD_NOT_ALLOWED);

        let wrong_method_enabled = app
            .as_service::<Body>()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(AI_BOOK_ENABLED_ROUTE)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(wrong_method_enabled.status(), StatusCode::METHOD_NOT_ALLOWED);

        let _ = tokio::fs::remove_dir_all(dir).await;
    }
}
