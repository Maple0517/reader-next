use crate::storage::db::v4::knowledge_repo::{KnowledgeCardRecord, KnowledgeRepo};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicResolutionStatus {
    Resolved,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicCandidateMatch {
    Exact,
    Slug,
    Alias,
    RecentSameCategory,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ProposedTopic {
    pub topic_key: String,
    pub topic_display: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TopicCandidate {
    pub book_id: String,
    pub category: String,
    pub card_id: String,
    pub topic_key: String,
    pub topic_display: String,
    pub current_summary: Option<String>,
    pub match_kind: TopicCandidateMatch,
    pub score: f64,
    pub last_updated_chapter: i64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TopicResolution {
    pub proposed: ProposedTopic,
    pub candidates: Vec<TopicCandidate>,
    pub status: TopicResolutionStatus,
    pub reason: Option<String>,
}

pub struct TopicResolver {
    knowledge_repo: KnowledgeRepo,
}

impl TopicResolver {
    pub fn new(knowledge_repo: KnowledgeRepo) -> Self {
        Self { knowledge_repo }
    }

    pub async fn resolve(
        &self,
        book_id: &str,
        category: &str,
        topic: &str,
        chapter_index: i64,
    ) -> anyhow::Result<TopicResolution> {
        let proposed = ProposedTopic {
            topic_key: slugify_topic(topic),
            topic_display: normalize_display_topic(topic),
        };

        let cards = self
            .knowledge_repo
            .list_cards(book_id, Some(category), Some("active"))
            .await?;
        let mut candidates = collect_candidates(
            book_id,
            category,
            &proposed.topic_key,
            &proposed.topic_display,
            &cards,
        );
        include_recent_same_category(&mut candidates, book_id, category, &cards, chapter_index);
        sort_candidates(&mut candidates);

        let (status, reason) = if proposed.topic_key.is_empty() {
            (
                TopicResolutionStatus::Uncertain,
                Some("no stable proposed topic metadata".to_string()),
            )
        } else if is_broad_topic_key(&proposed.topic_key) && candidates.len() > 1 {
            (
                TopicResolutionStatus::Uncertain,
                Some("topic is too broad for deterministic resolution".to_string()),
            )
        } else {
            (TopicResolutionStatus::Resolved, None)
        };

        Ok(TopicResolution {
            proposed,
            candidates,
            status,
            reason,
        })
    }
}

fn collect_candidates(
    book_id: &str,
    category: &str,
    proposed_key: &str,
    proposed_display: &str,
    cards: &[KnowledgeCardRecord],
) -> Vec<TopicCandidate> {
    let mut candidates = HashMap::new();
    for card in cards {
        if card.book_id != book_id || card.category != category || card.status != "active" {
            continue;
        }

        if normalize_exact_topic(&card.topic_display) == normalize_exact_topic(proposed_display) {
            upsert_candidate(&mut candidates, card, TopicCandidateMatch::Exact, 1.0);
        }

        if card.topic_key == proposed_key {
            upsert_candidate(&mut candidates, card, TopicCandidateMatch::Slug, 0.95);
        }

        if aliases_match(category, proposed_key, card) {
            upsert_candidate(&mut candidates, card, TopicCandidateMatch::Alias, 0.9);
        }
    }

    candidates.into_values().collect()
}

fn include_recent_same_category(
    candidates: &mut Vec<TopicCandidate>,
    book_id: &str,
    category: &str,
    cards: &[KnowledgeCardRecord],
    chapter_index: i64,
) {
    let mut recent = cards
        .iter()
        .filter(|card| {
            card.book_id == book_id && card.category == category && card.status == "active"
        })
        .filter(|card| card.last_updated_chapter <= chapter_index)
        .collect::<Vec<_>>();
    recent.sort_by(|a, b| {
        b.last_updated_chapter
            .cmp(&a.last_updated_chapter)
            .then_with(|| b.importance_score.total_cmp(&a.importance_score))
            .then_with(|| a.topic_display.cmp(&b.topic_display))
    });

    for card in recent.into_iter().take(5) {
        if candidates
            .iter()
            .any(|candidate| candidate.card_id == card.id)
        {
            continue;
        }
        candidates.push(build_candidate(
            card,
            TopicCandidateMatch::RecentSameCategory,
            0.2,
        ));
    }
}

fn upsert_candidate(
    candidates: &mut HashMap<String, TopicCandidate>,
    card: &KnowledgeCardRecord,
    match_kind: TopicCandidateMatch,
    score: f64,
) {
    let candidate = build_candidate(card, match_kind, score);
    candidates
        .entry(card.id.clone())
        .and_modify(|existing| {
            if candidate.score > existing.score {
                *existing = candidate.clone();
            }
        })
        .or_insert(candidate);
}

fn build_candidate(
    card: &KnowledgeCardRecord,
    match_kind: TopicCandidateMatch,
    score: f64,
) -> TopicCandidate {
    TopicCandidate {
        book_id: card.book_id.clone(),
        category: card.category.clone(),
        card_id: card.id.clone(),
        topic_key: card.topic_key.clone(),
        topic_display: card.topic_display.clone(),
        current_summary: card.current_summary.clone(),
        match_kind,
        score,
        last_updated_chapter: card.last_updated_chapter,
    }
}

fn sort_candidates(candidates: &mut [TopicCandidate]) {
    candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| b.last_updated_chapter.cmp(&a.last_updated_chapter))
            .then_with(|| {
                a.topic_display
                    .partial_cmp(&b.topic_display)
                    .unwrap_or(Ordering::Equal)
            })
    });
}

fn aliases_match(category: &str, proposed_key: &str, card: &KnowledgeCardRecord) -> bool {
    alias_groups_for_category(category).iter().any(|group| {
        group.contains(&proposed_key)
            && (group.contains(&card.topic_key.as_str())
                || group.contains(&slugify_topic(&card.topic_display).as_str()))
    })
}

fn alias_groups_for_category(category: &str) -> &'static [&'static [&'static str]] {
    match category {
        "power_system" => &[&[
            "cultivation-system",
            "cultivation-realms",
            "realm-system",
            "realm-systems",
            "power-system",
            "修炼体系",
            "境界体系",
            "境界系统",
            "力量体系",
        ]],
        "world_rule" => &[&[
            "world-rules",
            "world-rule",
            "rule-system",
            "rules-system",
            "law-system",
            "世界规则",
            "运行规则",
            "法则体系",
        ]],
        _ => &[],
    }
}

fn normalize_display_topic(topic: &str) -> String {
    topic.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_exact_topic(topic: &str) -> String {
    normalize_full_width(&normalize_display_topic(topic))
        .chars()
        .flat_map(char::to_lowercase)
        .collect()
}

fn slugify_topic(topic: &str) -> String {
    let mut normalized = String::new();
    let mut last_dash = false;
    for ch in normalize_full_width(topic)
        .chars()
        .flat_map(char::to_lowercase)
    {
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

fn normalize_full_width(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| match ch {
            '\u{FF01}'..='\u{FF5E}' => (ch as u32 - 0xFF01 + 0x21) as u8 as char,
            '\u{3000}' => ' ',
            _ => ch,
        })
        .collect()
}

fn is_broad_topic_key(topic_key: &str) -> bool {
    matches!(
        topic_key,
        "system"
            | "systems"
            | "rule"
            | "rules"
            | "history"
            | "secret"
            | "knowledge"
            | "world"
            | "体系"
            | "系统"
            | "规则"
            | "历史"
            | "秘密"
    )
}

#[cfg(test)]
mod tests {
    use super::{TopicCandidateMatch, TopicResolutionStatus, TopicResolver};
    use crate::storage::db;
    use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir =
            std::env::temp_dir().join(format!("reader-topic-resolver-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    #[tokio::test]
    async fn returns_existing_card_candidate_for_synonym_topic_in_same_category() {
        let pool = setup_test_db().await;
        let repo = KnowledgeRepo::new(pool);
        let existing = repo
            .find_or_create_card(
                "book1",
                "power_system",
                "cultivation-realms",
                "Cultivation Realms",
                Some("Realm ladder"),
                0.8,
                0.9,
                3,
            )
            .await
            .unwrap();
        repo.find_or_create_card(
            "book1",
            "history",
            "cultivation-realms",
            "Cultivation Realms",
            None,
            0.8,
            0.9,
            3,
        )
        .await
        .unwrap();

        let resolver = TopicResolver::new(repo);
        let resolved = resolver
            .resolve("book1", "power_system", "realm system", 5)
            .await
            .unwrap();

        assert_eq!(resolved.proposed.topic_key, "realm-system");
        assert!(matches!(resolved.status, TopicResolutionStatus::Resolved));
        assert!(resolved
            .candidates
            .iter()
            .any(|candidate| candidate.card_id == existing.id
                && candidate.match_kind == TopicCandidateMatch::Alias));
        assert!(resolved.candidates.iter().all(|candidate| {
            candidate.book_id == "book1" && candidate.category == "power_system"
        }));
    }

    #[tokio::test]
    async fn returns_stable_proposed_key_for_new_topic_without_using_assertion_text() {
        let pool = setup_test_db().await;
        let repo = KnowledgeRepo::new(pool);

        let resolver = TopicResolver::new(repo);
        let resolved = resolver
            .resolve("book1", "world_rule", "  Mana  Conservation Rules!!! ", 2)
            .await
            .unwrap();

        assert_eq!(resolved.proposed.topic_key, "mana-conservation-rules");
        assert_eq!(
            resolved.proposed.topic_display,
            "Mana Conservation Rules!!!"
        );
        assert!(resolved.candidates.is_empty());
        assert!(matches!(resolved.status, TopicResolutionStatus::Resolved));
    }

    #[tokio::test]
    async fn ambiguous_broad_topic_becomes_uncertain() {
        let pool = setup_test_db().await;
        let repo = KnowledgeRepo::new(pool);
        repo.find_or_create_card(
            "book1",
            "power_system",
            "cultivation-realms",
            "Cultivation Realms",
            None,
            0.8,
            0.9,
            3,
        )
        .await
        .unwrap();
        repo.find_or_create_card(
            "book1",
            "power_system",
            "mana-rules",
            "Mana Rules",
            None,
            0.8,
            0.8,
            4,
        )
        .await
        .unwrap();

        let resolver = TopicResolver::new(repo);
        let resolved = resolver
            .resolve("book1", "power_system", "system", 5)
            .await
            .unwrap();

        assert!(matches!(resolved.status, TopicResolutionStatus::Uncertain));
        assert_eq!(resolved.proposed.topic_key, "system");
        assert!(resolved.reason.unwrap().contains("too broad"));
        assert_eq!(resolved.candidates.len(), 2);
    }

    #[tokio::test]
    async fn normalizes_trim_lower_and_slug_punctuation() {
        let pool = setup_test_db().await;
        let repo = KnowledgeRepo::new(pool);
        let existing = repo
            .find_or_create_card(
                "book1",
                "world_rule",
                "mana-conservation-rules",
                "Mana Conservation Rules",
                None,
                0.8,
                0.7,
                1,
            )
            .await
            .unwrap();

        let resolver = TopicResolver::new(repo);
        let resolved = resolver
            .resolve("book1", "world_rule", "  MANA_conservation--rules ", 2)
            .await
            .unwrap();

        assert_eq!(resolved.proposed.topic_key, "mana-conservation-rules");
        assert_eq!(resolved.candidates.len(), 1);
        assert_eq!(resolved.candidates[0].card_id, existing.id);
        assert_eq!(resolved.candidates[0].match_kind, TopicCandidateMatch::Slug);
    }
}
