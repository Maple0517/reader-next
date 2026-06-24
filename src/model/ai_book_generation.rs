use serde::{Deserialize, Serialize};

use super::ai_book::{
    AiBookChapterDigestV3, AiBookCharacterRelationV3, AiBookCharacterStateV3, AiBookLocationEdgeV3,
    AiBookLocationV3,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookChapterDigestCandidateV3 {
    pub chapter_index: i32,
    pub chapter_title: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub has_important_changes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookKnowledgePatchV3 {
    pub chapter_index: i32,
    pub summary: Option<String>,
    pub characters: Vec<AiBookCharacterPatchV3>,
    pub character_states: Vec<AiBookCharacterStatePatchV3>,
    pub character_relations: Vec<AiBookCharacterRelationPatchV3>,
    pub knowledge_facts: Vec<AiBookKnowledgeFactPatchV3>,
    pub locations: Vec<AiBookLocationPatchV3>,
    pub location_edges: Vec<AiBookLocationEdgePatchV3>,
}

pub type KnowledgePatchV3 = AiBookKnowledgePatchV3;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCombinedChapterGenerationV3 {
    pub chapter_digest: AiBookChapterDigestCandidateV3,
    pub patch: AiBookKnowledgePatchV3,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCharacterPatchV3 {
    pub name: String,
    pub aliases: Vec<String>,
    pub status: Option<String>,
    pub faction: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub last_seen_chapter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCharacterStatePatchV3 {
    pub name: String,
    pub status: String,
    pub description: Option<String>,
    pub last_seen_chapter_index: Option<i32>,
    pub last_seen_chapter_title: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCharacterRelationPatchV3 {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub polarity: String,
    pub strength: String,
    pub status: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookKnowledgeFactPatchV3 {
    pub title: String,
    pub content: String,
    pub category: String,
    pub confidence: String,
    pub importance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookLocationPatchV3 {
    pub name: String,
    pub kind: Option<String>,
    pub description: String,
    pub status: Option<String>,
    pub related_characters: Vec<String>,
    pub first_seen_chapter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookLocationEdgePatchV3 {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookChapterDigestPatchV3 {
    pub digest: AiBookChapterDigestV3,
    pub patch: AiBookKnowledgePatchV3,
}

#[allow(dead_code)]
fn _type_check() {
    let _ = (
        std::any::type_name::<AiBookCharacterRelationV3>(),
        std::any::type_name::<AiBookCharacterStateV3>(),
        std::any::type_name::<AiBookLocationV3>(),
        std::any::type_name::<AiBookLocationEdgeV3>(),
    );
}
