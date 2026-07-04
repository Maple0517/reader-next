use sqlx::{Row, SqliteConnection, SqlitePool};

// --- Records ---

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RelationshipRecord {
    pub id: String,
    pub book_id: String,
    pub subject_character_id: String,
    pub object_character_id: String,
    pub relation_group: String,
    pub relation_label: String,
    pub directionality: String,
    pub current_state: Option<String>,
    pub strength: f64,
    pub polarity: String,
    pub confidence: f64,
    pub importance_score: f64,
    pub first_seen_chapter: i64,
    pub last_changed_chapter: i64,
    pub last_seen_chapter: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RelationshipEventRecord {
    pub id: String,
    pub book_id: String,
    pub relationship_id: String,
    pub event_type: String,
    pub relation_group: String,
    pub relation_label: String,
    pub state_after: Option<String>,
    pub strength_after: Option<f64>,
    pub polarity_after: Option<String>,
    pub chapter_index: i64,
    pub source_claim_id: String,
    pub confidence: f64,
    pub created_at: String,
}

// --- Validation ---

const VALID_RELATION_GROUPS: &[&str] = &[
    "family",
    "romance",
    "friendship",
    "mentorship",
    "hierarchy",
    "alliance",
    "rivalry",
    "hostility",
    "debt_obligation",
    "contract",
    "acquaintance",
    "other_social",
    "unknown_significant",
];

const VALID_DIRECTIONALITIES: &[&str] = &["directed", "undirected"];

const VALID_POLARITIES: &[&str] = &["positive", "negative", "mixed", "neutral", "unknown"];

const VALID_RELATIONSHIP_STATUSES: &[&str] = &["active", "inactive", "uncertain", "rejected"];

pub fn validate_relation_group(group: &str) -> bool {
    VALID_RELATION_GROUPS.contains(&group)
}

pub fn validate_directionality(dir: &str) -> bool {
    VALID_DIRECTIONALITIES.contains(&dir)
}

pub fn validate_polarity(polarity: &str) -> bool {
    VALID_POLARITIES.contains(&polarity)
}

pub fn validate_relationship_status(status: &str) -> bool {
    VALID_RELATIONSHIP_STATUSES.contains(&status)
}

// --- RelationshipRepo ---

pub struct RelationshipRepo {
    pool: SqlitePool,
}

impl RelationshipRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_relationship(
        &self,
        book_id: &str,
        subject_character_id: &str,
        object_character_id: &str,
        relation_group: &str,
        relation_label: &str,
        directionality: &str,
        current_state: Option<&str>,
        strength: f64,
        polarity: &str,
        confidence: f64,
        importance_score: f64,
        first_seen_chapter: i64,
    ) -> anyhow::Result<RelationshipRecord> {
        Self::create_relationship_with_conn(
            &mut *self.pool.acquire().await?,
            book_id,
            subject_character_id,
            object_character_id,
            relation_group,
            relation_label,
            directionality,
            current_state,
            strength,
            polarity,
            confidence,
            importance_score,
            first_seen_chapter,
        )
        .await
    }

    pub async fn find_existing(
        &self,
        book_id: &str,
        subject_character_id: &str,
        object_character_id: &str,
        relation_group: &str,
        directionality: &str,
    ) -> anyhow::Result<Option<RelationshipRecord>> {
        Self::find_existing_with_conn(
            &mut *self.pool.acquire().await?,
            book_id,
            subject_character_id,
            object_character_id,
            relation_group,
            directionality,
        )
        .await
    }

    pub async fn update_relationship(
        &self,
        id: &str,
        relation_label: &str,
        current_state: Option<&str>,
        strength: f64,
        polarity: &str,
        confidence: f64,
        importance_score: f64,
        last_changed_chapter: i64,
        last_seen_chapter: i64,
        status: &str,
    ) -> anyhow::Result<()> {
        Self::update_relationship_with_conn(
            &mut *self.pool.acquire().await?,
            id,
            relation_label,
            current_state,
            strength,
            polarity,
            confidence,
            importance_score,
            last_changed_chapter,
            last_seen_chapter,
            status,
        )
        .await
    }

    pub async fn get_by_id(&self, id: &str) -> anyhow::Result<Option<RelationshipRecord>> {
        let row = sqlx::query_as::<_, RelationshipRecord>(
            "SELECT id, book_id, subject_character_id, object_character_id, relation_group, relation_label, directionality, current_state, strength, polarity, confidence, importance_score, first_seen_chapter, last_changed_chapter, last_seen_chapter, status, created_at, updated_at
             FROM relationships WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list_by_book(
        &self,
        book_id: &str,
        group_filter: Option<&str>,
        min_importance: Option<f64>,
    ) -> anyhow::Result<Vec<RelationshipRecord>> {
        let mut sql = String::from(
            "SELECT id, book_id, subject_character_id, object_character_id, relation_group, relation_label, directionality, current_state, strength, polarity, confidence, importance_score, first_seen_chapter, last_changed_chapter, last_seen_chapter, status, created_at, updated_at
             FROM relationships WHERE book_id = ? AND status = 'active'",
        );
        if group_filter.is_some() {
            sql.push_str(" AND relation_group = ?");
        }
        if min_importance.is_some() {
            sql.push_str(" AND importance_score >= ?");
        }
        sql.push_str(" ORDER BY importance_score DESC");

        let mut query = sqlx::query_as::<_, RelationshipRecord>(&sql).bind(book_id);
        if let Some(g) = group_filter {
            query = query.bind(g);
        }
        if let Some(m) = min_importance {
            query = query.bind(m);
        }

        Ok(query.fetch_all(&self.pool).await?)
    }

    pub async fn list_active_by_book(
        &self,
        book_id: &str,
    ) -> anyhow::Result<Vec<RelationshipRecord>> {
        let rows = sqlx::query_as::<_, RelationshipRecord>(
            "SELECT id, book_id, subject_character_id, object_character_id, relation_group, relation_label, directionality, current_state, strength, polarity, confidence, importance_score, first_seen_chapter, last_changed_chapter, last_seen_chapter, status, created_at, updated_at
             FROM relationships WHERE book_id = ? AND status = 'active' ORDER BY importance_score DESC"
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn list_by_character(
        &self,
        book_id: &str,
        character_id: &str,
    ) -> anyhow::Result<Vec<RelationshipRecord>> {
        let rows = sqlx::query_as::<_, RelationshipRecord>(
            "SELECT id, book_id, subject_character_id, object_character_id, relation_group, relation_label, directionality, current_state, strength, polarity, confidence, importance_score, first_seen_chapter, last_changed_chapter, last_seen_chapter, status, created_at, updated_at
             FROM relationships WHERE book_id = ? AND (subject_character_id = ? OR object_character_id = ?) AND status = 'active' ORDER BY importance_score DESC"
        )
        .bind(book_id)
        .bind(character_id)
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn count_active_by_book(&self, book_id: &str) -> anyhow::Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM relationships WHERE book_id = ? AND status = 'active'",
        )
        .bind(book_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    pub async fn list_relationships_by_chapter(
        &self,
        book_id: &str,
        chapter_index: i64,
    ) -> anyhow::Result<Vec<(RelationshipEventRecord, RelationshipRecord)>> {
        let rows = sqlx::query(
            "SELECT
                re.id as ev_id, re.book_id as ev_book_id, re.relationship_id, re.event_type, re.relation_group as ev_relation_group, re.relation_label as ev_relation_label, re.state_after, re.strength_after, re.polarity_after, re.chapter_index, re.source_claim_id, re.confidence as ev_confidence, re.created_at as ev_created_at,
                r.id as rel_id, r.book_id as rel_book_id, r.subject_character_id, r.object_character_id, r.relation_group as rel_relation_group, r.relation_label as rel_relation_label, r.directionality, r.current_state, r.strength, r.polarity, r.confidence as rel_confidence, r.importance_score, r.first_seen_chapter, r.last_changed_chapter, r.last_seen_chapter, r.status, r.created_at as rel_created_at, r.updated_at
             FROM relationship_events re
             JOIN relationships r ON r.id = re.relationship_id
             WHERE re.book_id = ? AND re.chapter_index = ?
             ORDER BY re.created_at ASC"
        )
        .bind(book_id)
        .bind(chapter_index)
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let ev = RelationshipEventRecord {
                id: row.get("ev_id"),
                book_id: row.get("ev_book_id"),
                relationship_id: row.get("relationship_id"),
                event_type: row.get("event_type"),
                relation_group: row.get("ev_relation_group"),
                relation_label: row.get("ev_relation_label"),
                state_after: row.get("state_after"),
                strength_after: row.get("strength_after"),
                polarity_after: row.get("polarity_after"),
                chapter_index: row.get("chapter_index"),
                source_claim_id: row.get("source_claim_id"),
                confidence: row.get("ev_confidence"),
                created_at: row.get("ev_created_at"),
            };
            let rel = RelationshipRecord {
                id: row.get("rel_id"),
                book_id: row.get("rel_book_id"),
                subject_character_id: row.get("subject_character_id"),
                object_character_id: row.get("object_character_id"),
                relation_group: row.get("rel_relation_group"),
                relation_label: row.get("rel_relation_label"),
                directionality: row.get("directionality"),
                current_state: row.get("current_state"),
                strength: row.get("strength"),
                polarity: row.get("polarity"),
                confidence: row.get("rel_confidence"),
                importance_score: row.get("importance_score"),
                first_seen_chapter: row.get("first_seen_chapter"),
                last_changed_chapter: row.get("last_changed_chapter"),
                last_seen_chapter: row.get("last_seen_chapter"),
                status: row.get("status"),
                created_at: row.get("rel_created_at"),
                updated_at: row.get("updated_at"),
            };
            result.push((ev, rel));
        }

        Ok(result)
    }

    // --- Transaction-aware variants ---

    pub async fn create_relationship_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        subject_character_id: &str,
        object_character_id: &str,
        relation_group: &str,
        relation_label: &str,
        directionality: &str,
        current_state: Option<&str>,
        strength: f64,
        polarity: &str,
        confidence: f64,
        importance_score: f64,
        first_seen_chapter: i64,
    ) -> anyhow::Result<RelationshipRecord> {
        // Canonicalize undirected pairs: smaller ID always subject
        let (final_subject, final_object) = if directionality == "undirected" {
            if subject_character_id < object_character_id {
                (
                    subject_character_id.to_string(),
                    object_character_id.to_string(),
                )
            } else {
                (
                    object_character_id.to_string(),
                    subject_character_id.to_string(),
                )
            }
        } else {
            (
                subject_character_id.to_string(),
                object_character_id.to_string(),
            )
        };

        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO relationships (id, book_id, subject_character_id, object_character_id, relation_group, relation_label, directionality, current_state, strength, polarity, confidence, importance_score, first_seen_chapter, last_changed_chapter, last_seen_chapter, status, created_at, updated_at)
             VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)"
        )
        .bind(book_id)
        .bind(&final_subject)
        .bind(&final_object)
        .bind(relation_group)
        .bind(relation_label)
        .bind(directionality)
        .bind(current_state)
        .bind(strength)
        .bind(polarity)
        .bind(confidence)
        .bind(importance_score)
        .bind(first_seen_chapter)
        .bind(first_seen_chapter)
        .bind(first_seen_chapter)
        .bind(&now)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        // Re-fetch to get the generated id
        let record = Self::find_existing_with_conn(
            conn,
            book_id,
            &final_subject,
            &final_object,
            relation_group,
            directionality,
        )
        .await?
        .expect("relationship just inserted must exist");

        Ok(record)
    }

    pub async fn find_existing_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        subject_character_id: &str,
        object_character_id: &str,
        relation_group: &str,
        directionality: &str,
    ) -> anyhow::Result<Option<RelationshipRecord>> {
        let row = sqlx::query_as::<_, RelationshipRecord>(
            "SELECT id, book_id, subject_character_id, object_character_id, relation_group, relation_label, directionality, current_state, strength, polarity, confidence, importance_score, first_seen_chapter, last_changed_chapter, last_seen_chapter, status, created_at, updated_at
             FROM relationships WHERE book_id = ? AND subject_character_id = ? AND object_character_id = ? AND relation_group = ? AND directionality = ?"
        )
        .bind(book_id)
        .bind(subject_character_id)
        .bind(object_character_id)
        .bind(relation_group)
        .bind(directionality)
        .fetch_optional(conn)
        .await?;

        Ok(row)
    }

    pub async fn update_relationship_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
        relation_label: &str,
        current_state: Option<&str>,
        strength: f64,
        polarity: &str,
        confidence: f64,
        importance_score: f64,
        last_changed_chapter: i64,
        last_seen_chapter: i64,
        status: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE relationships SET relation_label = ?, current_state = ?, strength = ?, polarity = ?, confidence = ?, importance_score = ?, last_changed_chapter = ?, last_seen_chapter = ?, status = ?, updated_at = ? WHERE id = ?"
        )
        .bind(relation_label)
        .bind(current_state)
        .bind(strength)
        .bind(polarity)
        .bind(confidence)
        .bind(importance_score)
        .bind(last_changed_chapter)
        .bind(last_seen_chapter)
        .bind(status)
        .bind(&now)
        .bind(id)
        .execute(conn)
        .await?;

        Ok(())
    }
}

