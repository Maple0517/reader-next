use reader_rust::model::ai_book::{AiBookMap, AiBookMemory, AiBookNote};
use reader_rust::service::ai_book_service::AiBookService;
use reader_rust::storage::db;

fn temp_storage_dir(name: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "reader-rust-ai-book-test-{}-{}",
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
async fn ai_book_memory_round_trips_and_isolated_by_user() {
    let (service, storage_dir) = create_service("round-trip").await;
    let memory = AiBookMemory {
        book_url: "https://example.test/book/1".to_string(),
        book_name: Some("山海旧事".to_string()),
        enabled: true,
        processed_chapter_index: Some(7),
        summary: "主角抵达北境，首次听闻旧神传说。".to_string(),
        worldview: vec![AiBookNote {
            title: "旧神信仰".to_string(),
            content: "北境仍保留旧神祭仪，但真伪未知。".to_string(),
            confidence: Some("推断".to_string()),
        }],
        map: Some(AiBookMap {
            image_url: Some("/assets/alice/ai-maps/map.png".to_string()),
            prompt: Some("ink fantasy map of northern border".to_string()),
            updated_at: Some(1_700_000_000),
            source_chapter_index: Some(7),
        }),
        ..AiBookMemory::default()
    };

    service
        .save_for_book("alice", "https://example.test/book/1", memory.clone())
        .await
        .unwrap();

    let saved = service
        .get("alice", "https://example.test/book/1")
        .await
        .unwrap()
        .expect("alice should have memory");
    assert_eq!(saved.book_name.as_deref(), Some("山海旧事"));
    assert_eq!(saved.processed_chapter_index, Some(7));
    assert_eq!(saved.worldview[0].title, "旧神信仰");
    assert_eq!(
        saved.map.and_then(|map| map.image_url),
        Some("/assets/alice/ai-maps/map.png".to_string())
    );

    let bob = service
        .get("bob", "https://example.test/book/1")
        .await
        .unwrap();
    assert!(bob.is_none(), "memory must be isolated by user namespace");

    assert!(service
        .delete("alice", "https://example.test/book/1")
        .await
        .unwrap());
    assert!(service
        .get("alice", "https://example.test/book/1")
        .await
        .unwrap()
        .is_none());

    let _ = std::fs::remove_dir_all(storage_dir);
}

#[tokio::test]
async fn ai_book_memory_json_round_trips_v1_and_v2_extension_fields() {
    let (service, storage_dir) = create_service("json-round-trip").await;
    let mut memory = serde_json::json!({
        "schemaVersion": 2,
        "bookUrl": "https://example.test/book/v2",
        "bookName": "山海旧事",
        "author": "佚名",
        "enabled": true,
        "updatedAt": 0,
        "summary": {
            "current": "林舟抵达北境。",
            "recentChanges": ["林舟离开旧村"],
            "openQuestions": ["旧神传说真伪未知"]
        },
        "worldFacts": [{
            "id": "fact-old-god",
            "category": "历史传说",
            "title": "旧神传说",
            "content": "北境流传旧神传说。",
            "confidence": "推断",
            "importance": "high",
            "evidence": [{"chapterIndex": 7, "chapterTitle": "第八章", "note": "首次提及旧神"}]
        }],
        "worldview": [{
            "category": "历史传说",
            "title": "旧神传说",
            "content": "北境流传旧神传说。",
            "confidence": "推断",
            "importance": "high"
        }],
        "locations": [{
            "name": "北境",
            "parentName": "山海大陆",
            "kind": "区域",
            "description": "寒冷边境。",
            "importance": "high"
        }],
        "map": {
            "prompt": "绘制北境地图",
            "fallback": "relationship-graph",
            "fallbackReason": "图片模型不可用"
        },
        "mapState": {
            "dirty": true,
            "reason": "新增北境",
            "nodes": [],
            "edges": []
        },
        "renderArtifacts": {
            "mapFallbackReason": "图片模型不可用"
        }
    });

    let saved = service
        .save_value_for_book("alice", "https://example.test/book/v2", memory.clone())
        .await
        .unwrap();
    assert!(saved["updatedAt"].as_i64().unwrap() > 0);
    memory["updatedAt"] = saved["updatedAt"].clone();

    let loaded = service
        .get_value("alice", "https://example.test/book/v2")
        .await
        .unwrap()
        .expect("memory should exist");

    assert_eq!(loaded["schemaVersion"], 2);
    assert_eq!(loaded["worldFacts"][0]["category"], "历史传说");
    assert_eq!(loaded["worldview"][0]["importance"], "high");
    assert_eq!(loaded["locations"][0]["parentName"], "山海大陆");
    assert_eq!(loaded["map"]["fallbackReason"], "图片模型不可用");
    assert_eq!(loaded["mapState"]["reason"], "新增北境");
    assert_eq!(loaded["renderArtifacts"]["mapFallbackReason"], "图片模型不可用");
    assert_eq!(loaded, memory);

    let _ = std::fs::remove_dir_all(storage_dir);
}

#[tokio::test]
async fn ai_book_memory_rejects_mismatched_book_url_on_save() {
    let (service, storage_dir) = create_service("mismatch").await;

    let mut memory = AiBookMemory::default();
    memory.book_url = "https://example.test/book/1".to_string();

    let err = service
        .save_for_book("alice", "https://example.test/book/2", memory)
        .await
        .expect_err("mismatched bookUrl should fail");

    assert!(err.to_string().contains("bookUrl mismatch"));
    let _ = std::fs::remove_dir_all(storage_dir);
}
