use serde::{Deserialize, Serialize};

use super::ai_book::{
    AiBookChapterDigestV3, AiBookDroppedFactV3, AiBookFactV3, AiBookLocationV3,
    AiBookRelationshipV3,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookChapterDigestCandidateV3 {
    pub chapter_index: i32,
    pub chapter_title: String,
    pub summary: String,
    pub key_points: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct KnowledgePatchV3 {
    pub summary: Option<String>,
    pub characters: Vec<String>,
    pub relationships: Vec<AiBookRelationshipV3>,
    pub locations: Vec<AiBookLocationV3>,
    pub facts: Vec<AiBookFactV3>,
    pub dropped_facts: Vec<AiBookDroppedFactV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCombinedChapterGenerationV3 {
    pub chapter_digest: AiBookChapterDigestCandidateV3,
    pub knowledge_patch: KnowledgePatchV3,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookChapterDigestPatchV3 {
    pub digest: AiBookChapterDigestV3,
    pub patch: KnowledgePatchV3,
}
