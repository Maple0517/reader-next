use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::property_repo::PropertyRepo;
use sqlx::SqlitePool;

/// Builds context for AI extraction from book state.
///
/// Assembles:
/// - Current segment spans (from source_spans)
/// - Previous chapter summary (from chapter_summaries)
/// - Candidate entities (from entity_aliases + entities)
/// - Active characters (from entity_current_properties)
/// - Schema brief (hardcoded prompt instructions)
pub struct ContextBuilder {
    pool: SqlitePool,
}

impl ContextBuilder {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Build context for a given book/chapter/segment.
    pub async fn build_context(
        &self,
        book_id: &str,
        chapter_index: i64,
        segment_text: &str,
    ) -> anyhow::Result<String> {
        let mut parts: Vec<String> = Vec::new();

        // 1. Schema brief — extraction instructions
        parts.push(SCHEMA_BRIEF.to_string());

        // 2. Segment text
        parts.push(format!("## Current Segment Text\n{}", segment_text));

        // 3. Previous chapter summary
        if chapter_index > 0 {
            let prev_index = chapter_index - 1;
            if let Some(summary) = self.get_chapter_summary(book_id, prev_index).await? {
                parts.push(format!("## Previous Chapter Summary\n{}", summary));
            }
        }

        // 4. Candidate entities with aliases
        let entities_context = self.get_entities_context(book_id).await?;
        if !entities_context.is_empty() {
            parts.push(format!("## Known Entities\n{}", entities_context));
        }

        // 5. Active character states
        let states_context = self.get_character_states(book_id).await?;
        if !states_context.is_empty() {
            parts.push(format!("## Active Character States\n{}", states_context));
        }

        Ok(parts.join("\n\n"))
    }

    async fn get_chapter_summary(
        &self,
        book_id: &str,
        chapter_index: i64,
    ) -> anyhow::Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT summary FROM chapter_summaries WHERE book_id = ? AND chapter_index = ?",
        )
        .bind(book_id)
        .bind(chapter_index)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0))
    }

    async fn get_entities_context(&self, book_id: &str) -> anyhow::Result<String> {
        let entity_repo = EntityRepo::new(self.pool.clone());
        let entities = entity_repo.list_by_book(book_id).await?;

        if entities.is_empty() {
            return Ok(String::new());
        }

        let mut lines = Vec::new();
        for entity in &entities {
            let aliases = entity_repo.list_aliases_by_entity(&entity.id).await?;
            let alias_str = if aliases.is_empty() {
                String::new()
            } else {
                let alias_names: Vec<&str> = aliases.iter().map(|a| a.alias.as_str()).collect();
                format!(" (aliases: {})", alias_names.join(", "))
            };
            let summary = entity.short_summary.as_deref().unwrap_or("");
            lines.push(format!(
                "- [{}] {}{} — {}",
                entity.entity_type, entity.canonical_name, alias_str, summary
            ));
        }

        Ok(lines.join("\n"))
    }

    async fn get_character_states(&self, book_id: &str) -> anyhow::Result<String> {
        let rows: Vec<(String, String, String, String, i64)> = sqlx::query_as(
            "SELECT e.canonical_name, pd.display_name, COALESCE(ecp.value_text, ''), pd.dimension_key, ecp.updated_chapter
             FROM entity_current_properties ecp
             JOIN entities e ON e.id = ecp.entity_id
             JOIN property_dimensions pd ON pd.dimension_key = ecp.dimension_key AND pd.book_id = '__global__' AND pd.entity_type = 'character'
             WHERE ecp.book_id = ?
             ORDER BY e.canonical_name, pd.importance DESC"
        )
        .bind(book_id)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(String::new());
        }

        let mut lines = Vec::new();
        let mut current_entity = String::new();
        for (name, dim_label, value, _dim_key, updated_ch) in &rows {
            if *name != current_entity {
                current_entity = name.clone();
                lines.push(format!("**{}**:", name));
            }
            lines.push(format!("  - {}: {} (ch.{})", dim_label, value, updated_ch));
        }

        Ok(lines.join("\n"))
    }
}

const SCHEMA_BRIEF: &str = r#"## Extraction Schema

You are extracting structured observations from a novel chapter segment. Output observations as one of these types:

1. **EntityIntroduction** — A new character/entity appears for the first time
   - subject_mention, entity_type, aliases, short_summary, evidence_span_ids, confidence

2. **Alias** — A known entity is referred to by a new name/title
   - subject_mention, alias, alias_type, evidence_span_ids, confidence

3. **PropertyUpdate** — A character's attribute changes (realm, affiliation, life_status, etc.)
   - subject_mention, dimension_key, value_text, evidence_span_ids, confidence
   - dimension_key must be one of: identity, life_status, affiliation, rank, occupation, realm, ability, equipment, location, mental_state, goal, injury, appearance, background

4. **MinorEvent** — A noteworthy event that doesn't change entity state
   - description, involved_mentions, evidence_span_ids, confidence

5. **Summary** — Chapter summary (only one per chapter)
   - summary, key_points, has_important_changes

