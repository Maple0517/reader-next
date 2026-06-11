use reader_rust::service::local_txt_book::{build_chapter_url, parse_txt_chapters};

#[test]
fn txt_parser_splits_common_chapter_headings() {
    let text = "序言\n这里是序言。\n第1章 初见\n第一章正文。\n第二章 重逢\n第二章正文。\n";

    let chapters = parse_txt_chapters("local-txt:abc123", text);

    assert_eq!(chapters.len(), 3);
    assert_eq!(chapters[0].title, "序章");
    assert_eq!(chapters[0].url, "local-txt:abc123#0");
    assert_eq!(chapters[0].content.trim(), "序言\n这里是序言。");
    assert_eq!(chapters[1].title, "第1章 初见");
    assert_eq!(chapters[1].content.trim(), "第一章正文。");
    assert_eq!(chapters[2].title, "第二章 重逢");
    assert_eq!(chapters[2].content.trim(), "第二章正文。");
}

#[test]
fn txt_parser_falls_back_to_single_body_chapter() {
    let text = "没有章节标题\n只有正文\n";

    let chapters = parse_txt_chapters("local-txt:book", text);

    assert_eq!(chapters.len(), 1);
    assert_eq!(chapters[0].title, "正文");
    assert_eq!(chapters[0].url, "local-txt:book#0");
    assert_eq!(chapters[0].content, text);
}

#[test]
fn txt_chapter_urls_are_stable_by_book_url_and_index() {
    assert_eq!(
        build_chapter_url("local-txt:abc123", 7),
        "local-txt:abc123#7"
    );
}

#[tokio::test]
async fn txt_import_saves_book_index_and_content_for_server_bookshelf() {
    let storage_dir =
        std::env::temp_dir().join(format!("reader-rust-local-txt-test-{}", std::process::id()));
    if storage_dir.exists() {
        std::fs::remove_dir_all(&storage_dir).unwrap();
    }
    let service = reader_rust::service::local_txt_book::LocalTxtBookService::new(&storage_dir);
    let bytes = "第1章 开始\n第一章内容。\n第二章 后来\n第二章内容。".as_bytes();

    let book = service
        .import_txt_book("alice", "测试小说.txt", bytes)
        .await
        .unwrap();
    let chapters = service
        .get_chapter_list("alice", &book.book_url)
        .await
        .unwrap();
    let content = service
        .get_content("alice", &chapters[1].url)
        .await
        .unwrap();

    assert_eq!(book.name, "测试小说");
    assert_eq!(book.origin, "local-txt");
    assert_eq!(book.origin_name.as_deref(), Some("本地 TXT"));
    assert_eq!(book.can_update, Some(false));
    assert_eq!(chapters.len(), 2);
    assert_eq!(chapters[0].title, "第1章 开始");
    assert_eq!(chapters[1].title, "第二章 后来");
    assert_eq!(content.trim(), "第二章内容。");

    let _ = std::fs::remove_dir_all(storage_dir);
}

#[tokio::test]
async fn txt_import_rejects_non_txt_files() {
    let storage_dir = std::env::temp_dir().join(format!(
        "reader-rust-local-txt-reject-test-{}",
        std::process::id()
    ));
    let service = reader_rust::service::local_txt_book::LocalTxtBookService::new(&storage_dir);

    let err = service
        .import_txt_book("alice", "测试小说.md", "正文".as_bytes())
        .await
        .expect_err("non txt file should be rejected");

    assert!(err.to_string().contains(".txt"));
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[tokio::test]
async fn saving_same_named_local_txt_books_keeps_distinct_book_urls() {
    use reader_rust::crawler::http_client::HttpClient;
    use reader_rust::parser::rule_engine::RuleEngine;
    use reader_rust::service::book_service::BookService;
    use reader_rust::storage::cache::file_cache::FileCache;

    let storage_dir = std::env::temp_dir().join(format!(
        "reader-rust-local-txt-save-test-{}",
        std::process::id()
    ));
    if storage_dir.exists() {
        std::fs::remove_dir_all(&storage_dir).unwrap();
    }

    let local_service =
        reader_rust::service::local_txt_book::LocalTxtBookService::new(&storage_dir);
    let book_service = BookService::new(
        HttpClient::new(5, None).unwrap(),
        RuleEngine::new().unwrap(),
        FileCache::new(storage_dir.join("cache")),
        storage_dir.to_str().unwrap(),
    );

    let first = local_service
        .import_txt_book("alice", "同名小说.txt", "第一版正文".as_bytes())
        .await
        .unwrap();
    let second = local_service
        .import_txt_book("alice", "同名小说.txt", "第二版正文".as_bytes())
        .await
        .unwrap();

    book_service
        .save_book("alice", first.clone())
        .await
        .unwrap();
    book_service
        .save_book("alice", second.clone())
        .await
        .unwrap();
    let shelf = book_service.get_bookshelf("alice").await.unwrap();

    assert_ne!(first.book_url, second.book_url);
    assert_eq!(shelf.len(), 2);
    assert!(shelf.iter().any(|book| book.book_url == first.book_url));
    assert!(shelf.iter().any(|book| book.book_url == second.book_url));

    let _ = std::fs::remove_dir_all(storage_dir);
}
