use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct SourceSpanRecord {
    pub id: String,
    pub book_id: String,
    pub chapter_id: String,
    pub chapter_hash: String,
    pub segment_id: String,
    pub span_index: i64,
    pub start_offset: i64,
    pub end_offset: i64,
    pub text_excerpt: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ClaimRecord {
    pub id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub claim_type: String,
    pub subject_mention: Option<String>,
    pub object_mention: Option<String>,
    pub subject_entity_id: Option<String>,
    pub object_entity_id: Option<String>,
    pub predicate: String,
    pub value_json: Option<String>,
    pub value_text: Option<String>,
    pub primary_source_span_id: String,
    pub ai_run_id: String,
    pub confidence: f64,
    pub risk_level: String,
    pub status: String,
    pub supersedes_claim_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct ClaimRepo {
    pool: SqlitePool,
}

impl ClaimRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Source Span operations ---

    pub async fn create_span(
        &self,
        book_id: &str,
        chapter_id: &str,
        chapter_hash: &str,
        segment_id: &str,
        span_index: i64,
        start_offset: i64,
        end_offset: i64,
        text_excerpt: &str,
    ) -> anyhow::Result<SourceSpanRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(chapter_id)
        .bind(chapter_hash)
        .bind(segment_id)
        .bind(span_index)
        .bind(start_offset)
        .bind(end_offset)
        .bind(text_excerpt)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(SourceSpanRecord {
            id,
            book_id: book_id.to_string(),
            chapter_id: chapter_id.to_string(),
            chapter_hash: chapter_hash.to_string(),
            segment_id: segment_id.to_string(),
            span_index,
            start_offset,
            end_offset,
            text_excerpt: text_excerpt.to_string(),
            status: "active".to_string(),
            created_at: now,
        })
    }

    pub async fn get_span(&self, span_id: &str) -> anyhow::Result<Option<SourceSpanRecord>> {
        let row = sqlx::query_as::<_, SourceSpanRow>(
            "SELECT id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, status, created_at
             FROM source_spans WHERE id = ?"
        )
        .bind(span_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn list_spans_by_segment(
        &self,
        segment_id: &str,
    ) -> anyhow::Result<Vec<SourceSpanRecord>> {
        let rows = sqlx::query_as::<_, SourceSpanRow>(
            "SELECT id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, status, created_at
             FROM source_spans WHERE segment_id = ? AND status = 'active' ORDER BY span_index ASC"
        )
        .bind(segment_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    // --- Claim operations ---

    pub async fn create_claim(
        &self,
        book_id: &str,
        chapter_index: i64,
        claim_type: &str,
        subject_mention: Option<&str>,
        object_mention: Option<&str>,
        subject_entity_id: Option<&str>,
        object_entity_id: Option<&str>,
        predicate: &str,
        value_text: Option<&str>,
        value_json: Option<&str>,
        primary_source_span_id: &str,
        ai_run_id: &str,
        confidence: f64,
        risk_level: &str,
    ) -> anyhow::Result<ClaimRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO claims (id, book_id, chapter_index, claim_type, subject_mention, object_mention, subject_entity_id, object_entity_id, predicate, value_text, value_json, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'proposed', ?, ?)"
        )
        .bind(&id)
        .bind(book_id)
        .bind(chapter_index)
        .bind(claim_type)
        .bind(subject_mention)
        .bind(object_mention)
        .bind(subject_entity_id)
        .bind(object_entity_id)
        .bind(predicate)
        .bind(value_text)
        .bind(value_json)
        .bind(primary_source_span_id)
        .bind(ai_run_id)
        .bind(confidence)
        .bind(risk_level)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(ClaimRecord {
            id,
            book_id: book_id.to_string(),
            chapter_index,
            claim_type: claim_type.to_string(),
            subject_mention: subject_mention.map(|s| s.to_string()),
            object_mention: object_mention.map(|s| s.to_string()),
            subject_entity_id: subject_entity_id.map(|s| s.to_string()),
            object_entity_id: object_entity_id.map(|s| s.to_string()),
            predicate: predicate.to_string(),
            value_json: value_json.map(|s| s.to_string()),
            value_text: value_text.map(|s| s.to_string()),
            primary_source_span_id: primary_source_span_id.to_string(),
            ai_run_id: ai_run_id.to_string(),
            confidence,
            risk_level: risk_level.to_string(),
            status: "proposed".to_string(),
            supersedes_claim_id: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn get_claim(&self, claim_id: &str) -> anyhow::Result<Option<ClaimRecord>> {
        let row = sqlx::query_as::<_, ClaimRow>(
            "SELECT id, book_id, chapter_index, claim_type, subject_mention, object_mention, subject_entity_id, object_entity_id, predicate, value_json, value_text, primary_source_span_id, ai_run_id, confidence, risk_level, status, supersedes_claim_id, created_at, updated_at
             FROM claims WHERE id = ?"
        )
        .bind(claim_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    pub async fn list_claims_by_chapter(
        &self,
        book_id: &str,
        chapter_index: i64,
    ) -> anyhow::Result<Vec<ClaimRecord>> {
        let rows = sqlx::query_as::<_, ClaimRow>(
            "SELECT id, book_id, chapter_index, claim_type, subject_mention, object_mention, subject_entity_id, object_entity_id, predicate, value_json, value_text, primary_source_span_id, ai_run_id, confidence, risk_level, status, supersedes_claim_id, created_at, updated_at
             FROM claims WHERE book_id = ? AND chapter_index = ? ORDER BY created_at ASC"
        )
        .bind(book_id)
        .bind(chapter_index)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn list_claims_by_status(
        &self,
        book_id: &str,
        status: &str,
    ) -> anyhow::Result<Vec<ClaimRecord>> {
        let rows = sqlx::query_as::<_, ClaimRow>(
            "SELECT id, book_id, chapter_index, claim_type, subject_mention, object_mention, subject_entity_id, object_entity_id, predicate, value_json, value_text, primary_source_span_id, ai_run_id, confidence, risk_level, status, supersedes_claim_id, created_at, updated_at
             FROM claims WHERE book_id = ? AND status = ? ORDER BY created_at ASC"
        )
        .bind(book_id)
        .bind(status)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn update_claim_status(
        &self,
        claim_id: &str,
        new_status: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE claims SET status = ?, updated_at = ? WHERE id = ?")
            .bind(new_status)
            .bind(&now)
            .bind(claim_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Update a claim's value_json (e.g., to write back AI judge normalized fields).
    pub async fn update_claim_value_json(
        &self,
        claim_id: &str,
        value_json: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE claims SET value_json = ?, updated_at = ? WHERE id = ?")
            .bind(value_json)
            .bind(&now)
            .bind(claim_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn batch_update_claim_status(
        &self,
        claim_ids: &[String],
        new_status: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        for id in claim_ids {
            sqlx::query("UPDATE claims SET status = ?, updated_at = ? WHERE id = ?")
                .bind(new_status)
                .bind(&now)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // --- Claim-SourceSpan association ---

    pub async fn add_claim_source_span(
        &self,
        claim_id: &str,
        source_span_id: &str,
        role: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO claim_source_spans (claim_id, source_span_id, role) VALUES (?, ?, ?)"
        )
        .bind(claim_id)
        .bind(source_span_id)
        .bind(role)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_claim_spans(&self, claim_id: &str) -> anyhow::Result<Vec<SourceSpanRecord>> {
        let rows = sqlx::query_as::<_, SourceSpanRow>(
            "SELECT s.id, s.book_id, s.chapter_id, s.chapter_hash, s.segment_id, s.span_index, s.start_offset, s.end_offset, s.text_excerpt, s.status, s.created_at
             FROM source_spans s
             JOIN claim_source_spans css ON css.source_span_id = s.id
             WHERE css.claim_id = ?
             ORDER BY s.span_index ASC"
        )
        .bind(claim_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    // Transaction-aware methods for use within reducer

    pub async fn batch_update_claim_status_with_conn(
        conn: &mut sqlx::SqliteConnection,
        claim_ids: &[String],
        new_status: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        for claim_id in claim_ids {
            sqlx::query("UPDATE claims SET status = ?, updated_at = ? WHERE id = ?")
                .bind(new_status)
                .bind(&now)
                .bind(claim_id)
                .execute(&mut *conn)
                .await?;
        }
        Ok(())
    }

    pub async fn update_claim_status_with_conn(
        conn: &mut sqlx::SqliteConnection,
        claim_id: &str,
        new_status: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE claims SET status = ?, updated_at = ? WHERE id = ?")
            .bind(new_status)
            .bind(&now)
            .bind(claim_id)
            .execute(conn)
            .await?;
        Ok(())
    }

    pub async fn list_claims_by_status_with_conn(
        conn: &mut sqlx::SqliteConnection,
        book_id: &str,
        status: &str,
    ) -> anyhow::Result<Vec<ClaimRecord>> {
        let rows = sqlx::query_as::<_, ClaimRow>(
            "SELECT id, book_id, chapter_index, claim_type, subject_mention, object_mention, subject_entity_id, object_entity_id, predicate, value_json, value_text, primary_source_span_id, ai_run_id, confidence, risk_level, status, supersedes_claim_id, created_at, updated_at
             FROM claims WHERE book_id = ? AND status = ? ORDER BY chapter_index ASC"
        )
        .bind(book_id)
        .bind(status)
        .fetch_all(conn)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }
}

#[derive(sqlx::FromRow)]
struct SourceSpanRow {
    id: String,
    book_id: String,
    chapter_id: String,
    chapter_hash: String,
    segment_id: String,
    span_index: i64,
    start_offset: i64,
    end_offset: i64,
    text_excerpt: String,
    status: String,
    created_at: String,
}

impl From<SourceSpanRow> for SourceSpanRecord {
    fn from(r: SourceSpanRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            chapter_id: r.chapter_id,
            chapter_hash: r.chapter_hash,
            segment_id: r.segment_id,
            span_index: r.span_index,
            start_offset: r.start_offset,
            end_offset: r.end_offset,
            text_excerpt: r.text_excerpt,
            status: r.status,
            created_at: r.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ClaimRow {
    id: String,
    book_id: String,
    chapter_index: i64,
    claim_type: String,
    subject_mention: Option<String>,
    object_mention: Option<String>,
    subject_entity_id: Option<String>,
    object_entity_id: Option<String>,
    predicate: String,
    value_json: Option<String>,
    value_text: Option<String>,
    primary_source_span_id: String,
    ai_run_id: String,
    confidence: f64,
    risk_level: String,
    status: String,
    supersedes_claim_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<ClaimRow> for ClaimRecord {
    fn from(r: ClaimRow) -> Self {
        Self {
            id: r.id,
            book_id: r.book_id,
            chapter_index: r.chapter_index,
            claim_type: r.claim_type,
            subject_mention: r.subject_mention,
            object_mention: r.object_mention,
            subject_entity_id: r.subject_entity_id,
            object_entity_id: r.object_entity_id,
            predicate: r.predicate,
            value_json: r.value_json,
            value_text: r.value_text,
            primary_source_span_id: r.primary_source_span_id,
            ai_run_id: r.ai_run_id,
            confidence: r.confidence,
            risk_level: r.risk_level,
            status: r.status,
            supersedes_claim_id: r.supersedes_claim_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    /// Create chapter + segment + span + ai_run, return (span_id, run_id)
    async fn setup_test_infra(pool: &SqlitePool, book_id: &str) -> (String, String) {
        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, ?, 1, 'test text', 'hash1', datetime('now'))")
            .bind(&chapter_id).bind(book_id).execute(pool).await.unwrap();

        let segment_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, ?, ?, 'hash1', 0, datetime('now'))")
            .bind(&segment_id).bind(book_id).bind(&chapter_id).execute(pool).await.unwrap();

        let span_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, ?, ?, 'hash1', ?, 0, 0, 10, '张三走进了大殿', datetime('now'))")
            .bind(&span_id).bind(book_id).bind(&chapter_id).bind(&segment_id).execute(pool).await.unwrap();

        let run_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, ?, ?, 'extract', 'test-model', 'v1', 1, 'inputhash', 'success', datetime('now'))")
            .bind(&run_id).bind(book_id).bind(&chapter_id).execute(pool).await.unwrap();

        (span_id, run_id)
    }

    async fn setup() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("reader-v4-claim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn create_and_get_span() {
        let pool = setup().await;
        let repo = ClaimRepo::new(pool.clone());

        // Need chapter + segment first
        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 1, 'text', 'h', datetime('now'))")
            .bind(&chapter_id).execute(&pool).await.unwrap();
        let segment_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'h', 0, datetime('now'))")
            .bind(&segment_id).bind(&chapter_id).execute(&pool).await.unwrap();

        let span = repo
            .create_span(
                "b1",
                &chapter_id,
                "h",
                &segment_id,
                0,
                0,
                20,
                "张三走进了大殿",
            )
            .await
            .unwrap();
        assert_eq!(span.text_excerpt, "张三走进了大殿");
        assert_eq!(span.span_index, 0);

        let fetched = repo.get_span(&span.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, span.id);
    }

    #[tokio::test]
    async fn list_spans_by_segment() {
        let pool = setup().await;
        let repo = ClaimRepo::new(pool.clone());

        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 1, 'text', 'h', datetime('now'))")
            .bind(&chapter_id).execute(&pool).await.unwrap();
        let segment_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'h', 0, datetime('now'))")
            .bind(&segment_id).bind(&chapter_id).execute(&pool).await.unwrap();

        repo.create_span("b1", &chapter_id, "h", &segment_id, 0, 0, 10, "span0")
            .await
            .unwrap();
        repo.create_span("b1", &chapter_id, "h", &segment_id, 1, 10, 20, "span1")
            .await
            .unwrap();
        repo.create_span("b1", &chapter_id, "h", &segment_id, 2, 20, 30, "span2")
            .await
            .unwrap();

        let spans = repo.list_spans_by_segment(&segment_id).await.unwrap();
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].span_index, 0);
        assert_eq!(spans[2].span_index, 2);
    }

    #[tokio::test]
    async fn create_and_get_claim() {
        let pool = setup().await;
        let repo = ClaimRepo::new(pool.clone());
        let (span_id, run_id) = setup_test_infra(&pool, "b1").await;

        let claim = repo
            .create_claim(
                "b1",
                1,
                "entity_introduction",
                Some("张三"),
                None,
                None,
                None,
                "is_character",
                Some("一个修士"),
                None,
                &span_id,
                &run_id,
                0.9,
                "low",
            )
            .await
            .unwrap();

        assert_eq!(claim.claim_type, "entity_introduction");
        assert_eq!(claim.status, "proposed");
        assert_eq!(claim.subject_mention.as_deref(), Some("张三"));

        let fetched = repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, claim.id);
    }

    #[tokio::test]
    async fn claim_status_transitions() {
        let pool = setup().await;
        let repo = ClaimRepo::new(pool.clone());
        let (span_id, run_id) = setup_test_infra(&pool, "b1").await;

        let claim = repo
            .create_claim(
                "b1",
                1,
                "property_update",
                Some("张三"),
                None,
                None,
                None,
                "has_realm",
                Some("练气期"),
                None,
                &span_id,
                &run_id,
                0.8,
                "low",
            )
            .await
            .unwrap();

        // proposed → accepted
        repo.update_claim_status(&claim.id, "accepted")
            .await
            .unwrap();
        let c = repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(c.status, "accepted");
    }

    #[tokio::test]
    async fn high_risk_claim_quarantined() {
        let pool = setup().await;
        let repo = ClaimRepo::new(pool.clone());
        let (span_id, run_id) = setup_test_infra(&pool, "b1").await;

        // Create high-risk claim (e.g. death)
        let claim = repo
            .create_claim(
                "b1",
                1,
                "property_update",
                Some("张三"),
                None,
                None,
                None,
                "life_status",
                Some("死亡"),
                None,
                &span_id,
                &run_id,
                0.7,
                "high",
            )
            .await
            .unwrap();

        // High risk → quarantined by Claim Writer (simulated here)
        repo.update_claim_status(&claim.id, "quarantined")
            .await
            .unwrap();
        let c = repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(c.status, "quarantined");

        // Should not appear in "proposed" list
        let proposed = repo.list_claims_by_status("b1", "proposed").await.unwrap();
        assert!(proposed.is_empty());

        // Should appear in "quarantined" list
        let quarantined = repo
            .list_claims_by_status("b1", "quarantined")
            .await
            .unwrap();
        assert_eq!(quarantined.len(), 1);
    }

    #[tokio::test]
    async fn list_claims_by_chapter() {
        let pool = setup().await;
        let repo = ClaimRepo::new(pool.clone());
        let (span_id, run_id) = setup_test_infra(&pool, "b1").await;

        repo.create_claim(
            "b1",
            1,
            "entity_introduction",
            Some("张三"),
            None,
            None,
            None,
            "is_char",
            None,
            None,
            &span_id,
            &run_id,
            0.9,
            "low",
        )
        .await
        .unwrap();
        repo.create_claim(
            "b1",
            1,
            "alias",
            Some("张三"),
            None,
            None,
            None,
            "alias",
            Some("小三"),
            None,
            &span_id,
            &run_id,
            0.8,
            "low",
        )
        .await
        .unwrap();

        let claims = repo.list_claims_by_chapter("b1", 1).await.unwrap();
        assert_eq!(claims.len(), 2);
    }

    #[tokio::test]
    async fn claim_source_span_association() {
        let pool = setup().await;
        let repo = ClaimRepo::new(pool.clone());
        let (span_id, run_id) = setup_test_infra(&pool, "b1").await;

        let claim = repo
            .create_claim(
                "b1",
                1,
                "entity_introduction",
                Some("张三"),
                None,
                None,
                None,
                "is_character",
                None,
                None,
                &span_id,
                &run_id,
                0.9,
                "low",
            )
            .await
            .unwrap();

        repo.add_claim_source_span(&claim.id, &span_id, "primary")
            .await
            .unwrap();

        let spans = repo.list_claim_spans(&claim.id).await.unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].id, span_id);
    }

    #[tokio::test]
    async fn batch_update_status() {
        let pool = setup().await;
        let repo = ClaimRepo::new(pool.clone());
        let (span_id, run_id) = setup_test_infra(&pool, "b1").await;

        let c1 = repo
            .create_claim(
                "b1",
                1,
                "alias",
                Some("A"),
                None,
                None,
                None,
                "a",
                None,
                None,
                &span_id,
                &run_id,
                0.9,
                "low",
            )
            .await
            .unwrap();
        let c2 = repo
            .create_claim(
                "b1",
                1,
                "alias",
                Some("B"),
                None,
                None,
                None,
                "b",
                None,
                None,
                &span_id,
                &run_id,
                0.8,
                "low",
            )
            .await
            .unwrap();

        repo.batch_update_claim_status(&[c1.id.clone(), c2.id.clone()], "accepted")
            .await
            .unwrap();

        assert_eq!(
            repo.get_claim(&c1.id).await.unwrap().unwrap().status,
            "accepted"
        );
        assert_eq!(
            repo.get_claim(&c2.id).await.unwrap().unwrap().status,
            "accepted"
        );
    }
}
