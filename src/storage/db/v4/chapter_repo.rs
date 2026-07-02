use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct ChapterRecord {
    pub id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub title: Option<String>,
    pub raw_text: String,
    pub text_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SegmentRecord {
    pub id: String,
    pub book_id: String,
    pub chapter_id: String,
    pub chapter_hash: String,
    pub segment_index: i64,
    pub segment_type: String,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub text_hash: Option<String>,
    pub status: String,
    pub created_at: String,
}

pub struct ChapterRepo {
    pool: SqlitePool,
}

impl ChapterRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Chapter operations ---

    pub async fn upsert_chapter(
        &self,
        book_id: &str,
        chapter_index: i64,
        title: Option<&str>,
        raw_text: &str,
        text_hash: &str,
    ) -> anyhow::Result<ChapterRecord> {
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO chapters (id, book_id, chapter_index, title, raw_text, text_hash, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(book_id, chapter_index) DO UPDATE SET
               title = excluded.title, raw_text = excluded.raw_text, text_hash = excluded.text_hash"
        )
        .bind(&id)
        .bind(book_id)
        .bind(chapter_index)
        .bind(title)
        .bind(raw_text)
        .bind(text_hash)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        // Fetch the actual record (may be the existing one on conflict)
        let record = self
            .get_chapter(book_id, chapter_index)
            .await?
            .ok_or_else(|| anyhow::anyhow!("chapter not found after upsert"))?;
        Ok(record)
    }

    pub async fn get_chapter(
        &self,
        book_id: &str,
        chapter_index: i64,
    ) -> anyhow::Result<Option<ChapterRecord>> {
        let row = sqlx::query_as::<_, ChapterRow>(
            "SELECT id, book_id, chapter_index, title, raw_text, text_hash, created_at
             FROM chapters WHERE book_id = ? AND chapter_index = ?",
        )
        .bind(book_id)
        .bind(chapter_index)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn get_chapter_by_id(
        &self,
        chapter_id: &str,
    ) -> anyhow::Result<Option<ChapterRecord>> {
        let row = sqlx::query_as::<_, ChapterRow>(
            "SELECT id, book_id, chapter_index, title, raw_text, text_hash, created_at
             FROM chapters WHERE id = ?",
        )
        .bind(chapter_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    // --- Segment operations ---

    pub async fn create_segment(
        &self,
        book_id: &str,
        chapter_id: &str,
        chapter_hash: &str,
        segment_index: i64,
        segment_type: &str,
        start_offset: Option<i64>,
        end_offset: Option<i64>,
        text_hash: Option<&str>,
    ) -> anyhow::Result<SegmentRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, segment_type, start_offset, end_offset, text_hash, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(chapter_id)
        .bind(chapter_hash)
        .bind(segment_index)
        .bind(segment_type)
        .bind(start_offset)
        .bind(end_offset)
        .bind(text_hash)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(SegmentRecord {
            id,
            book_id: book_id.to_string(),
            chapter_id: chapter_id.to_string(),
            chapter_hash: chapter_hash.to_string(),
            segment_index,
            segment_type: segment_type.to_string(),
            start_offset,
            end_offset,
            text_hash: text_hash.map(|s| s.to_string()),
            status: "active".to_string(),
            created_at: now,
        })
    }

    pub async fn get_segment(&self, segment_id: &str) -> anyhow::Result<Option<SegmentRecord>> {
        let row = sqlx::query_as::<_, SegmentRow>(
            "SELECT id, book_id, chapter_id, chapter_hash, segment_index, segment_type, start_offset, end_offset, text_hash, status, created_at
             FROM chapter_segments WHERE id = ?"
        )
        .bind(segment_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn list_active_segments(
        &self,
        chapter_id: &str,
    ) -> anyhow::Result<Vec<SegmentRecord>> {
        let rows = sqlx::query_as::<_, SegmentRow>(
            "SELECT id, book_id, chapter_id, chapter_hash, segment_index, segment_type, start_offset, end_offset, text_hash, status, created_at
             FROM chapter_segments WHERE chapter_id = ? AND status = 'active' ORDER BY segment_index ASC"
        )
        .bind(chapter_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn mark_segments_stale(&self, chapter_id: &str) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE chapter_segments SET status = 'stale' WHERE chapter_id = ? AND status = 'active'"
        )
        .bind(chapter_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_spans_stale(&self, chapter_id: &str) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE source_spans SET status = 'stale' WHERE chapter_id = ? AND status = 'active'",
        )
        .bind(chapter_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check if active segments already exist for this chapter_hash.
    pub async fn has_active_segments(
        &self,
        chapter_id: &str,
        chapter_hash: &str,
    ) -> anyhow::Result<bool> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM chapter_segments WHERE chapter_id = ? AND chapter_hash = ? AND status = 'active'"
        )
        .bind(chapter_id)
        .bind(chapter_hash)
        .fetch_one(&self.pool)
        .await?;

        Ok(count.0 > 0)
    }

    // --- Chapter Summary operations ---

    /// Upsert a chapter summary. Uses UNIQUE(book_id, chapter_index) for conflict resolution.
    pub async fn upsert_chapter_summary(
        &self,
        book_id: &str,
        chapter_index: i64,
        summary: &str,
        key_points_json: Option<&str>,
    ) -> anyhow::Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO chapter_summaries (id, book_id, chapter_index, summary, key_points_json, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(book_id, chapter_index) DO UPDATE SET
               summary = excluded.summary, key_points_json = excluded.key_points_json, updated_at = excluded.updated_at"
        )
        .bind(&id)
        .bind(book_id)
        .bind(chapter_index)
        .bind(summary)
        .bind(key_points_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a chapter summary by book_id and chapter_index.
    pub async fn get_chapter_summary(
        &self,
        book_id: &str,
        chapter_index: i64,
    ) -> anyhow::Result<Option<ChapterSummaryRecord>> {
        let row = sqlx::query_as::<_, ChapterSummaryRow>(
            "SELECT id, book_id, chapter_index, summary, key_points_json, updated_at
             FROM chapter_summaries WHERE book_id = ? AND chapter_index = ?",
        )
        .bind(book_id)
        .bind(chapter_index)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }
}

// SQLx row types

#[derive(sqlx::FromRow)]
struct ChapterRow {
    id: String,
    book_id: String,
    chapter_index: i64,
    title: Option<String>,
    raw_text: String,
    text_hash: String,
    created_at: String,
}

impl From<ChapterRow> for ChapterRecord {
    fn from(r: ChapterRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            chapter_index: r.chapter_index,
            title: r.title,
            raw_text: r.raw_text,
            text_hash: r.text_hash,
            created_at: r.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct SegmentRow {
    id: String,
    book_id: String,
    chapter_id: String,
    chapter_hash: String,
    segment_index: i64,
    segment_type: String,
    start_offset: Option<i64>,
    end_offset: Option<i64>,
    text_hash: Option<String>,
    status: String,
    created_at: String,
}

impl From<SegmentRow> for SegmentRecord {
    fn from(r: SegmentRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            chapter_id: r.chapter_id,
            chapter_hash: r.chapter_hash,
            segment_index: r.segment_index,
            segment_type: r.segment_type,
            start_offset: r.start_offset,
            end_offset: r.end_offset,
            text_hash: r.text_hash,
            status: r.status,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChapterSummaryRecord {
    pub id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub summary: String,
    pub key_points_json: Option<String>,
    pub updated_at: String,
}

#[derive(sqlx::FromRow)]
struct ChapterSummaryRow {
    id: String,
    book_id: String,
    chapter_index: i64,
    summary: String,
    key_points_json: Option<String>,
    updated_at: String,
}

impl From<ChapterSummaryRow> for ChapterSummaryRecord {
    fn from(r: ChapterSummaryRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            chapter_index: r.chapter_index,
            summary: r.summary,
            key_points_json: r.key_points_json,
            updated_at: r.updated_at,
        }
    }
}

/// Simple segmenter: split chapter text into segments.
/// Short chapters (< 3000 chars) → 1 segment.
/// Long chapters → split by paragraphs, group into ~3000-char segments.
pub fn segment_chapter(
    book_id: &str,
    chapter_id: &str,
    chapter_hash: &str,
    raw_text: &str,
) -> Vec<(SegmentRecord, Vec<(i64, i64, String)>)> {
    let char_count = raw_text.chars().count();

    if char_count < 3000 {
        // Short chapter: single segment, single span
        let segment = SegmentRecord {
            id: String::new(), // will be assigned on insert
            book_id: book_id.to_string(),
            chapter_id: chapter_id.to_string(),
            chapter_hash: chapter_hash.to_string(),
            segment_index: 0,
            segment_type: "default".to_string(),
            start_offset: Some(0),
            end_offset: Some(raw_text.len() as i64),
            text_hash: Some(chapter_hash.to_string()),
            status: "active".to_string(),
            created_at: String::new(),
        };
        let spans = vec![(0, raw_text.len() as i64, raw_text.to_string())];
        return vec![(segment, spans)];
    }

    // Long chapter: split by paragraphs
    let paragraphs: Vec<&str> = raw_text.split("\n\n").collect();
    let mut segments = Vec::new();
    let mut current_spans = Vec::new();
    let mut current_offset: i64 = 0;
    let mut segment_start: i64 = 0;
    let mut current_size: usize = 0;
    let mut seg_index: i64 = 0;

    for para in &paragraphs {
        let para_bytes = para.len() as i64;
        current_spans.push((
            current_offset,
            current_offset + para_bytes,
            para.to_string(),
        ));
        current_offset += para_bytes + 2; // +2 for "\n\n"
        current_size += para.chars().count();

        if current_size >= 3000 {
            let segment = SegmentRecord {
                id: String::new(),
                book_id: book_id.to_string(),
                chapter_id: chapter_id.to_string(),
                chapter_hash: chapter_hash.to_string(),
                segment_index: seg_index,
                segment_type: "default".to_string(),
                start_offset: Some(segment_start),
                end_offset: Some(current_offset),
                text_hash: Some(chapter_hash.to_string()),
                status: "active".to_string(),
                created_at: String::new(),
            };
            segments.push((segment, current_spans.clone()));
            current_spans.clear();
            segment_start = current_offset;
            current_size = 0;
            seg_index += 1;
        }
    }

    // Remaining spans
    if !current_spans.is_empty() {
        let segment = SegmentRecord {
            id: String::new(),
            book_id: book_id.to_string(),
            chapter_id: chapter_id.to_string(),
            chapter_hash: chapter_hash.to_string(),
            segment_index: seg_index,
            segment_type: "default".to_string(),
            start_offset: Some(segment_start),
            end_offset: Some(current_offset),
            text_hash: Some(chapter_hash.to_string()),
            status: "active".to_string(),
            created_at: String::new(),
        };
        segments.push((segment, current_spans));
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup() -> (SqlitePool, ChapterRepo) {
        let dir = std::env::temp_dir().join(format!("reader-v4-chapter-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        let repo = ChapterRepo::new(pool.clone());
        (pool, repo)
    }

    #[tokio::test]
    async fn upsert_chapter_creates_new() {
        let (_pool, repo) = setup().await;
        let ch = repo
            .upsert_chapter("b1", 1, Some("第一章"), "这是正文内容", "hash1")
            .await
            .unwrap();
        assert_eq!(ch.chapter_index, 1);
        assert_eq!(ch.title.as_deref(), Some("第一章"));
        assert_eq!(ch.raw_text, "这是正文内容");
    }

    #[tokio::test]
    async fn upsert_chapter_updates_existing() {
        let (_pool, repo) = setup().await;
        repo.upsert_chapter("b1", 1, Some("第一章"), "旧内容", "hash1")
            .await
            .unwrap();
        let updated = repo
            .upsert_chapter("b1", 1, Some("第一章v2"), "新内容", "hash2")
            .await
            .unwrap();
        assert_eq!(updated.title.as_deref(), Some("第一章v2"));
        assert_eq!(updated.raw_text, "新内容");
    }

    #[tokio::test]
    async fn create_and_list_segments() {
        let (_pool, repo) = setup().await;
        let ch = repo
            .upsert_chapter("b1", 1, None, "text", "h")
            .await
            .unwrap();

        repo.create_segment(
            "b1",
            &ch.id,
            "h",
            0,
            "default",
            Some(0),
            Some(100),
            Some("h"),
        )
        .await
        .unwrap();
        repo.create_segment(
            "b1",
            &ch.id,
            "h",
            1,
            "dialogue",
            Some(100),
            Some(200),
            Some("h"),
        )
        .await
        .unwrap();

        let segments = repo.list_active_segments(&ch.id).await.unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].segment_index, 0);
        assert_eq!(segments[1].segment_type, "dialogue");
    }

    #[tokio::test]
    async fn mark_segments_stale() {
        let (_pool, repo) = setup().await;
        let ch = repo
            .upsert_chapter("b1", 1, None, "text", "h")
            .await
            .unwrap();
        repo.create_segment("b1", &ch.id, "h", 0, "default", None, None, None)
            .await
            .unwrap();

        repo.mark_segments_stale(&ch.id).await.unwrap();

        let active = repo.list_active_segments(&ch.id).await.unwrap();
        assert!(
            active.is_empty(),
            "should have no active segments after stale"
        );
    }

    #[tokio::test]
    async fn has_active_segments_check() {
        let (_pool, repo) = setup().await;
        let ch = repo
            .upsert_chapter("b1", 1, None, "text", "h")
            .await
            .unwrap();

        assert!(!repo.has_active_segments(&ch.id, "h").await.unwrap());

        repo.create_segment("b1", &ch.id, "h", 0, "default", None, None, None)
            .await
            .unwrap();

        assert!(repo.has_active_segments(&ch.id, "h").await.unwrap());
        assert!(!repo
            .has_active_segments(&ch.id, "different_hash")
            .await
            .unwrap());
    }

    #[test]
    fn segmenter_short_chapter() {
        let text = "这是一个短章节。";
        let segments = segment_chapter("b1", "ch1", "hash1", text);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0.segment_index, 0);
        assert_eq!(segments[0].1.len(), 1); // one span
        assert_eq!(segments[0].1[0].2, text);
    }

    #[test]
    fn segmenter_long_chapter_splits() {
        // Create a text longer than 3000 chars
        let para = "这是一段文字内容。".repeat(200); // ~1600 chars per paragraph
        let text = format!("{}\n\n{}\n\n{}", para, para, para); // ~4800 chars
        let segments = segment_chapter("b1", "ch1", "hash1", &text);
        assert!(
            segments.len() >= 2,
            "long chapter should split into multiple segments, got {}",
            segments.len()
        );
    }

    #[tokio::test]
    async fn upsert_chapter_summary_creates() {
        let (_pool, repo) = setup().await;
        repo.upsert_chapter_summary("b1", 1, "本章介绍了主角", Some(r#"["关键点1","关键点2"]"#))
            .await
            .unwrap();

        let summary = repo.get_chapter_summary("b1", 1).await.unwrap().unwrap();
        assert_eq!(summary.summary, "本章介绍了主角");
        assert_eq!(
            summary.key_points_json.as_deref(),
            Some(r#"["关键点1","关键点2"]"#)
        );
    }

    #[tokio::test]
    async fn upsert_chapter_summary_updates_on_conflict() {
        let (_pool, repo) = setup().await;
        repo.upsert_chapter_summary("b1", 1, "旧摘要", None)
            .await
            .unwrap();
        repo.upsert_chapter_summary("b1", 1, "新摘要", Some(r#"["更新"]"#))
            .await
            .unwrap();

        let summary = repo.get_chapter_summary("b1", 1).await.unwrap().unwrap();
        assert_eq!(summary.summary, "新摘要");
        assert_eq!(summary.key_points_json.as_deref(), Some(r#"["更新"]"#));
    }

    #[tokio::test]
    async fn get_chapter_summary_not_found() {
        let (_pool, repo) = setup().await;
        let result = repo.get_chapter_summary("b1", 999).await.unwrap();
        assert!(result.is_none());
    }
}
