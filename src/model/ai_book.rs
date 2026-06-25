use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookMemory {
    pub book_url: String,
    pub book_name: Option<String>,
    pub author: Option<String>,
    pub enabled: bool,
    pub processed_chapter_index: Option<i32>,
    pub processed_chapter_title: Option<String>,
    pub updated_at: i64,
    pub summary: String,
    pub worldview: Vec<AiBookNote>,
    pub characters: Vec<AiBookCharacter>,
    pub relationships: Vec<AiBookRelationship>,
    pub locations: Vec<AiBookLocation>,
    pub map: Option<AiBookMap>,
    pub map_dirty: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookNote {
    pub title: String,
    pub content: String,
    pub confidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCharacter {
    pub name: String,
    pub aliases: Vec<String>,
    pub status: String,
    pub faction: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub last_seen_chapter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookRelationship {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub status: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookLocation {
    pub name: String,
    pub kind: Option<String>,
    pub description: String,
    pub status: Option<String>,
    pub related_characters: Vec<String>,
    pub first_seen_chapter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookMap {
    pub image_url: Option<String>,
    pub prompt: Option<String>,
    pub updated_at: Option<i64>,
    pub source_chapter_index: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookMemoryV3 {
    #[serde(rename = "schemaVersion")]
    pub schema_version: i32,
    pub book_url: String,
    pub book_name: Option<String>,
    pub author: Option<String>,
    pub enabled: bool,
    pub processed_chapter_index: Option<i32>,
    pub processed_chapter_title: Option<String>,
    pub updated_at: i64,
    pub summary: AiBookSummaryV3,
    pub chapter_digests: Vec<AiBookChapterDigestV3>,
    pub characters: Vec<AiBookCharacterV3>,
    pub character_states: Vec<AiBookCharacterStateV3>,
    pub character_relations: Vec<AiBookCharacterRelationV3>,
    pub knowledge_facts: Vec<AiBookKnowledgeFactV3>,
    pub locations: Vec<AiBookLocationV3>,
    pub location_edges: Vec<AiBookLocationEdgeV3>,
    pub map_state: Option<AiBookMapStateV3>,
    pub render_artifacts: Option<AiBookRenderArtifactsV3>,
    pub dropped_facts: Vec<AiBookDroppedFactV3>,
    pub catchup_stats: Option<AiBookCatchupStatsV3>,
    pub last_error: Option<String>,
    pub last_error_chapter_index: Option<i32>,
    pub last_error_chapter_title: Option<String>,
}

impl Default for AiBookMemoryV3 {
    fn default() -> Self {
        Self {
            schema_version: 3,
            book_url: String::new(),
            book_name: None,
            author: None,
            enabled: false,
            processed_chapter_index: None,
            processed_chapter_title: None,
            updated_at: 0,
            summary: AiBookSummaryV3::default(),
            chapter_digests: Vec::new(),
            characters: Vec::new(),
            character_states: Vec::new(),
            character_relations: Vec::new(),
            knowledge_facts: Vec::new(),
            locations: Vec::new(),
            location_edges: Vec::new(),
            map_state: None,
            render_artifacts: None,
            dropped_facts: Vec::new(),
            catchup_stats: None,
            last_error: None,
            last_error_chapter_index: None,
            last_error_chapter_title: None,
        }
    }
}

impl AiBookMemoryV3 {
    pub fn new(book_url: impl Into<String>) -> Self {
        Self {
            schema_version: 3,
            book_url: book_url.into(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookSummaryV3 {
    pub current: String,
    pub recent_changes: Vec<String>,
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookDroppedFactV3 {
    pub source: String,
    pub reason: String,
    pub original_value_preview: String,
    pub original_value_hash: String,
    pub redirected_to: Option<String>,
    pub chapter_index: Option<i32>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookChapterDigestV3 {
    pub chapter_index: i32,
    pub chapter_title: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub characters: Vec<AiBookCharacterV3>,
    pub character_states: Vec<AiBookCharacterStateV3>,
    pub character_relations: Vec<AiBookCharacterRelationV3>,
    pub knowledge_facts: Vec<AiBookKnowledgeFactV3>,
    pub locations: Vec<AiBookLocationV3>,
    pub location_edges: Vec<AiBookLocationEdgeV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCharacterV3 {
    pub name: String,
    pub aliases: Vec<String>,
    pub status: String,
    pub faction: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub last_seen_chapter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCharacterStateV3 {
    pub name: String,
    pub status: String,
    pub description: Option<String>,
    pub last_seen_chapter_index: Option<i32>,
    pub last_seen_chapter_title: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCharacterRelationV3 {
    pub source: String,
    pub target: String,
    pub kind: AiBookRelationKind,
    pub polarity: AiBookRelationPolarity,
    pub strength: AiBookRelationStrength,
    pub status: AiBookRelationStatus,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AiBookRelationKind {
    #[default]
    Unknown,
    Family,
    Romance,
    Friendship,
    Rivalry,
    Alliance,
    Conflict,
    Affiliation,
    Supervision,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AiBookRelationPolarity {
    #[default]
    Neutral,
    Positive,
    Negative,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AiBookRelationStrength {
    #[default]
    Unknown,
    Weak,
    Moderate,
    Strong,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AiBookRelationStatus {
    #[default]
    Unknown,
    Active,
    Distant,
    Broken,
    Developing,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookKnowledgeFactV3 {
    pub title: String,
    pub content: String,
    pub category: AiBookFactCategory,
    pub confidence: AiBookFactConfidence,
    pub importance: AiBookFactImportance,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookLocationV3 {
    pub name: String,
    pub kind: Option<String>,
    pub description: String,
    pub status: Option<String>,
    pub related_characters: Vec<String>,
    pub first_seen_chapter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookLocationEdgeV3 {
    pub source: String,
    pub target: String,
    pub kind: AiBookLocationEdgeKind,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AiBookLocationEdgeKind {
    #[default]
    Unknown,
    Contains,
    Adjacent,
    LeadsTo,
    PartOf,
    Near,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookMapStateV3 {
    pub dirty: bool,
    pub nodes: Vec<AiBookMapStateNodeV3>,
    pub edges: Vec<AiBookMapStateEdgeV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookMapStateNodeV3 {
    pub id: String,
    pub label: String,
    pub kind: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookMapStateEdgeV3 {
    pub source: String,
    pub target: String,
    pub kind: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookRenderArtifactsV3 {
    pub chapter_index: Option<i32>,
    pub chapter_title: Option<String>,
    pub summary: Option<String>,
    pub image_url: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCatchupStatsV3 {
    pub total_model_calls: i32,
    pub digest_calls: i32,
    pub patch_calls: i32,
    pub skipped_patch_chapters: i32,
    pub total_input_bytes: i64,
    pub total_output_bytes: i64,
    pub last_call_latency_ms: Option<i64>,
    pub average_call_latency_ms: Option<i64>,
    pub last_chapter_index: Option<i32>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookMemoryViewModel {
    pub book_url: String,
    pub book_name: Option<String>,
    pub author: Option<String>,
    pub enabled: bool,
    pub processed_chapter_index: Option<i32>,
    pub processed_chapter_title: Option<String>,
    pub updated_at: i64,
    pub summary: AiBookSummaryV3,
    pub characters: Vec<AiBookCharacterView>,
    pub relationships: Vec<AiBookRelationView>,
    pub knowledge_facts: Vec<AiBookKnowledgeFactView>,
    pub locations: Vec<AiBookLocationView>,
    pub map: Option<AiBookMapView>,
    pub cleanup: AiBookDroppedFactsSummary,
    pub catchup_stats: Option<AiBookCatchupStatsV3>,
    pub last_error: Option<String>,
    pub last_error_chapter_index: Option<i32>,
    pub last_error_chapter_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookChapterMemoryViewModel {
    pub book_url: String,
    pub chapter_index: i32,
    pub chapter_title: Option<String>,
    pub digest: Option<AiBookChapterDigestV3>,
    pub characters: Vec<AiBookCharacterView>,
    pub relationships: Vec<AiBookRelationView>,
    pub knowledge_facts: Vec<AiBookKnowledgeFactView>,
    pub locations: Vec<AiBookLocationView>,
    pub generation_status: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCharacterView {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub importance: String,
    pub description: Option<String>,
    pub first_seen_chapter_index: Option<i32>,
    pub last_seen_chapter_index: Option<i32>,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCharacterStateView {
    pub character_id: String,
    pub current_status: Option<String>,
    pub current_location_id: Option<String>,
    pub affiliations: Vec<String>,
    pub abilities: Vec<AiBookCharacterAbilityView>,
    pub resources: Vec<String>,
    pub last_seen_chapter_index: Option<i32>,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCharacterAbilityView {
    pub name: String,
    pub level: Option<String>,
    pub status: Option<String>,
    pub knowledge_fact_id: Option<String>,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookRelationView {
    pub id: String,
    pub source_character_id: String,
    pub target_character_id: String,
    pub kind: AiBookRelationKind,
    pub subtype: Option<String>,
    pub label: String,
    pub polarity: AiBookRelationPolarity,
    pub strength: AiBookRelationStrength,
    pub status: AiBookRelationStatus,
    pub direction: String,
    pub summary: String,
    pub current_dynamics: Vec<String>,
    pub facets: Vec<AiBookRelationFacetView>,
    pub first_seen_chapter_index: Option<i32>,
    pub last_updated_chapter_index: Option<i32>,
    pub evidence: Vec<AiBookEvidenceV3>,
    pub history: Vec<AiBookRelationChangeView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookRelationFacetView {
    pub kind: AiBookRelationKind,
    pub subtype: Option<String>,
    pub polarity: AiBookRelationPolarity,
    pub status: AiBookRelationStatus,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookRelationChangeView {
    pub chapter_index: i32,
    pub chapter_title: String,
    pub previous_kind: Option<AiBookRelationKind>,
    pub next_kind: AiBookRelationKind,
    pub previous_polarity: Option<AiBookRelationPolarity>,
    pub next_polarity: AiBookRelationPolarity,
    pub previous_status: Option<AiBookRelationStatus>,
    pub next_status: AiBookRelationStatus,
    pub note: String,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookKnowledgeFactView {
    pub id: String,
    pub category: AiBookFactCategory,
    pub title: String,
    pub content: String,
    pub confidence: AiBookFactConfidence,
    pub importance: AiBookFactImportance,
    pub first_seen_chapter_index: Option<i32>,
    pub last_confirmed_chapter_index: Option<i32>,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookLocationView {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub kind: String,
    pub scale: String,
    pub parent_location_id: Option<String>,
    pub description: String,
    pub current_status: Option<String>,
    pub importance: String,
    pub first_seen_chapter_index: Option<i32>,
    pub last_seen_chapter_index: Option<i32>,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookMapView {
    pub state: Option<AiBookMapStateV3>,
    pub render_artifacts: Option<AiBookRenderArtifactsV3>,
    pub locations: Vec<AiBookLocationView>,
    pub location_edges: Vec<AiBookLocationEdgeV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookDroppedFactsSummary {
    pub dropped_facts_count: i32,
    pub dropped_by_reason: std::collections::BTreeMap<String, i32>,
    pub old_schema_backed_up: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookMemoryViewResponse {
    pub memory: AiBookMemoryViewModel,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookChapterMemoryViewResponse {
    pub chapter: AiBookChapterMemoryViewModel,
    pub memory: AiBookMemoryViewModel,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookEvidenceV3 {
    pub chapter_index: i32,
    pub chapter_title: String,
    pub quote: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AiBookFactCategory {
    #[default]
    Unknown,
    BasicRule,
    PowerFaction,
    HistoryLegend,
    TechMagic,
    SocialCulture,
    Geography,
    Organization,
    Unconfirmed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AiBookFactConfidence {
    #[default]
    Unknown,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AiBookFactImportance {
    #[default]
    Unknown,
    Low,
    Medium,
    High,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn ai_book_v3_empty_memory_is_valid() {
        let memory = AiBookMemoryV3::new("book://test");
        let value = serde_json::to_value(&memory).unwrap();

        assert_eq!(value["schemaVersion"], json!(3));
        assert_eq!(value["bookUrl"], json!("book://test"));
        assert!(value["summary"].is_object());
        assert_eq!(value["chapterDigests"], Value::Array(vec![]));
        assert_eq!(value["characterStates"], Value::Array(vec![]));
        assert_eq!(value["characterRelations"], Value::Array(vec![]));
        assert_eq!(value["knowledgeFacts"], Value::Array(vec![]));
        assert_eq!(value["locationEdges"], Value::Array(vec![]));
        assert_eq!(value["droppedFacts"], Value::Array(vec![]));
        assert!(value["catchupStats"].is_null());
        assert!(value["mapState"].is_null());
        assert!(value["renderArtifacts"].is_null());
        assert!(value["lastError"].is_null());
    }
}
