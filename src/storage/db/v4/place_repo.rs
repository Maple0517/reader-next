use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PlaceDetailRecord {
    pub entity_id: String,
    pub book_id: String,
    pub place_type: String,
    pub parent_place_id: Option<String>,
    pub scale_level: i64,
    pub importance_score: f64,
    pub map_visible: i64,
    pub first_seen_chapter: i64,
    pub last_seen_chapter: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PlaceEdgeRecord {
    pub id: String,
    pub book_id: String,
    pub from_place_id: String,
    pub to_place_id: String,
    pub edge_type: String,
    pub direction_hint: Option<String>,
    pub distance_hint: Option<String>,
    pub confidence: f64,
    pub source_claim_id: String,
    pub latest_source_claim_id: Option<String>,
    pub first_seen_chapter: i64,
    pub last_seen_chapter: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PlaceEdgeSourceRecord {
    pub id: String,
    pub book_id: String,
    pub edge_id: String,
    pub source_claim_id: String,
    pub chapter_index: i64,
    pub confidence: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PlaceEdgeConflictRecord {
    pub id: String,
    pub book_id: String,
    pub new_edge_claim_id: String,
    pub existing_edge_id: Option<String>,
    pub conflict_type: String,
    pub reason_code: String,
    pub judge_output_json: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MapLayoutSnapshotRecord {
    pub id: String,
    pub book_id: String,
    pub max_chapter: i64,
    pub layout_version: String,
    pub layout_json: String,
    pub source_edge_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EntityLinkRecord {
    pub id: String,
    pub book_id: String,
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub link_type: String,
    pub source_claim_id: String,
    pub confidence: f64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct PlaceRepo {
    pool: SqlitePool,
}

impl PlaceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_place_detail(
        &self,
        book_id: &str,
        entity_id: &str,
        place_type: &str,
        parent_place_id: Option<&str>,
        scale_level: i64,
        importance_score: f64,
        map_visible: bool,
        first_seen_chapter: i64,
        last_seen_chapter: i64,
        status: &str,
    ) -> anyhow::Result<PlaceDetailRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::upsert_place_detail_with_conn(
            &mut conn,
            book_id,
            entity_id,
            place_type,
            parent_place_id,
            scale_level,
            importance_score,
            map_visible,
            first_seen_chapter,
            last_seen_chapter,
            status,
        )
        .await
    }

    pub async fn upsert_place_detail_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        entity_id: &str,
        place_type: &str,
        parent_place_id: Option<&str>,
        scale_level: i64,
        importance_score: f64,
        map_visible: bool,
        first_seen_chapter: i64,
        last_seen_chapter: i64,
        status: &str,
    ) -> anyhow::Result<PlaceDetailRecord> {
        Self::ensure_place_entity_with_conn(conn, book_id, entity_id).await?;
        if let Some(parent_id) = parent_place_id {
            Self::ensure_place_entity_with_conn(conn, book_id, parent_id).await?;
        }

        sqlx::query(
            "INSERT INTO place_details (
                entity_id, book_id, place_type, parent_place_id, scale_level, importance_score,
                map_visible, first_seen_chapter, last_seen_chapter, status, created_at, updated_at
             )
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))
             ON CONFLICT(entity_id) DO UPDATE SET
                place_type = excluded.place_type,
                parent_place_id = excluded.parent_place_id,
                scale_level = excluded.scale_level,
                importance_score = excluded.importance_score,
                map_visible = excluded.map_visible,
                first_seen_chapter = MIN(place_details.first_seen_chapter, excluded.first_seen_chapter),
                last_seen_chapter = MAX(place_details.last_seen_chapter, excluded.last_seen_chapter),
                status = excluded.status,
                updated_at = datetime('now')",
        )
        .bind(entity_id)
        .bind(book_id)
        .bind(place_type)
        .bind(parent_place_id)
        .bind(scale_level)
        .bind(importance_score)
        .bind(if map_visible { 1 } else { 0 })
        .bind(first_seen_chapter)
        .bind(last_seen_chapter)
        .bind(status)
        .execute(&mut *conn)
        .await?;

        Self::get_place_detail_with_conn(conn, entity_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("place detail not found after upsert: {}", entity_id))
    }

    pub async fn get_place_detail(
        &self,
        entity_id: &str,
    ) -> anyhow::Result<Option<PlaceDetailRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::get_place_detail_with_conn(&mut conn, entity_id).await
    }

    pub async fn get_place_detail_with_conn(
        conn: &mut SqliteConnection,
        entity_id: &str,
    ) -> anyhow::Result<Option<PlaceDetailRecord>> {
        let row = sqlx::query_as::<_, PlaceDetailRecord>(
            "SELECT entity_id, book_id, place_type, parent_place_id, scale_level,
                    importance_score, map_visible, first_seen_chapter, last_seen_chapter,
                    status, created_at, updated_at
             FROM place_details
             WHERE entity_id = ?",
        )
        .bind(entity_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn find_or_create_edge(
        &self,
        book_id: &str,
        from_place_id: &str,
        to_place_id: &str,
        edge_type: &str,
        direction_hint: Option<&str>,
        distance_hint: Option<&str>,
        confidence: f64,
        source_claim_id: &str,
        chapter_index: i64,
    ) -> anyhow::Result<PlaceEdgeRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::find_or_create_edge_with_conn(
            &mut conn,
            book_id,
            from_place_id,
            to_place_id,
            edge_type,
            direction_hint,
            distance_hint,
            confidence,
            source_claim_id,
            chapter_index,
        )
        .await
    }

    pub async fn find_or_create_edge_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        from_place_id: &str,
        to_place_id: &str,
        edge_type: &str,
        direction_hint: Option<&str>,
        distance_hint: Option<&str>,
        confidence: f64,
        source_claim_id: &str,
        chapter_index: i64,
    ) -> anyhow::Result<PlaceEdgeRecord> {
        let (from_place_id, to_place_id) =
            canonicalize_edge_pair(from_place_id, to_place_id, edge_type);
        Self::ensure_place_entity_with_conn(conn, book_id, &from_place_id).await?;
        Self::ensure_place_entity_with_conn(conn, book_id, &to_place_id).await?;

        if let Some(existing) = Self::find_active_edge_with_conn(
            conn,
            book_id,
            &from_place_id,
            &to_place_id,
            edge_type,
            direction_hint,
        )
        .await?
        {
            sqlx::query(
                "UPDATE place_edges
                 SET confidence = MAX(confidence, ?),
                     latest_source_claim_id = ?,
                     last_seen_chapter = MAX(last_seen_chapter, ?),
                     updated_at = datetime('now')
                 WHERE id = ?",
            )
            .bind(confidence)
            .bind(source_claim_id)
            .bind(chapter_index)
            .bind(&existing.id)
            .execute(&mut *conn)
            .await?;
            Self::insert_edge_source_with_conn(
                conn,
                book_id,
                &existing.id,
                source_claim_id,
                chapter_index,
                confidence,
            )
            .await?;
            return Self::get_edge_by_id_with_conn(conn, &existing.id)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("place edge not found after update: {}", existing.id)
                });
        }

        let edge = sqlx::query_as::<_, PlaceEdgeRecord>(
            "INSERT INTO place_edges (
                id, book_id, from_place_id, to_place_id, edge_type, direction_hint, distance_hint,
                confidence, source_claim_id, latest_source_claim_id, first_seen_chapter,
                last_seen_chapter, status, created_at, updated_at
             )
             VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', datetime('now'), datetime('now'))
             RETURNING id, book_id, from_place_id, to_place_id, edge_type, direction_hint,
                       distance_hint, confidence, source_claim_id, latest_source_claim_id,
                       first_seen_chapter, last_seen_chapter, status, created_at, updated_at",
        )
        .bind(book_id)
        .bind(&from_place_id)
        .bind(&to_place_id)
        .bind(edge_type)
        .bind(direction_hint)
        .bind(distance_hint)
        .bind(confidence)
        .bind(source_claim_id)
        .bind(source_claim_id)
        .bind(chapter_index)
        .bind(chapter_index)
        .fetch_one(&mut *conn)
        .await?;

        Self::insert_edge_source_with_conn(
            conn,
            book_id,
            &edge.id,
            source_claim_id,
            chapter_index,
            confidence,
        )
        .await?;
        Ok(edge)
    }

    pub async fn get_edge_by_id(&self, edge_id: &str) -> anyhow::Result<Option<PlaceEdgeRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::get_edge_by_id_with_conn(&mut conn, edge_id).await
    }

    pub async fn get_edge_by_id_with_conn(
        conn: &mut SqliteConnection,
        edge_id: &str,
    ) -> anyhow::Result<Option<PlaceEdgeRecord>> {
        let row = sqlx::query_as::<_, PlaceEdgeRecord>(
            "SELECT id, book_id, from_place_id, to_place_id, edge_type, direction_hint,
                    distance_hint, confidence, source_claim_id, latest_source_claim_id,
                    first_seen_chapter, last_seen_chapter, status, created_at, updated_at
             FROM place_edges
             WHERE id = ?",
        )
        .bind(edge_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn find_active_edge_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        from_place_id: &str,
        to_place_id: &str,
        edge_type: &str,
        direction_hint: Option<&str>,
    ) -> anyhow::Result<Option<PlaceEdgeRecord>> {
        let row = sqlx::query_as::<_, PlaceEdgeRecord>(
            "SELECT id, book_id, from_place_id, to_place_id, edge_type, direction_hint,
                    distance_hint, confidence, source_claim_id, latest_source_claim_id,
                    first_seen_chapter, last_seen_chapter, status, created_at, updated_at
             FROM place_edges
             WHERE book_id = ? AND from_place_id = ? AND to_place_id = ?
               AND edge_type = ? AND COALESCE(direction_hint, '') = COALESCE(?, '')
               AND status = 'active'
             LIMIT 1",
        )
        .bind(book_id)
        .bind(from_place_id)
        .bind(to_place_id)
        .bind(edge_type)
        .bind(direction_hint)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn insert_edge_source(
        &self,
        book_id: &str,
        edge_id: &str,
        source_claim_id: &str,
        chapter_index: i64,
        confidence: f64,
    ) -> anyhow::Result<PlaceEdgeSourceRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::insert_edge_source_with_conn(
            &mut conn,
            book_id,
            edge_id,
            source_claim_id,
            chapter_index,
            confidence,
        )
        .await
    }

    pub async fn insert_edge_source_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        edge_id: &str,
        source_claim_id: &str,
        chapter_index: i64,
        confidence: f64,
    ) -> anyhow::Result<PlaceEdgeSourceRecord> {
        if let Some(existing) =
            Self::find_edge_source_with_conn(conn, edge_id, source_claim_id).await?
        {
            return Ok(existing);
        }

        let row = sqlx::query_as::<_, PlaceEdgeSourceRecord>(
            "INSERT INTO place_edge_sources (
                id, book_id, edge_id, source_claim_id, chapter_index, confidence, created_at
             )
             VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, datetime('now'))
             RETURNING id, book_id, edge_id, source_claim_id, chapter_index, confidence, created_at",
        )
        .bind(book_id)
        .bind(edge_id)
        .bind(source_claim_id)
        .bind(chapter_index)
        .bind(confidence)
        .fetch_one(&mut *conn)
        .await?;
        Ok(row)
    }

    async fn find_edge_source_with_conn(
        conn: &mut SqliteConnection,
        edge_id: &str,
        source_claim_id: &str,
    ) -> anyhow::Result<Option<PlaceEdgeSourceRecord>> {
        let row = sqlx::query_as::<_, PlaceEdgeSourceRecord>(
            "SELECT id, book_id, edge_id, source_claim_id, chapter_index, confidence, created_at
             FROM place_edge_sources
             WHERE edge_id = ? AND source_claim_id = ?
             LIMIT 1",
        )
        .bind(edge_id)
        .bind(source_claim_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn list_edge_sources(
        &self,
        edge_id: &str,
    ) -> anyhow::Result<Vec<PlaceEdgeSourceRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::list_edge_sources_with_conn(&mut conn, edge_id).await
    }

    pub async fn list_edge_sources_with_conn(
        conn: &mut SqliteConnection,
        edge_id: &str,
    ) -> anyhow::Result<Vec<PlaceEdgeSourceRecord>> {
        let rows = sqlx::query_as::<_, PlaceEdgeSourceRecord>(
            "SELECT id, book_id, edge_id, source_claim_id, chapter_index, confidence, created_at
             FROM place_edge_sources
             WHERE edge_id = ?
             ORDER BY chapter_index ASC, created_at ASC, id ASC",
        )
        .bind(edge_id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows)
    }

    pub async fn insert_conflict(
        &self,
        book_id: &str,
        new_edge_claim_id: &str,
        existing_edge_id: Option<&str>,
        conflict_type: &str,
        reason_code: &str,
        judge_output_json: Option<&str>,
    ) -> anyhow::Result<PlaceEdgeConflictRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::insert_conflict_with_conn(
            &mut conn,
            book_id,
            new_edge_claim_id,
            existing_edge_id,
            conflict_type,
            reason_code,
            judge_output_json,
        )
        .await
    }

    pub async fn insert_conflict_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        new_edge_claim_id: &str,
        existing_edge_id: Option<&str>,
        conflict_type: &str,
        reason_code: &str,
        judge_output_json: Option<&str>,
    ) -> anyhow::Result<PlaceEdgeConflictRecord> {
        let row = sqlx::query_as::<_, PlaceEdgeConflictRecord>(
            "INSERT INTO place_edge_conflicts (
                id, book_id, new_edge_claim_id, existing_edge_id, conflict_type,
                reason_code, judge_output_json, status, created_at
             )
             VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, 'open', datetime('now'))
             RETURNING id, book_id, new_edge_claim_id, existing_edge_id, conflict_type,
                       reason_code, judge_output_json, status, created_at",
        )
        .bind(book_id)
        .bind(new_edge_claim_id)
        .bind(existing_edge_id)
        .bind(conflict_type)
        .bind(reason_code)
        .bind(judge_output_json)
        .fetch_one(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn list_conflicts(
        &self,
        book_id: &str,
        status: Option<&str>,
    ) -> anyhow::Result<Vec<PlaceEdgeConflictRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::list_conflicts_with_conn(&mut conn, book_id, status).await
    }

    pub async fn list_conflicts_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        status: Option<&str>,
    ) -> anyhow::Result<Vec<PlaceEdgeConflictRecord>> {
        let rows = if let Some(status) = status {
            sqlx::query_as::<_, PlaceEdgeConflictRecord>(
                "SELECT id, book_id, new_edge_claim_id, existing_edge_id, conflict_type,
                        reason_code, judge_output_json, status, created_at
                 FROM place_edge_conflicts
                 WHERE book_id = ? AND status = ?
                 ORDER BY created_at ASC, id ASC",
            )
            .bind(book_id)
            .bind(status)
            .fetch_all(&mut *conn)
            .await?
        } else {
            sqlx::query_as::<_, PlaceEdgeConflictRecord>(
                "SELECT id, book_id, new_edge_claim_id, existing_edge_id, conflict_type,
                        reason_code, judge_output_json, status, created_at
                 FROM place_edge_conflicts
                 WHERE book_id = ?
                 ORDER BY created_at ASC, id ASC",
            )
            .bind(book_id)
            .fetch_all(&mut *conn)
            .await?
        };
        Ok(rows)
    }

    pub async fn create_layout_snapshot(
        &self,
        book_id: &str,
        max_chapter: i64,
        layout_version: &str,
        layout_json: &str,
        source_edge_hash: &str,
    ) -> anyhow::Result<MapLayoutSnapshotRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::create_layout_snapshot_with_conn(
            &mut conn,
            book_id,
            max_chapter,
            layout_version,
            layout_json,
            source_edge_hash,
        )
        .await
    }

    pub async fn create_layout_snapshot_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        max_chapter: i64,
        layout_version: &str,
        layout_json: &str,
        source_edge_hash: &str,
    ) -> anyhow::Result<MapLayoutSnapshotRecord> {
        let row = sqlx::query_as::<_, MapLayoutSnapshotRecord>(
            "INSERT INTO map_layout_snapshots (
                id, book_id, max_chapter, layout_version, layout_json, source_edge_hash, created_at
             )
             VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, datetime('now'))
             RETURNING id, book_id, max_chapter, layout_version, layout_json, source_edge_hash, created_at",
        )
        .bind(book_id)
        .bind(max_chapter)
        .bind(layout_version)
        .bind(layout_json)
        .bind(source_edge_hash)
        .fetch_one(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn latest_layout_snapshot(
        &self,
        book_id: &str,
        layout_version: &str,
    ) -> anyhow::Result<Option<MapLayoutSnapshotRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::latest_layout_snapshot_with_conn(&mut conn, book_id, layout_version).await
    }

    pub async fn latest_layout_snapshot_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        layout_version: &str,
    ) -> anyhow::Result<Option<MapLayoutSnapshotRecord>> {
        let row = sqlx::query_as::<_, MapLayoutSnapshotRecord>(
            "SELECT id, book_id, max_chapter, layout_version, layout_json, source_edge_hash, created_at
             FROM map_layout_snapshots
             WHERE book_id = ? AND layout_version = ?
             ORDER BY max_chapter DESC, created_at DESC, id DESC
             LIMIT 1",
        )
        .bind(book_id)
        .bind(layout_version)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn find_or_create_entity_link(
        &self,
        book_id: &str,
        entity_a_id: &str,
        entity_b_id: &str,
        link_type: &str,
        source_claim_id: &str,
        confidence: f64,
    ) -> anyhow::Result<EntityLinkRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::find_or_create_entity_link_with_conn(
            &mut conn,
            book_id,
            entity_a_id,
            entity_b_id,
            link_type,
            source_claim_id,
            confidence,
        )
        .await
    }

    pub async fn find_or_create_entity_link_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        entity_a_id: &str,
        entity_b_id: &str,
        link_type: &str,
        source_claim_id: &str,
        confidence: f64,
    ) -> anyhow::Result<EntityLinkRecord> {
        let (entity_a_id, entity_b_id) = if link_type == "related_entity" {
            canonicalize_pair(entity_a_id, entity_b_id)
        } else {
            (entity_a_id.to_string(), entity_b_id.to_string())
        };

        let row = sqlx::query_as::<_, EntityLinkRecord>(
            "INSERT INTO entity_links (
                id, book_id, entity_a_id, entity_b_id, link_type, source_claim_id,
                confidence, status, created_at, updated_at
             )
             VALUES (lower(hex(randomblob(16))), ?, ?, ?, ?, ?, ?, 'active', datetime('now'), datetime('now'))
             ON CONFLICT(book_id, entity_a_id, entity_b_id, link_type, status)
             WHERE status = 'active'
             DO UPDATE SET id = entity_links.id
             RETURNING id, book_id, entity_a_id, entity_b_id, link_type, source_claim_id,
                       confidence, status, created_at, updated_at",
        )
        .bind(book_id)
        .bind(&entity_a_id)
        .bind(&entity_b_id)
        .bind(link_type)
        .bind(source_claim_id)
        .bind(confidence)
        .fetch_one(&mut *conn)
        .await?;
        Ok(row)
    }

    pub async fn list_entity_links_for_entity(
        &self,
        book_id: &str,
        entity_id: &str,
    ) -> anyhow::Result<Vec<EntityLinkRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::list_entity_links_for_entity_with_conn(&mut conn, book_id, entity_id).await
    }

    pub async fn list_entity_links_for_entity_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        entity_id: &str,
    ) -> anyhow::Result<Vec<EntityLinkRecord>> {
        let rows = sqlx::query_as::<_, EntityLinkRecord>(
            "SELECT id, book_id, entity_a_id, entity_b_id, link_type, source_claim_id,
                    confidence, status, created_at, updated_at
             FROM entity_links
             WHERE book_id = ? AND status = 'active'
               AND (entity_a_id = ? OR entity_b_id = ?)
             ORDER BY created_at ASC, id ASC",
        )
        .bind(book_id)
        .bind(entity_id)
        .bind(entity_id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows)
    }

    pub fn source_edge_hash(edges: &[PlaceEdgeRecord]) -> String {
        let mut parts = edges
            .iter()
            .map(|edge| {
                format!(
                    "{}|{}|{}|{}|{}|{}",
                    edge.from_place_id,
                    edge.to_place_id,
                    edge.edge_type,
                    edge.direction_hint.as_deref().unwrap_or(""),
                    edge.status,
                    edge.last_seen_chapter
                )
            })
            .collect::<Vec<_>>();
        parts.sort();
        stable_hash(&parts.join("\n"))
    }

    async fn ensure_place_entity_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        entity_id: &str,
    ) -> anyhow::Result<()> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT entity_type FROM entities WHERE book_id = ? AND id = ?")
                .bind(book_id)
                .bind(entity_id)
                .fetch_optional(&mut *conn)
                .await?;
        match row {
            Some((entity_type,)) if entity_type == "place" => Ok(()),
            Some((entity_type,)) => {
                anyhow::bail!("entity {} is {}, expected place", entity_id, entity_type)
            }
            None => anyhow::bail!("place entity not found: {}", entity_id),
        }
    }
}