// --- RelationshipEventRepo ---

pub struct RelationshipEventRepo {
    pool: SqlitePool,
}

impl RelationshipEventRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_event(
        &self,
        book_id: &str,
        relationship_id: &str,
        event_type: &str,
        relation_group: &str,
        relation_label: &str,
        state_after: Option<&str>,
        strength_after: Option<f64>,
        polarity_after: Option<&str>,
        chapter_index: i64,
        source_claim_id: &str,
        confidence: f64,
    ) -> anyhow::Result<RelationshipEventRecord> {
        Self::create_event_with_conn(
            &mut *self.pool.acquire().await?,
            book_id,
            relationship_id,
            event_type,
            relation_group,
            relation_label,
            state_after,
            strength_after,
            polarity_after,
            chapter_index,
            source_claim_id,
            confidence,
        )
        .await
    }

    pub async fn list_by_relationship(
        &self,
        relationship_id: &str,
    ) -> anyhow::Result<Vec<RelationshipEventRecord>> {
        let rows = sqlx::query_as::<_, RelationshipEventRecord>(
            "SELECT id, book_id, relationship_id, event_type, relation_group, relation_label, state_after, strength_after, polarity_after, chapter_index, source_claim_id, confidence, created_at
             FROM relationship_events WHERE relationship_id = ? ORDER BY chapter_index ASC, created_at ASC"
        )
        .bind(relationship_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn count_by_relationship(&self, relationship_id: &str) -> anyhow::Result<i64> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM relationship_events WHERE relationship_id = ?")
                .bind(relationship_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(row.0)
    }

    // --- Transaction-aware variants ---

    pub async fn create_event_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        relationship_id: &str,
        event_type: &str,
        relation_group: &str,
        relation_label: &str,
        state_after: Option<&str>,
        strength_after: Option<f64>,
        polarity_after: Option<&str>,
        chapter_index: i64,
        source_claim_id: &str,
        confidence: f64,
    ) -> anyhow::Result<RelationshipEventRecord> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO relationship_events (id, book_id, relationship_id, event_type, relation_group, relation_label, state_after, strength_after, polarity_after, chapter_index, source_claim_id, confidence, created_at)
             VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(book_id)
        .bind(relationship_id)
        .bind(event_type)
        .bind(relation_group)
        .bind(relation_label)
        .bind(state_after)
        .bind(strength_after)
        .bind(polarity_after)
        .bind(chapter_index)
        .bind(source_claim_id)
        .bind(confidence)
        .bind(&now)
        .execute(&mut *conn)
        .await?;

        // Re-fetch by source_claim_id (unique) to get the generated id
        let record = sqlx::query_as::<_, RelationshipEventRecord>(
            "SELECT id, book_id, relationship_id, event_type, relation_group, relation_label, state_after, strength_after, polarity_after, chapter_index, source_claim_id, confidence, created_at
             FROM relationship_events WHERE source_claim_id = ?"
        )
        .bind(source_claim_id)
        .fetch_one(conn)
        .await?;

        Ok(record)
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup_test_db() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("reader-rel-repo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    /// Helper: insert two entities and a chapter+segment+source_span+ai_run+claim
    /// for FK dependencies. Returns (entity_id_1, entity_id_2).
    async fn seed_prereqs(pool: &SqlitePool) -> (String, String) {
        let e1 = uuid::Uuid::new_v4().to_string();
        let e2 = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES (?, 'b1', 'character', 'Alice', 'Alice', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))"
        )
        .bind(&e1)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES (?, 'b1', 'character', 'Bob', 'Bob', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))"
        )
        .bind(&e2)
        .execute(pool)
        .await
        .unwrap();
        (e1, e2)
    }

    /// Helper: insert chapter/span/ai_run/claim FK chain, return claim_id
    async fn seed_claim_chain(pool: &SqlitePool, chapter: i64) -> String {
        let ch_id = uuid::Uuid::new_v4().to_string();
        let seg_id = uuid::Uuid::new_v4().to_string();
        let span_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let claim_id = uuid::Uuid::new_v4().to_string();

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', ?, 'text', 'hash', datetime('now'))")
            .bind(&ch_id).bind(chapter).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'hash', 0, datetime('now'))")
            .bind(&seg_id).bind(&ch_id).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 0, 0, 10, 'text', datetime('now'))")
            .bind(&span_id).bind(&ch_id).bind(&seg_id).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, 'b1', ?, 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
            .bind(&run_id).bind(&ch_id).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', ?, 'relationship_update', 'test', ?, ?, 0.9, 'low', 'proposed', datetime('now'), datetime('now'))")
            .bind(&claim_id).bind(chapter).bind(&span_id).bind(&run_id).execute(pool).await.unwrap();

        claim_id
    }

    #[tokio::test]
    async fn crud_roundtrip() {
        let pool = setup_test_db().await;
        let repo = RelationshipRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;

        // Create
        let rel = repo
            .create_relationship(
                "b1",
                &e1,
                &e2,
                "friendship",
                "friends",
                "undirected",
                None,
                0.5,
                "positive",
                0.8,
                0.7,
                1,
            )
            .await
            .unwrap();
        assert!(!rel.id.is_empty());
        assert_eq!(rel.book_id, "b1");
        assert_eq!(rel.relation_group, "friendship");
        assert_eq!(rel.status, "active");

        // Get by id
        let fetched = repo.get_by_id(&rel.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, rel.id);
        let (expected_sub, expected_obj) = if e1 < e2 { (&e1, &e2) } else { (&e2, &e1) };
        assert_eq!(&fetched.subject_character_id, expected_sub);
        assert_eq!(&fetched.object_character_id, expected_obj);

        // Update
        repo.update_relationship(
            &rel.id,
            "close friends",
            Some("close friends"),
            0.9,
            "positive",
            0.9,
            0.8,
            2,
            2,
            "active",
        )
        .await
        .unwrap();
        let updated = repo.get_by_id(&rel.id).await.unwrap().unwrap();
        assert_eq!(updated.current_state.as_deref(), Some("close friends"));
        assert!((updated.strength - 0.9).abs() < f64::EPSILON);
        assert_eq!(updated.last_changed_chapter, 2);
    }

    #[tokio::test]
    async fn find_existing_exact_match() {
        let pool = setup_test_db().await;
        let repo = RelationshipRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;

        let rel = repo
            .create_relationship(
                "b1",
                &e1,
                &e2,
                "rivalry",
                "rivals",
                "directed",
                Some("tense"),
                0.6,
                "negative",
                0.7,
                0.5,
                1,
            )
            .await
            .unwrap();

        // Exact match
        let found = repo
            .find_existing("b1", &e1, &e2, "rivalry", "directed")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, rel.id);

        // Different group should not match
        let not_found = repo
            .find_existing("b1", &e1, &e2, "friendship", "directed")
            .await
            .unwrap();
        assert!(not_found.is_none());

        // Different directionality should not match
        let not_found = repo
            .find_existing("b1", &e1, &e2, "rivalry", "undirected")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn event_roundtrip() {
        let pool = setup_test_db().await;
        let rel_repo = RelationshipRepo::new(pool.clone());
        let ev_repo = RelationshipEventRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;
        let claim_id = seed_claim_chain(&pool, 1).await;

        let rel = rel_repo
            .create_relationship(
                "b1",
                &e1,
                &e2,
                "alliance",
                "allies",
                "undirected",
                None,
                0.5,
                "positive",
                0.7,
                0.6,
                1,
            )
            .await
            .unwrap();

        let ev = ev_repo
            .create_event(
                "b1",
                &rel.id,
                "established",
                "alliance",
                "allies",
                Some("cordial"),
                Some(0.8),
                Some("positive"),
                1,
                &claim_id,
                0.9,
            )
            .await
            .unwrap();
        assert_eq!(ev.relationship_id, rel.id);
        assert_eq!(ev.event_type, "established");

        let events = ev_repo.list_by_relationship(&rel.id).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, ev.id);

        let count = ev_repo.count_by_relationship(&rel.id).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn enum_validation() {
        // relation_group
        assert!(validate_relation_group("family"));
        assert!(validate_relation_group("romance"));
        assert!(validate_relation_group("unknown_significant"));
        assert!(!validate_relation_group("ally"));
        assert!(!validate_relation_group(""));

        // directionality
        assert!(validate_directionality("directed"));
        assert!(validate_directionality("undirected"));
        assert!(!validate_directionality("both"));

        // polarity
        assert!(validate_polarity("positive"));
        assert!(validate_polarity("negative"));
        assert!(validate_polarity("mixed"));
        assert!(validate_polarity("neutral"));
        assert!(validate_polarity("unknown"));
        assert!(!validate_polarity("bad"));

        // status
        assert!(validate_relationship_status("active"));
        assert!(validate_relationship_status("inactive"));
        assert!(validate_relationship_status("uncertain"));
        assert!(validate_relationship_status("rejected"));
        assert!(!validate_relationship_status("deleted"));
    }

    #[tokio::test]
    async fn list_by_book_with_group_filter() {
        let pool = setup_test_db().await;
        let repo = RelationshipRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;

        // Also insert a third entity for a second relationship
        let e3 = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES (?, 'b1', 'character', 'Charlie', 'Charlie', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))"
        )
        .bind(&e3)
        .execute(&pool)
        .await
        .unwrap();

        repo.create_relationship(
            "b1",
            &e1,
            &e2,
            "friendship",
            "friends",
            "undirected",
            None,
            0.5,
            "positive",
            0.8,
            0.7,
            1,
        )
        .await
        .unwrap();
        repo.create_relationship(
            "b1", &e1, &e3, "rivalry", "rivals", "directed", None, 0.5, "negative", 0.7, 0.6, 1,
        )
        .await
        .unwrap();

        // No filter
        let all = repo.list_by_book("b1", None, None).await.unwrap();
        assert_eq!(all.len(), 2);

        // Group filter
        let filtered = repo
            .list_by_book("b1", Some("friendship"), None)
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].relation_group, "friendship");

        // Min importance
        let filtered = repo.list_by_book("b1", None, Some(0.65)).await.unwrap();
        assert_eq!(filtered.len(), 1);
    }

    #[tokio::test]
    async fn list_by_character() {
        let pool = setup_test_db().await;
        let repo = RelationshipRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;

        let e3 = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES (?, 'b1', 'character', 'Charlie', 'Charlie', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))"
        )
        .bind(&e3)
        .execute(&pool)
        .await
        .unwrap();

        // e1--e2 (friendship)
        repo.create_relationship(
            "b1",
            &e1,
            &e2,
            "friendship",
            "friends",
            "undirected",
            None,
            0.5,
            "positive",
            0.8,
            0.7,
            1,
        )
        .await
        .unwrap();
        // e1--e3 (alliance)
        repo.create_relationship(
            "b1",
            &e1,
            &e3,
            "alliance",
            "allies",
            "undirected",
            None,
            0.5,
            "positive",
            0.7,
            0.6,
            1,
        )
        .await
        .unwrap();

        // e1 appears in 2 relationships
        let rels = repo.list_by_character("b1", &e1).await.unwrap();
        assert_eq!(rels.len(), 2);

        // e2 appears in 1
        let rels = repo.list_by_character("b1", &e2).await.unwrap();
        assert_eq!(rels.len(), 1);

        // e3 appears in 1
        let rels = repo.list_by_character("b1", &e3).await.unwrap();
        assert_eq!(rels.len(), 1);
    }

    #[tokio::test]
    async fn count_active_by_book() {
        let pool = setup_test_db().await;
        let repo = RelationshipRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;

        assert_eq!(repo.count_active_by_book("b1").await.unwrap(), 0);

        let rel = repo
            .create_relationship(
                "b1",
                &e1,
                &e2,
                "friendship",
                "friends",
                "undirected",
                None,
                0.5,
                "positive",
                0.8,
                0.7,
                1,
            )
            .await
            .unwrap();
        assert_eq!(repo.count_active_by_book("b1").await.unwrap(), 1);

        // Set inactive
        repo.update_relationship(
            &rel.id, "inactive", None, 0.5, "neutral", 0.5, 0.5, 1, 1, "inactive",
        )
        .await
        .unwrap();
        assert_eq!(repo.count_active_by_book("b1").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn list_relationships_by_chapter_join() {
        let pool = setup_test_db().await;
        let repo = RelationshipRepo::new(pool.clone());
        let ev_repo = RelationshipEventRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;
        let claim_id = seed_claim_chain(&pool, 5).await;

        let rel = repo
            .create_relationship(
                "b1",
                &e1,
                &e2,
                "mentorship",
                "mentor",
                "directed",
                None,
                0.5,
                "positive",
                0.8,
                0.7,
                1,
            )
            .await
            .unwrap();

        ev_repo
            .create_event(
                "b1",
                &rel.id,
                "established",
                "mentorship",
                "mentor",
                Some("teaching"),
                Some(0.9),
                Some("positive"),
                5,
                &claim_id,
                0.95,
            )
            .await
            .unwrap();

        let pairs = repo.list_relationships_by_chapter("b1", 5).await.unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0.event_type, "established");
        assert_eq!(pairs[0].1.id, rel.id);

        // No events at chapter 99
        let pairs = repo.list_relationships_by_chapter("b1", 99).await.unwrap();
        assert!(pairs.is_empty());
    }

    #[tokio::test]
    async fn unique_source_claim_prevents_duplicates() {
        let pool = setup_test_db().await;
        let rel_repo = RelationshipRepo::new(pool.clone());
        let ev_repo = RelationshipEventRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;
        let claim_id = seed_claim_chain(&pool, 1).await;

        let rel = rel_repo
            .create_relationship(
                "b1",
                &e1,
                &e2,
                "family",
                "siblings",
                "undirected",
                None,
                0.5,
                "positive",
                0.9,
                0.8,
                1,
            )
            .await
            .unwrap();

        // First event succeeds
        ev_repo
            .create_event(
                "b1",
                &rel.id,
                "established",
                "family",
                "siblings",
                None,
                None,
                None,
                1,
                &claim_id,
                0.9,
            )
            .await
            .unwrap();

        // Second event with same source_claim_id should fail
        let result = ev_repo
            .create_event(
                "b1", &rel.id, "updated", "family", "siblings", None, None, None, 1, &claim_id, 0.8,
            )
            .await;
        assert!(
            result.is_err(),
            "UNIQUE constraint should reject duplicate source_claim_id"
        );
    }

    #[tokio::test]
    async fn active_list_and_inactive_excluded() {
        let pool = setup_test_db().await;
        let repo = RelationshipRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;

        let rel = repo
            .create_relationship(
                "b1",
                &e1,
                &e2,
                "romance",
                "lovers",
                "undirected",
                None,
                0.5,
                "positive",
                0.9,
                0.9,
                1,
            )
            .await
            .unwrap();

        let active = repo.list_active_by_book("b1").await.unwrap();
        assert_eq!(active.len(), 1);

        // Mark inactive
        repo.update_relationship(
            &rel.id, "inactive", None, 0.5, "neutral", 0.5, 0.5, 1, 1, "inactive",
        )
        .await
        .unwrap();

        let active = repo.list_active_by_book("b1").await.unwrap();
        assert_eq!(active.len(), 0);
    }

    #[tokio::test]
    async fn undirected_pair_canonicalized() {
        let pool = setup_test_db().await;
        let repo = RelationshipRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;

        // Pass the larger ID as subject to test canonicalization
        let (larger, smaller) = if e1 > e2 { (&e1, &e2) } else { (&e2, &e1) };
        let rel = repo
            .create_relationship(
                "b1",
                larger,  // subject = larger ID
                smaller, // object = smaller ID
                "friendship",
                "friends",
                "undirected",
                None,
                0.5,
                "positive",
                0.8,
                0.7,
                1,
            )
            .await
            .unwrap();

        // Verify canonicalized: smaller ID should be subject
        let fetched = repo.get_by_id(&rel.id).await.unwrap().unwrap();
        assert_eq!(
            &fetched.subject_character_id, smaller,
            "undirected: smaller ID should be subject"
        );
        assert_eq!(
            &fetched.object_character_id, larger,
            "undirected: larger ID should be object"
        );
    }

    #[tokio::test]
    async fn directed_pair_not_canonicalized() {
        let pool = setup_test_db().await;
        let repo = RelationshipRepo::new(pool.clone());
        let (e1, e2) = seed_prereqs(&pool).await;

        // Create directed with subject=e2, object=e1
        let rel = repo
            .create_relationship(
                "b1",
                &e2, // subject
                &e1, // object
                "mentorship",
                "mentor",
                "directed",
                None,
                0.5,
                "positive",
                0.8,
                0.7,
                1,
            )
            .await
            .unwrap();

        // Verify NOT canonicalized: original order preserved
        let fetched = repo.get_by_id(&rel.id).await.unwrap().unwrap();
        assert_eq!(
            fetched.subject_character_id, e2,
            "directed: subject should be preserved"
        );
        assert_eq!(
            fetched.object_character_id, e1,
            "directed: object should be preserved"
        );
    }
}
