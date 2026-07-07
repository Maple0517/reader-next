use crate::service::v4::extractor::Observation;
use crate::storage::db::v4::chapter_repo::ChapterRepo;

#[derive(Debug, Default)]
pub struct SummaryWriteResult {
    pub summaries_written: Vec<String>,
}

pub async fn write_chapter_summaries(
    observations: &[Observation],
    book_id: &str,
    chapter_index: i64,
    chapter_repo: &ChapterRepo,
) -> anyhow::Result<SummaryWriteResult> {
    let mut result = SummaryWriteResult::default();

    for obs in observations {
        if let Observation::Summary {
            summary,
            key_points,
            ..
        } = obs
        {
            let key_points_json = serde_json::to_string(key_points).ok();
            chapter_repo
                .upsert_chapter_summary(book_id, chapter_index, summary, key_points_json.as_deref())
                .await?;
            result.summaries_written.push(summary.clone());
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use sqlx::SqlitePool;

    async fn setup() -> (SqlitePool, ChapterRepo) {
        let dir = std::env::temp_dir().join(format!(
            "reader-v4-summary-processor-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();

        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 1, 'text', 'hash', datetime('now'))")
            .bind(&chapter_id)
            .execute(&pool)
            .await
            .unwrap();

        (pool.clone(), ChapterRepo::new(pool))
    }

    #[tokio::test]
    async fn summary_processor_writes_chapter_summary() {
        let (_pool, chapter_repo) = setup().await;
        let observations = vec![Observation::Summary {
            summary: "张三突破到筑基期".to_string(),
            key_points: vec!["突破".to_string(), "筑基".to_string()],
            has_important_changes: true,
        }];

        let result = write_chapter_summaries(&observations, "b1", 1, &chapter_repo)
            .await
            .unwrap();

        assert_eq!(result.summaries_written, vec!["张三突破到筑基期"]);
        let summary = chapter_repo.get_chapter_summary("b1", 1).await.unwrap();
        assert!(summary.is_some());
        assert_eq!(summary.unwrap().summary, "张三突破到筑基期");
    }
}