fn canonicalize_edge_pair(
    from_place_id: &str,
    to_place_id: &str,
    edge_type: &str,
) -> (String, String) {
    if is_undirected_edge(edge_type) {
        canonicalize_pair(from_place_id, to_place_id)
    } else {
        (from_place_id.to_string(), to_place_id.to_string())
    }
}

fn canonicalize_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

fn is_undirected_edge(edge_type: &str) -> bool {
    matches!(
        edge_type,
        "near" | "adjacent_to" | "connects_to" | "unknown_spatial"
    )
}

fn stable_hash(value: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::PlaceRepo;
    use crate::storage::db;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("reader-place-repo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    async fn seed_base(pool: &SqlitePool) {
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg1', 'b1', 'ch1', 'hash', 0, datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('ss1', 'b1', 'ch1', 'hash', 'seg1', 0, 0, 10, 'text', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'ch1', 'extract', 'test', 'v1', 1, 'hash', 'completed', datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c1', 'b1', 1, 'location_edge', 'spatial', 'ss1', 'run1', 0.9, 'high', 'proposed', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES ('c2', 'b1', 1, 'location_edge', 'spatial', 'ss1', 'run1', 0.8, 'high', 'proposed', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('p1', 'b1', 'place', '青云城', '青云城', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('p2', 'b1', 'place', '落霞山', '落霞山', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('org1', 'b1', 'organization', '青云门', '青云门', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool).await.unwrap();
    }

    fn edge_record(
        id: &str,
        from_place_id: &str,
        to_place_id: &str,
        edge_type: &str,
        last_seen_chapter: i64,
    ) -> super::PlaceEdgeRecord {
        super::PlaceEdgeRecord {
            id: id.to_string(),
            book_id: "b1".to_string(),
            from_place_id: from_place_id.to_string(),
            to_place_id: to_place_id.to_string(),
            edge_type: edge_type.to_string(),
            direction_hint: None,
            distance_hint: None,
            confidence: 0.8,
            source_claim_id: "c1".to_string(),
            latest_source_claim_id: Some("c1".to_string()),
            first_seen_chapter: 1,
            last_seen_chapter,
            status: "active".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    #[tokio::test]
    async fn place_repo_requires_place_entity_for_detail() {
        let pool = setup_test_db().await;
        seed_base(&pool).await;
        let repo = PlaceRepo::new(pool);

        let place = repo
            .upsert_place_detail("b1", "p1", "city", None, 2, 0.7, true, 1, 3, "active")
            .await
            .unwrap();
        assert_eq!(place.entity_id, "p1");
        assert_eq!(place.last_seen_chapter, 3);

        let org_result = repo
            .upsert_place_detail(
                "b1",
                "org1",
                "sect_site",
                None,
                1,
                0.6,
                true,
                1,
                1,
                "active",
            )
            .await;
        assert!(
            org_result.is_err(),
            "organization must not get place_details"
        );
    }

    #[tokio::test]
    async fn undirected_edge_canonicalizes_pair() {
        let pool = setup_test_db().await;
        seed_base(&pool).await;
        let repo = PlaceRepo::new(pool);

        let edge = repo
            .find_or_create_edge("b1", "p2", "p1", "near", None, None, 0.8, "c1", 1)
            .await
            .unwrap();

        assert_eq!(edge.from_place_id, "p1");
        assert_eq!(edge.to_place_id, "p2");
    }

    #[tokio::test]
    async fn directed_edge_preserves_direction() {
        let pool = setup_test_db().await;
        seed_base(&pool).await;
        let repo = PlaceRepo::new(pool);

        let edge = repo
            .find_or_create_edge(
                "b1",
                "p2",
                "p1",
                "north_of",
                Some("north"),
                None,
                0.8,
                "c1",
                1,
            )
            .await
            .unwrap();

        assert_eq!(edge.from_place_id, "p2");
        assert_eq!(edge.to_place_id, "p1");
    }

    #[tokio::test]
    async fn duplicate_edge_idempotent() {
        let pool = setup_test_db().await;
        seed_base(&pool).await;
        let repo = PlaceRepo::new(pool);

        let first = repo
            .find_or_create_edge("b1", "p1", "p2", "near", None, None, 0.7, "c1", 1)
            .await
            .unwrap();
        let second = repo
            .find_or_create_edge("b1", "p2", "p1", "near", None, None, 0.9, "c2", 2)
            .await
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(second.latest_source_claim_id.as_deref(), Some("c2"));
        assert_eq!(second.last_seen_chapter, 2);
    }

    #[tokio::test]
    async fn duplicate_edge_preserves_source_history() {
        let pool = setup_test_db().await;
        seed_base(&pool).await;
        let repo = PlaceRepo::new(pool);

        let edge = repo
            .find_or_create_edge("b1", "p1", "p2", "near", None, None, 0.7, "c1", 1)
            .await
            .unwrap();
        repo.find_or_create_edge("b1", "p2", "p1", "near", None, None, 0.9, "c2", 2)
            .await
            .unwrap();

        let sources = repo.list_edge_sources(&edge.id).await.unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].source_claim_id, "c1");
        assert_eq!(sources[1].source_claim_id, "c2");
    }

    #[tokio::test]
    async fn conflict_table_records_conflict() {
        let pool = setup_test_db().await;
        seed_base(&pool).await;
        let repo = PlaceRepo::new(pool);
        let edge = repo
            .find_or_create_edge(
                "b1",
                "p1",
                "p2",
                "north_of",
                Some("north"),
                None,
                0.7,
                "c1",
                1,
            )
            .await
            .unwrap();

        let conflict = repo
            .insert_conflict(
                "b1",
                "c2",
                Some(&edge.id),
                "opposite_direction",
                "judge_conflict",
                Some("{\"decision\":\"conflict\"}"),
            )
            .await
            .unwrap();
        let conflicts = repo.list_conflicts("b1", Some("open")).await.unwrap();

        assert_eq!(conflict.conflict_type, "opposite_direction");
        assert_eq!(conflicts.len(), 1);
    }

    #[tokio::test]
    async fn layout_snapshot_helpers_return_latest_snapshot() {
        let pool = setup_test_db().await;
        seed_base(&pool).await;
        let repo = PlaceRepo::new(pool);

        repo.create_layout_snapshot("b1", 1, "phase5-simple", "{\"nodes\":[]}", "hash1")
            .await
            .unwrap();
        repo.create_layout_snapshot("b1", 3, "phase5-simple", "{\"nodes\":[1]}", "hash2")
            .await
            .unwrap();

        let latest = repo
            .latest_layout_snapshot("b1", "phase5-simple")
            .await
            .unwrap()
            .expect("latest layout snapshot");
        assert_eq!(latest.max_chapter, 3);
        assert_eq!(latest.source_edge_hash, "hash2");
    }

    #[test]
    fn layout_source_edge_hash_is_deterministic() {
        let first = vec![
            edge_record("e1", "p1", "p2", "near", 2),
            edge_record("e2", "p2", "p1", "north_of", 3),
        ];
        let second = vec![
            edge_record("e2", "p2", "p1", "north_of", 3),
            edge_record("e1", "p1", "p2", "near", 2),
        ];

        assert_eq!(
            PlaceRepo::source_edge_hash(&first),
            PlaceRepo::source_edge_hash(&second)
        );
    }

    #[tokio::test]
    async fn related_entity_link_canonicalizes_pair() {
        let pool = setup_test_db().await;
        seed_base(&pool).await;
        let repo = PlaceRepo::new(pool);

        let first = repo
            .find_or_create_entity_link("b1", "p2", "org1", "related_entity", "c1", 0.7)
            .await
            .unwrap();
        let second = repo
            .find_or_create_entity_link("b1", "org1", "p2", "related_entity", "c2", 0.8)
            .await
            .unwrap();
        let links = repo
            .list_entity_links_for_entity("b1", "org1")
            .await
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].entity_a_id, "org1");
        assert_eq!(links[0].entity_b_id, "p2");
    }

    #[tokio::test]
    async fn entity_links_active_pair_unique() {
        let pool = setup_test_db().await;
        seed_base(&pool).await;

        sqlx::query(
            "INSERT INTO entity_links (
                id, book_id, entity_a_id, entity_b_id, link_type, source_claim_id,
                confidence, status, created_at, updated_at
             )
             VALUES ('link1', 'b1', 'org1', 'p2', 'related_entity', 'c1', 0.7, 'active', datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        let duplicate = sqlx::query(
            "INSERT INTO entity_links (
                id, book_id, entity_a_id, entity_b_id, link_type, source_claim_id,
                confidence, status, created_at, updated_at
             )
             VALUES ('link2', 'b1', 'org1', 'p2', 'related_entity', 'c2', 0.8, 'active', datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await;

        assert!(duplicate.is_err());
    }
}
