use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KnowledgeCardRecord {
    pub id: String,
    pub book_id: String,
    pub category: String,
    pub topic_key: String,
    pub topic_display: String,
    pub current_summary: Option<String>,
    pub confidence: f64,
    pub importance_score: f64,
    pub first_seen_chapter: i64,
    pub last_updated_chapter: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KnowledgeAssertionRecord {
    pub id: String,
    pub book_id: String,
    pub card_id: String,
    pub source_claim_id: String,
    pub assertion_text: String,
    pub status: String,
    pub confidence: f64,
    pub importance_score: f64,
    pub chapter_index: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KnowledgeAssertionLinkRecord {
    pub id: String,
    pub book_id: String,
    pub from_assertion_id: String,
    pub to_assertion_id: String,
    pub link_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KnowledgeAssertionEntityRecord {
    pub id: String,
    pub book_id: String,
    pub assertion_id: String,
    pub entity_id: String,
    pub role: String,
    pub created_at: String,
}

pub struct KnowledgeRepo {
    pool: SqlitePool,
}

impl KnowledgeRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn find_or_create_card(
        &self,
        book_id: &str,
        category: &str,
        topic_key: &str,
        topic_display: &str,
        current_summary: Option<&str>,
        confidence: f64,
        importance_score: f64,
        chapter_index: i64,
    ) -> anyhow::Result<KnowledgeCardRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::find_or_create_card_with_conn(
            &mut conn,
            book_id,
            category,
            topic_key,
            topic_display,
            current_summary,
            confidence,
            importance_score,
            chapter_index,
        )
        .await
    }

    pub async fn find_or_create_card_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        category: &str,
        topic_key: &str,
        topic_display: &str,
        current_summary: Option<&str>,
        confidence: f64,
        importance_score: f64,
        chapter_index: i64,
    ) -> anyhow::Result<KnowledgeCardRecord> {
        let normalized_topic_key = normalize_topic_key(topic_key);
        if let Some(existing) =
            Self::find_card_by_topic_with_conn(conn, book_id, category, &normalized_topic_key)
                .await?
        {
            return Ok(existing);
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO knowledge_cards (id, book_id, category, topic_key, topic_display, current_summary, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)",
        )
        .bind(&id)
        .bind(book_id)
        .bind(category)
        .bind(&normalized_topic_key)
        .bind(topic_display.trim())
        .bind(current_summary.map(str::trim))
        .bind(confidence)
        .bind(importance_score)
        .bind(chapter_index)
        .bind(chapter_index)
        .bind(&now)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        Ok(KnowledgeCardRecord {
            id,
            book_id: book_id.to_string(),
            category: category.to_string(),
            topic_key: normalized_topic_key,
            topic_display: topic_display.trim().to_string(),
            current_summary: current_summary.map(|s| s.trim().to_string()),
            confidence,
            importance_score,
            first_seen_chapter: chapter_index,
            last_updated_chapter: chapter_index,
            status: "active".to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn find_card_by_topic(
        &self,
        book_id: &str,
        category: &str,
        topic_key: &str,
    ) -> anyhow::Result<Option<KnowledgeCardRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::find_card_by_topic_with_conn(&mut conn, book_id, category, topic_key).await
    }

    pub async fn find_card_by_topic_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        category: &str,
        topic_key: &str,
    ) -> anyhow::Result<Option<KnowledgeCardRecord>> {
        let normalized_topic_key = normalize_topic_key(topic_key);
        let row = sqlx::query_as::<_, KnowledgeCardRecord>(
            "SELECT id, book_id, category, topic_key, topic_display, current_summary, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at
             FROM knowledge_cards
             WHERE book_id = ? AND category = ? AND topic_key = ?
             LIMIT 1",
        )
        .bind(book_id)
        .bind(category)
        .bind(normalized_topic_key)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn get_card_by_id(
        &self,
        card_id: &str,
    ) -> anyhow::Result<Option<KnowledgeCardRecord>> {
        let row = sqlx::query_as::<_, KnowledgeCardRecord>(
            "SELECT id, book_id, category, topic_key, topic_display, current_summary, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at
             FROM knowledge_cards WHERE id = ?",
        )
        .bind(card_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_cards(
        &self,
        book_id: &str,
        category: Option<&str>,
        status: Option<&str>,
    ) -> anyhow::Result<Vec<KnowledgeCardRecord>> {
        match (category, status) {
            (Some(category), Some(status)) => {
                sqlx::query_as::<_, KnowledgeCardRecord>(
                    "SELECT id, book_id, category, topic_key, topic_display, current_summary, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at
                     FROM knowledge_cards
                     WHERE book_id = ? AND category = ? AND status = ?
                     ORDER BY importance_score DESC, last_updated_chapter DESC, topic_display ASC",
                )
                .bind(book_id)
                .bind(category)
                .bind(status)
                .fetch_all(&self.pool)
                .await
            }
            (Some(category), None) => {
                sqlx::query_as::<_, KnowledgeCardRecord>(
                    "SELECT id, book_id, category, topic_key, topic_display, current_summary, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at
                     FROM knowledge_cards
                     WHERE book_id = ? AND category = ?
                     ORDER BY importance_score DESC, last_updated_chapter DESC, topic_display ASC",
                )
                .bind(book_id)
                .bind(category)
                .fetch_all(&self.pool)
                .await
            }
            (None, Some(status)) => {
                sqlx::query_as::<_, KnowledgeCardRecord>(
                    "SELECT id, book_id, category, topic_key, topic_display, current_summary, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at
                     FROM knowledge_cards
                     WHERE book_id = ? AND status = ?
                     ORDER BY importance_score DESC, last_updated_chapter DESC, topic_display ASC",
                )
                .bind(book_id)
                .bind(status)
                .fetch_all(&self.pool)
                .await
            }
            (None, None) => {
                sqlx::query_as::<_, KnowledgeCardRecord>(
                    "SELECT id, book_id, category, topic_key, topic_display, current_summary, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at
                     FROM knowledge_cards
                     WHERE book_id = ?
                     ORDER BY importance_score DESC, last_updated_chapter DESC, topic_display ASC",
                )
                .bind(book_id)
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(Into::into)
    }

    pub async fn find_or_create_assertion(
        &self,
        book_id: &str,
        card_id: &str,
        source_claim_id: &str,
        assertion_text: &str,
        status: &str,
        confidence: f64,
        importance_score: f64,
        chapter_index: i64,
    ) -> anyhow::Result<KnowledgeAssertionRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::find_or_create_assertion_with_conn(
            &mut conn,
            book_id,
            card_id,
            source_claim_id,
            assertion_text,
            status,
            confidence,
            importance_score,
            chapter_index,
        )
        .await
    }

    pub async fn find_or_create_assertion_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        card_id: &str,
        source_claim_id: &str,
        assertion_text: &str,
        status: &str,
        confidence: f64,
        importance_score: f64,
        chapter_index: i64,
    ) -> anyhow::Result<KnowledgeAssertionRecord> {
        let normalized_assertion = normalize_assertion_text(assertion_text);
        let existing = sqlx::query_as::<_, KnowledgeAssertionRecord>(
            "SELECT id, book_id, card_id, source_claim_id, assertion_text, status, confidence, importance_score, chapter_index, created_at, updated_at
             FROM knowledge_assertions
             WHERE book_id = ? AND card_id = ? AND source_claim_id = ? AND lower(trim(assertion_text)) = ?
             LIMIT 1",
        )
        .bind(book_id)
        .bind(card_id)
        .bind(source_claim_id)
        .bind(&normalized_assertion)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(existing) = existing {
            return Ok(existing);
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let assertion_text = assertion_text.trim();
        sqlx::query(
            "INSERT INTO knowledge_assertions (id, book_id, card_id, source_claim_id, assertion_text, status, confidence, importance_score, chapter_index, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(book_id)
        .bind(card_id)
        .bind(source_claim_id)
        .bind(assertion_text)
        .bind(status)
        .bind(confidence)
        .bind(importance_score)
        .bind(chapter_index)
        .bind(&now)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        Ok(KnowledgeAssertionRecord {
            id,
            book_id: book_id.to_string(),
            card_id: card_id.to_string(),
            source_claim_id: source_claim_id.to_string(),
            assertion_text: assertion_text.to_string(),
            status: status.to_string(),
            confidence,
            importance_score,
            chapter_index,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn get_assertion_by_id(
        &self,
        assertion_id: &str,
    ) -> anyhow::Result<Option<KnowledgeAssertionRecord>> {
        let row = sqlx::query_as::<_, KnowledgeAssertionRecord>(
            "SELECT id, book_id, card_id, source_claim_id, assertion_text, status, confidence, importance_score, chapter_index, created_at, updated_at
             FROM knowledge_assertions WHERE id = ?",
        )
        .bind(assertion_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_assertions_for_card(
        &self,
        card_id: &str,
    ) -> anyhow::Result<Vec<KnowledgeAssertionRecord>> {
        let rows = sqlx::query_as::<_, KnowledgeAssertionRecord>(
            "SELECT id, book_id, card_id, source_claim_id, assertion_text, status, confidence, importance_score, chapter_index, created_at, updated_at
             FROM knowledge_assertions
             WHERE card_id = ?
             ORDER BY chapter_index ASC, created_at ASC, id ASC",
        )
        .bind(card_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_assertion_status(
        &self,
        assertion_id: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        Self::update_assertion_status_with_conn(
            &mut *self.pool.acquire().await?,
            assertion_id,
            status,
        )
        .await
    }

    pub async fn update_assertion_status_with_conn(
        conn: &mut SqliteConnection,
        assertion_id: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE knowledge_assertions SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status)
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(assertion_id)
            .execute(&mut *conn)
            .await?;
        Ok(())
    }

    pub async fn insert_assertion_link(
        &self,
        book_id: &str,
        from_assertion_id: &str,
        to_assertion_id: &str,
        link_type: &str,
    ) -> anyhow::Result<KnowledgeAssertionLinkRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::insert_assertion_link_with_conn(
            &mut conn,
            book_id,
            from_assertion_id,
            to_assertion_id,
            link_type,
        )
        .await
    }

    pub async fn insert_assertion_link_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        from_assertion_id: &str,
        to_assertion_id: &str,
        link_type: &str,
    ) -> anyhow::Result<KnowledgeAssertionLinkRecord> {
        if let Some(existing) = Self::find_assertion_link_with_conn(
            conn,
            book_id,
            from_assertion_id,
            to_assertion_id,
            link_type,
        )
        .await?
        {
            return Ok(existing);
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO knowledge_assertion_links (id, book_id, from_assertion_id, to_assertion_id, link_type, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(book_id)
        .bind(from_assertion_id)
        .bind(to_assertion_id)
        .bind(link_type)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        Ok(KnowledgeAssertionLinkRecord {
            id,
            book_id: book_id.to_string(),
            from_assertion_id: from_assertion_id.to_string(),
            to_assertion_id: to_assertion_id.to_string(),
            link_type: link_type.to_string(),
            created_at: now,
        })
    }

    async fn find_assertion_link_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        from_assertion_id: &str,
        to_assertion_id: &str,
        link_type: &str,
    ) -> anyhow::Result<Option<KnowledgeAssertionLinkRecord>> {
        let row = sqlx::query_as::<_, KnowledgeAssertionLinkRecord>(
            "SELECT id, book_id, from_assertion_id, to_assertion_id, link_type, created_at
             FROM knowledge_assertion_links
             WHERE book_id = ? AND from_assertion_id = ? AND to_assertion_id = ? AND link_type = ?
             LIMIT 1",
        )
        .bind(book_id)
        .bind(from_assertion_id)
        .bind(to_assertion_id)
        .bind(link_type)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn list_links_from(
        &self,
        book_id: &str,
        from_assertion_id: &str,
        link_type: Option<&str>,
    ) -> anyhow::Result<Vec<KnowledgeAssertionLinkRecord>> {
        let rows = if let Some(link_type) = link_type {
            sqlx::query_as::<_, KnowledgeAssertionLinkRecord>(
                "SELECT id, book_id, from_assertion_id, to_assertion_id, link_type, created_at
                 FROM knowledge_assertion_links
                 WHERE book_id = ? AND from_assertion_id = ? AND link_type = ?
                 ORDER BY created_at ASC, id ASC",
            )
            .bind(book_id)
            .bind(from_assertion_id)
            .bind(link_type)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, KnowledgeAssertionLinkRecord>(
                "SELECT id, book_id, from_assertion_id, to_assertion_id, link_type, created_at
                 FROM knowledge_assertion_links
                 WHERE book_id = ? AND from_assertion_id = ?
                 ORDER BY created_at ASC, id ASC",
            )
            .bind(book_id)
            .bind(from_assertion_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn insert_assertion_entity(
        &self,
        book_id: &str,
        assertion_id: &str,
        entity_id: &str,
        role: &str,
    ) -> anyhow::Result<KnowledgeAssertionEntityRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::insert_assertion_entity_with_conn(&mut conn, book_id, assertion_id, entity_id, role)
            .await
    }

    pub async fn insert_assertion_entity_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        assertion_id: &str,
        entity_id: &str,
        role: &str,
    ) -> anyhow::Result<KnowledgeAssertionEntityRecord> {
        let resolved_entity_id = Self::resolve_redirect_target_with_conn(conn, book_id, entity_id)
            .await?
            .unwrap_or_else(|| entity_id.to_string());
        if let Some(existing) =
            Self::find_assertion_entity_with_conn(conn, assertion_id, &resolved_entity_id, role)
                .await?
        {
            return Ok(existing);
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO knowledge_assertion_entities (id, book_id, assertion_id, entity_id, role, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(book_id)
        .bind(assertion_id)
        .bind(&resolved_entity_id)
        .bind(role)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        Ok(KnowledgeAssertionEntityRecord {
            id,
            book_id: book_id.to_string(),
            assertion_id: assertion_id.to_string(),
            entity_id: resolved_entity_id,
            role: role.to_string(),
            created_at: now,
        })
    }

    async fn find_assertion_entity_with_conn(
        conn: &mut SqliteConnection,
        assertion_id: &str,
        entity_id: &str,
        role: &str,
    ) -> anyhow::Result<Option<KnowledgeAssertionEntityRecord>> {
        let row = sqlx::query_as::<_, KnowledgeAssertionEntityRecord>(
            "SELECT id, book_id, assertion_id, entity_id, role, created_at
             FROM knowledge_assertion_entities
             WHERE assertion_id = ? AND entity_id = ? AND role = ?
             LIMIT 1",
        )
        .bind(assertion_id)
        .bind(entity_id)
        .bind(role)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn list_entities_for_assertion(
        &self,
        assertion_id: &str,
    ) -> anyhow::Result<Vec<KnowledgeAssertionEntityRecord>> {
        let rows = sqlx::query_as::<_, KnowledgeAssertionEntityRecord>(
            "SELECT id, book_id, assertion_id, entity_id, role, created_at
             FROM knowledge_assertion_entities
             WHERE assertion_id = ?
             ORDER BY role ASC, entity_id ASC",
        )
        .bind(assertion_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn remap_assertion_entities_after_redirect(
        &self,
        book_id: &str,
        victim_entity_id: &str,
        survivor_entity_id: &str,
    ) -> anyhow::Result<()> {
        let mut conn = self.pool.acquire().await?;
        Self::remap_assertion_entities_after_redirect_with_conn(
            &mut conn,
            book_id,
            victim_entity_id,
            survivor_entity_id,
        )
        .await
    }

    pub async fn remap_assertion_entities_after_redirect_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        victim_entity_id: &str,
        survivor_entity_id: &str,
    ) -> anyhow::Result<()> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, assertion_id, role
             FROM knowledge_assertion_entities
             WHERE book_id = ? AND entity_id = ?",
        )
        .bind(book_id)
        .bind(victim_entity_id)
        .fetch_all(&mut *conn)
        .await?;

        for (row_id, assertion_id, role) in rows {
            let existing: Option<(String,)> = sqlx::query_as(
                "SELECT id
                 FROM knowledge_assertion_entities
                 WHERE assertion_id = ? AND entity_id = ? AND role = ?
                 LIMIT 1",
            )
            .bind(&assertion_id)
            .bind(survivor_entity_id)
            .bind(&role)
            .fetch_optional(&mut *conn)
            .await?;

            if existing.is_some() {
                sqlx::query("DELETE FROM knowledge_assertion_entities WHERE id = ?")
                    .bind(&row_id)
                    .execute(&mut *conn)
                    .await?;
            } else {
                sqlx::query(
                    "UPDATE knowledge_assertion_entities
                     SET entity_id = ?
                     WHERE id = ?",
                )
                .bind(survivor_entity_id)
                .bind(&row_id)
                .execute(&mut *conn)
                .await?;
            }
        }
        Ok(())
    }

    async fn resolve_redirect_target_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        entity_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT entity_b_id
             FROM entity_identity_links
             WHERE book_id = ? AND entity_a_id = ?
               AND link_type = 'redirect' AND status = 'active'
             LIMIT 1",
        )
        .bind(book_id)
        .bind(entity_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|r| r.0))
    }
}

fn normalize_topic_key(value: &str) -> String {
    let mut normalized = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            normalized.push(ch);
            last_dash = false;
        } else if (ch.is_whitespace() || ch == '-' || ch == '_') && !last_dash {
            normalized.push('-');
            last_dash = true;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn normalize_assertion_text(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::KnowledgeRepo;
    use crate::storage::db;
    use crate::storage::db::v4::identity_repo::IdentityRepo;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir =
            std::env::temp_dir().join(format!("reader-knowledge-repo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    async fn insert_source_claim(pool: &SqlitePool, book_id: &str, claim_id: &str) {
        let chapter_id = format!("ch-{book_id}-{claim_id}");
        let segment_id = format!("seg-{book_id}-{claim_id}");
        let span_id = format!("span-{book_id}-{claim_id}");
        let run_id = format!("run-{book_id}-{claim_id}");

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, ?, 1, 'text', ?, datetime('now'))")
            .bind(&chapter_id)
            .bind(book_id)
            .bind(format!("hash-{claim_id}"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, ?, ?, ?, 0, datetime('now'))")
            .bind(&segment_id)
            .bind(book_id)
            .bind(&chapter_id)
            .bind(format!("hash-{claim_id}"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, ?, ?, ?, ?, 0, 0, 10, 'text', datetime('now'))")
            .bind(&span_id)
            .bind(book_id)
            .bind(&chapter_id)
            .bind(format!("hash-{claim_id}"))
            .bind(&segment_id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, ?, ?, 'extract', 'test', 'v1', 1, ?, 'completed', datetime('now'))")
            .bind(&run_id)
            .bind(book_id)
            .bind(&chapter_id)
            .bind(format!("input-{claim_id}"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, ?, 1, 'knowledge_assertion', 'test', ?, ?, 0.9, 'high', 'proposed', datetime('now'), datetime('now'))")
            .bind(claim_id)
            .bind(book_id)
            .bind(&span_id)
            .bind(&run_id)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn insert_entity(pool: &SqlitePool, book_id: &str, entity_id: &str) {
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES (?, ?, 'concept', ?, ?, 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .bind(entity_id)
            .bind(book_id)
            .bind(entity_id)
            .bind(entity_id)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn insert_redirect(pool: &SqlitePool, book_id: &str, victim_id: &str, survivor_id: &str) {
        let mut tx = pool.begin().await.unwrap();
        IdentityRepo::find_or_create_identity_link_with_conn(
            &mut tx,
            book_id,
            victim_id,
            survivor_id,
            "redirect",
            0.95,
            "c1",
            "active",
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn card_create_reuses_normalized_topic_key_and_lists_by_category() {
        let pool = setup_test_db().await;
        let repo = KnowledgeRepo::new(pool);

        let first = repo
            .find_or_create_card(
                "b1",
                "power_system",
                " Cultivation Realms ",
                "Cultivation Realms",
                Some("Initial summary"),
                0.8,
                0.9,
                1,
            )
            .await
            .unwrap();
        let second = repo
            .find_or_create_card(
                "b1",
                "power_system",
                "cultivation-realms",
                "Cultivation Realm System",
                None,
                0.7,
                0.8,
                2,
            )
            .await
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(first.topic_key, "cultivation-realms");

        let cards = repo
            .list_cards("b1", Some("power_system"), Some("active"))
            .await
            .unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].topic_display, "Cultivation Realms");
    }

    #[tokio::test]
    async fn assertion_create_dedupes_and_updates_status() {
        let pool = setup_test_db().await;
        insert_source_claim(&pool, "b1", "c1").await;
        let repo = KnowledgeRepo::new(pool);
        let card = repo
            .find_or_create_card(
                "b1",
                "world_rule",
                "magic-rules",
                "Magic Rules",
                None,
                0.8,
                0.9,
                1,
            )
            .await
            .unwrap();

        let first = repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "c1",
                " Magic has rules. ",
                "active",
                0.9,
                0.8,
                1,
            )
            .await
            .unwrap();
        let second = repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "c1",
                "magic has rules.",
                "active",
                0.7,
                0.6,
                1,
            )
            .await
            .unwrap();

        assert_eq!(first.id, second.id);

        repo.update_assertion_status(&first.id, "revised")
            .await
            .unwrap();
        let updated = repo.get_assertion_by_id(&first.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "revised");
    }

    #[tokio::test]
    async fn assertion_links_support_multiple_targets_and_list_by_source() {
        let pool = setup_test_db().await;
        insert_source_claim(&pool, "b1", "c1").await;
        let repo = KnowledgeRepo::new(pool);
        let card = repo
            .find_or_create_card("b1", "history", "old-war", "Old War", None, 0.8, 0.9, 1)
            .await
            .unwrap();
        let old_a = repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "c1",
                "Old war lasted a year.",
                "active",
                0.8,
                0.7,
                1,
            )
            .await
            .unwrap();
        let old_b = repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "c1",
                "Old war ended cleanly.",
                "active",
                0.8,
                0.7,
                1,
            )
            .await
            .unwrap();
        let new_a = repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "c1",
                "Old war lasted three years.",
                "active",
                0.9,
                0.9,
                2,
            )
            .await
            .unwrap();

        repo.insert_assertion_link("b1", &new_a.id, &old_a.id, "supersedes")
            .await
            .unwrap();
        repo.insert_assertion_link("b1", &new_a.id, &old_b.id, "supersedes")
            .await
            .unwrap();
        repo.insert_assertion_link("b1", &new_a.id, &old_b.id, "supersedes")
            .await
            .unwrap();

        let links = repo
            .list_links_from("b1", &new_a.id, Some("supersedes"))
            .await
            .unwrap();
        assert_eq!(links.len(), 2);
    }

    #[tokio::test]
    async fn assertion_entity_refs_are_role_aware_and_idempotent() {
        let pool = setup_test_db().await;
        insert_source_claim(&pool, "b1", "c1").await;
        insert_entity(&pool, "b1", "e1").await;
        let repo = KnowledgeRepo::new(pool);
        let card = repo
            .find_or_create_card(
                "b1",
                "world_rule",
                "magic-rules",
                "Magic Rules",
                None,
                0.8,
                0.9,
                1,
            )
            .await
            .unwrap();
        let assertion = repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "c1",
                "Magic has rules.",
                "active",
                0.9,
                0.8,
                1,
            )
            .await
            .unwrap();

        repo.insert_assertion_entity("b1", &assertion.id, "e1", "related")
            .await
            .unwrap();
        repo.insert_assertion_entity("b1", &assertion.id, "e1", "related")
            .await
            .unwrap();
        repo.insert_assertion_entity("b1", &assertion.id, "e1", "subject")
            .await
            .unwrap();

        let refs = repo
            .list_entities_for_assertion(&assertion.id)
            .await
            .unwrap();
        assert_eq!(refs.len(), 2);
    }

    #[tokio::test]
    async fn assertion_entity_ref_resolves_active_redirect_to_survivor() {
        let pool = setup_test_db().await;
        insert_source_claim(&pool, "b1", "c1").await;
        insert_entity(&pool, "b1", "victim").await;
        insert_entity(&pool, "b1", "survivor").await;
        insert_redirect(&pool, "b1", "victim", "survivor").await;
        let repo = KnowledgeRepo::new(pool);
        let card = repo
            .find_or_create_card("b1", "secret", "hidden", "Hidden", None, 0.8, 0.9, 1)
            .await
            .unwrap();
        let assertion = repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "c1",
                "Hidden truth references victim.",
                "active",
                0.9,
                0.8,
                1,
            )
            .await
            .unwrap();

        repo.insert_assertion_entity("b1", &assertion.id, "victim", "related")
            .await
            .unwrap();

        let refs = repo
            .list_entities_for_assertion(&assertion.id)
            .await
            .unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].entity_id, "survivor");
    }

    #[tokio::test]
    async fn existing_assertion_entity_refs_can_be_remapped_after_later_merge() {
        let pool = setup_test_db().await;
        insert_source_claim(&pool, "b1", "c1").await;
        insert_entity(&pool, "b1", "victim").await;
        insert_entity(&pool, "b1", "survivor").await;
        let repo = KnowledgeRepo::new(pool.clone());
        let card = repo
            .find_or_create_card("b1", "secret", "hidden", "Hidden", None, 0.8, 0.9, 1)
            .await
            .unwrap();
        let assertion = repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "c1",
                "Hidden truth references victim.",
                "active",
                0.9,
                0.8,
                1,
            )
            .await
            .unwrap();
        repo.insert_assertion_entity("b1", &assertion.id, "victim", "related")
            .await
            .unwrap();
        insert_redirect(&pool, "b1", "victim", "survivor").await;

        repo.remap_assertion_entities_after_redirect("b1", "victim", "survivor")
            .await
            .unwrap();

        let refs = repo
            .list_entities_for_assertion(&assertion.id)
            .await
            .unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].entity_id, "survivor");
    }

    #[tokio::test]
    async fn with_conn_helpers_roll_back_with_transaction() {
        let pool = setup_test_db().await;
        insert_source_claim(&pool, "b1", "c1").await;
        let mut tx = pool.begin().await.unwrap();

        let card = KnowledgeRepo::find_or_create_card_with_conn(
            &mut *tx,
            "b1",
            "secret",
            "hidden-truth",
            "Hidden Truth",
            None,
            0.8,
            0.9,
            1,
        )
        .await
        .unwrap();
        KnowledgeRepo::find_or_create_assertion_with_conn(
            &mut *tx,
            "b1",
            &card.id,
            "c1",
            "There is a hidden truth.",
            "active",
            0.9,
            0.8,
            1,
        )
        .await
        .unwrap();

        tx.rollback().await.unwrap();

        let repo = KnowledgeRepo::new(pool);
        assert!(repo.list_cards("b1", None, None).await.unwrap().is_empty());
    }
}
