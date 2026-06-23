use reader_next::model::ai_book::{
    AiBookCatchupStatsV3, AiBookMemoryV3, AiBookRenderArtifactsV3,
};
use reader_next::service::ai_book_memory_v3::create_empty_ai_book_memory_v3;
use reader_next::service::ai_book_service::AiBookService;
use reader_next::storage::db;

fn temp_storage_dir(name: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "reader-next-ai-book-test-{}-{}",
        name,
        std::process::id()
    ));
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path.to_string_lossy().to_string()
}

async fn create_service(name: &str) -> (AiBookService, String) {
    let storage_dir = temp_storage_dir(name);
    let database_url = format!(
        "sqlite:{}?mode=rwc",
        std::path::Path::new(&storage_dir)
            .join("reader.db")
            .display()
    );
    let pool = db::init_pool(&database_url).await.unwrap();
    (AiBookService::new(pool, &storage_dir), storage_dir)
}

#[tokio::test]
async fn ai_book_v3_memory_round_trips_and_isolated_by_user() {
    let (service, storage_dir) = create_service("round-trip").await;
    let book_url = "https://example.test/book/1";
    let mut memory = create_empty_ai_book_memory_v3(
        book_url,
        Some("山海旧事".to_string()),
        Some("佚名".to_string()),
    );
    memory.enabled = true;
    memory.processed_chapter_index = Some(7);
    memory.processed_chapter_title = Some("第八章".to_string());
    memory.summary.current = "主角抵达北境，首次听闻旧神传说。".to_string();
    memory.render_artifacts = Some(AiBookRenderArtifactsV3 {
        chapter_index: Some(7),
        chapter_title: Some("第八章".to_string()),
        summary: Some("北境地图".to_string()),
        image_url: Some("/assets/alice/ai-maps/map.png".to_string()),
        updated_at: Some(1_700_000_000),
    });

    service.save_v3("alice", book_url, memory).await.unwrap();

    let saved = service
        .get_or_create_v3("alice", book_url, None, None)
        .await
        .unwrap();
    assert_eq!(saved.book_name.as_deref(), Some("山海旧事"));
    assert_eq!(saved.processed_chapter_index, Some(7));
    assert_eq!(saved.summary.current, "主角抵达北境，首次听闻旧神传说。");
    assert_eq!(
        saved.render_artifacts.and_then(|artifact| artifact.image_url),
        Some("/assets/alice/ai-maps/map.png".to_string())
    );

    let bob = service
        .get_value("bob", book_url)
        .await
        .unwrap();
    assert!(bob.is_none(), "memory must be isolated by user namespace");

    assert!(service.delete("alice", book_url).await.unwrap());
    assert!(service.get_value("alice", book_url).await.unwrap().is_none());

    let _ = std::fs::remove_dir_all(storage_dir);
}

#[tokio::test]
async fn ai_book_v3_value_save_round_trips() {
    let (service, storage_dir) = create_service("json-round-trip").await;
    let book_url = "https://example.test/book/v3";
    let mut memory = create_empty_ai_book_memory_v3(
        book_url,
        Some("山海旧事".to_string()),
        Some("佚名".to_string()),
    );
    memory.enabled = true;
    memory.summary.current = "林舟抵达北境。".to_string();
    memory.summary.recent_changes = vec!["林舟离开旧村".to_string()];
    memory.summary.open_questions = vec!["旧神传说真伪未知".to_string()];
    memory.catchup_stats = Some(AiBookCatchupStatsV3 {
        total_model_calls: 3,
        digest_calls: 2,
        patch_calls: 1,
        skipped_patch_chapters: 1,
        total_input_bytes: 100,
        total_output_bytes: 50,
        last_call_latency_ms: Some(123),
        average_call_latency_ms: Some(100),
        last_chapter_index: Some(7),
        updated_at: 99,
    });

    let saved = service
        .save_value_as_v3("alice", book_url, serde_json::to_value(memory.clone()).unwrap())
        .await
        .unwrap();
    let saved_memory: AiBookMemoryV3 = serde_json::from_value(saved).unwrap();
    assert_eq!(saved_memory.schema_version, 3);
    assert_eq!(saved_memory.summary.current, "林舟抵达北境。");
    assert_eq!(saved_memory.catchup_stats.as_ref().unwrap().total_model_calls, 3);

    let loaded = service
        .get_or_create_v3("alice", book_url, None, None)
        .await
        .unwrap();
    assert_eq!(loaded.summary.recent_changes, vec!["林舟离开旧村"]);
    assert_eq!(loaded.catchup_stats.as_ref().unwrap().last_chapter_index, Some(7));

    let _ = std::fs::remove_dir_all(storage_dir);
}

#[tokio::test]
async fn ai_book_v3_save_rejects_mismatched_book_url() {
    let (service, storage_dir) = create_service("mismatch").await;
    let memory = create_empty_ai_book_memory_v3("https://example.test/book/1", None, None);

    let err = service
        .save_v3("alice", "https://example.test/book/2", memory)
        .await
        .expect_err("mismatched bookUrl should fail");

    assert!(err.to_string().contains("bookUrl mismatch"));
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[tokio::test]
async fn ai_book_v3_save_rejects_non_v3_schema() {
    let (service, storage_dir) = create_service("invalid-schema").await;
    let mut memory = create_empty_ai_book_memory_v3("https://example.test/book/schema", None, None);
    memory.schema_version = 2;

    let err = service
        .save_v3("alice", "https://example.test/book/schema", memory)
        .await
        .expect_err("non-v3 schema should fail");

    assert!(err.to_string().contains("unsupported ai book schema version"));
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[tokio::test]
async fn ai_book_v3_get_resets_non_v3_stored_data_without_backup() {
    let (service, storage_dir) = create_service("destructive-reset").await;
    let book_url = "https://example.test/book/old";
    let old = serde_json::json!({
        "schemaVersion": 2,
        "bookUrl": book_url,
        "summary": { "current": "旧资料" },
        "characters": [{ "name": "旧角色" }]
    });
    let pool = db::init_pool(&format!(
        "sqlite:{}?mode=rwc",
        std::path::Path::new(&storage_dir).join("reader.db").display()
    ))
    .await
    .unwrap();
    sqlx::query("INSERT INTO ai_book_memories (user_ns, book_key, book_url, json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)")
        .bind("alice")
        .bind(reader_next::util::hash::md5_hex(book_url))
        .bind(book_url)
        .bind(old.to_string())
        .bind(1_i64)
        .execute(&pool)
        .await
        .unwrap();

    let reset = service
        .get_or_create_v3("alice", book_url, Some("新书".to_string()), None)
        .await
        .unwrap();

    assert_eq!(reset.schema_version, 3);
    assert_eq!(reset.book_url, book_url);
    assert_eq!(reset.book_name.as_deref(), Some("新书"));
    assert!(reset.summary.current.is_empty());
    assert!(reset.characters.is_empty());

    let _ = std::fs::remove_dir_all(storage_dir);
}
