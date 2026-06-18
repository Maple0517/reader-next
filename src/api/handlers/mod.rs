mod ai_book;
mod ai_model;
mod ai_proxy;
mod book;
mod book_group;
mod book_source;
mod bookmark;
mod chapter_summary;
mod replace_rule;
mod rss;
mod update;
mod user;
mod webdav;

pub use ai_book::*;
pub use ai_model::*;
pub use ai_proxy::*;
pub use book::*;
pub use book_group::*;
pub use book_source::login_book_source;
pub use book_source::*;
pub use chapter_summary::*;
pub use bookmark::{
    delete_bookmark, delete_bookmarks, get_bookmarks, save_bookmark, save_bookmarks,
};
pub use replace_rule::{
    delete_replace_rule, delete_replace_rules, get_replace_rules, save_replace_rule,
    save_replace_rules,
};
pub use rss::{
    delete_rss_source, delete_rss_sources, get_rss_articles, get_rss_content, get_rss_sources,
    read_remote_rss_source_file, read_rss_source_file, save_rss_source, save_rss_sources,
};
pub use update::{dismiss_version_update, get_version_update};
pub use user::{
    add_user, change_password, delete_file, delete_users, get_user_config, get_user_info,
    get_user_list, login, logout, reset_password, save_user_config, update_user, upload_file,
};
pub use webdav::{
    delete_webdav_file, delete_webdav_file_list, get_webdav_file, get_webdav_file_list,
    upload_file_to_webdav, webdav_handler,
};

use crate::error::error::ApiResponse;
use axum::response::IntoResponse;
use axum::Json;

pub async fn health() -> impl IntoResponse {
    Json(ApiResponse::ok("ok"))
}