Rules:
- subject_mention: the exact text from the source that refers to the entity
- evidence_span_ids: IDs of source spans supporting this observation
- confidence: 0.0-1.0
- Do NOT invent entities that are not clearly introduced in the text
- Do NOT make death/resurrection claims unless explicitly stated"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup() -> (SqlitePool, ContextBuilder) {
        let dir = std::env::temp_dir().join(format!("reader-v4-context-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        let builder = ContextBuilder::new(pool.clone());
        (pool, builder)
    }

    #[tokio::test]
    async fn build_context_contains_schema_brief() {
        let (_pool, builder) = setup().await;
        let ctx = builder
            .build_context("book1", 1, "张三走进了大殿")
            .await
            .unwrap();
        assert!(
            ctx.contains("Extraction Schema"),
            "should contain schema brief"
        );
        assert!(
            ctx.contains("EntityIntroduction"),
            "should mention observation types"
        );
        assert!(
            ctx.contains("dimension_key"),
            "should mention dimension_key"
        );
    }

    #[tokio::test]
    async fn build_context_contains_segment_text() {
        let (_pool, builder) = setup().await;
        let ctx = builder
            .build_context("book1", 1, "张三走进了大殿，看到了李四。")
            .await
            .unwrap();
        assert!(
            ctx.contains("张三走进了大殿，看到了李四。"),
            "should contain segment text"
        );
        assert!(
            ctx.contains("Current Segment Text"),
            "should have section header"
        );
    }

    #[tokio::test]
    async fn build_context_contains_previous_summary() {
        let (pool, builder) = setup().await;

        // Insert a chapter summary for chapter 0
        sqlx::query(
            "INSERT INTO chapter_summaries (id, book_id, chapter_index, summary, updated_at)
             VALUES ('s1', 'book1', 0, '上一章张三初次登场', datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        let ctx = builder.build_context("book1", 1, "本章内容").await.unwrap();
        assert!(
            ctx.contains("上一章张三初次登场"),
            "should contain previous chapter summary"
        );
        assert!(
            ctx.contains("Previous Chapter Summary"),
            "should have section header"
        );
    }

    #[tokio::test]
    async fn build_context_contains_entities() {
        let (pool, builder) = setup().await;
        let entity_repo = EntityRepo::new(pool.clone());

        let entity = entity_repo
            .create_entity("book1", "character", "张三", "张三", Some("主角"), 0.9, 1)
            .await
            .unwrap();
        entity_repo
            .create_alias("book1", &entity.id, "小三", "nickname", 1, 0.8, None)
            .await
            .unwrap();

        let ctx = builder.build_context("book1", 1, "text").await.unwrap();
        assert!(
            ctx.contains("Known Entities"),
            "should have entities section"
        );
        assert!(ctx.contains("张三"), "should contain entity name");
        assert!(ctx.contains("小三"), "should contain alias");
        assert!(ctx.contains("character"), "should contain entity type");
    }

    #[tokio::test]
    async fn build_context_contains_character_states() {
        let (pool, builder) = setup().await;
        let entity_repo = EntityRepo::new(pool.clone());

        let entity = entity_repo
            .create_entity("book1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        // Create a claim first (required for FK)
        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'book1', 1, 'text', 'hash', datetime('now'))")
            .bind(&chapter_id).execute(&pool).await.unwrap();
        let segment_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'book1', ?, 'hash', 0, datetime('now'))")
            .bind(&segment_id).bind(&chapter_id).execute(&pool).await.unwrap();
        let span_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'book1', ?, 'hash', ?, 0, 0, 10, 'test', datetime('now'))")
            .bind(&span_id).bind(&chapter_id).bind(&segment_id).execute(&pool).await.unwrap();
        let run_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, 'book1', ?, 'extract', 'test', 'v1', 1, 'hash', 'success', datetime('now'))")
            .bind(&run_id).bind(&chapter_id).execute(&pool).await.unwrap();
        let claim_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'book1', 1, 'property_update', 'test', ?, ?, 0.9, 'low', 'accepted', datetime('now'), datetime('now'))")
            .bind(&claim_id).bind(&span_id).bind(&run_id).execute(&pool).await.unwrap();

        let prop_repo = PropertyRepo::new(pool.clone());
        prop_repo
            .apply_replace(
                "book1",
                &entity.id,
                "realm",
                Some("筑基期"),
                None,
                1,
                &claim_id,
                0.9,
            )
            .await
            .unwrap();

        let ctx = builder.build_context("book1", 1, "text").await.unwrap();
        assert!(
            ctx.contains("Active Character States"),
            "should have states section"
        );
        assert!(ctx.contains("张三"), "should contain character name");
        assert!(ctx.contains("筑基期"), "should contain realm value");
        assert!(
            ctx.contains("Realm/Cultivation"),
            "should contain dimension display name"
        );
    }

    #[tokio::test]
    async fn build_context_no_previous_summary_for_first_chapter() {
        let (_pool, builder) = setup().await;
        let ctx = builder
            .build_context("book1", 0, "第一章内容")
            .await
            .unwrap();
        assert!(
            !ctx.contains("Previous Chapter Summary"),
            "chapter 0 has no previous chapter"
        );
    }

    #[tokio::test]
    async fn build_context_empty_book_has_no_entities() {
        let (_pool, builder) = setup().await;
        let ctx = builder
            .build_context("empty_book", 1, "text")
            .await
            .unwrap();
        assert!(
            !ctx.contains("Known Entities"),
            "empty book should have no entities section"
        );
        assert!(
            !ctx.contains("Active Character States"),
            "empty book should have no states section"
        );
    }
}
