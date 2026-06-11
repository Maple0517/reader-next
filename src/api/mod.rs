pub mod auth;
pub mod handlers;
pub mod router;

use crate::app::config::AppConfig;
use crate::service::{
    ai_book_service::AiBookService, ai_model_service::AiModelService,
    book_group_service::BookGroupService, book_service::BookService,
    book_source_service::BookSourceService, json_document_service::JsonDocumentService,
    local_txt_book::LocalTxtBookService, update_service::UpdateService, user_service::UserService,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub book_service: Arc<BookService>,
    pub book_source_service: Arc<BookSourceService>,
    pub user_service: Arc<UserService>,
    pub book_group_service: Arc<BookGroupService>,
    pub local_txt_book_service: Arc<LocalTxtBookService>,
    pub json_document_service: Arc<JsonDocumentService>,
    pub ai_book_service: Arc<AiBookService>,
    pub ai_model_service: Arc<AiModelService>,
    pub update_service: Arc<UpdateService>,
}
