use serde::{Deserialize, Serialize};

pub type AiBookCatchupTaskStats = crate::model::ai_book::AiBookCatchupStatsV3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AiBookCatchupTaskStatus {
    Idle,
    Running,
    Canceling,
    Canceled,
    Completed,
    Failed,
}

impl AiBookCatchupTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Canceling => "canceling",
            Self::Canceled => "canceled",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCatchupStartRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
    pub target_chapter_index: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCatchupStatusRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCatchupCancelRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookCatchupTaskView {
    #[serde(skip_serializing)]
    pub user_ns: String,
    pub book_url: String,
    pub status: String,
    #[serde(default)]
    pub current_stage: Option<String>,
    pub start_chapter_index: Option<i32>,
    pub target_chapter_index: Option<i32>,
    pub total_chapters: i32,
    pub completed_chapters: i32,
    pub current_chapter_index: Option<i32>,
    pub current_chapter_title: Option<String>,
    pub processed_chapter_index: Option<i32>,
    pub processed_chapter_title: Option<String>,
    pub error: Option<String>,
    pub updated_at: i64,
    #[serde(default)]
    pub stats: Option<AiBookCatchupTaskStats>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_book_v3_catchup_status_serializes_new_states() {
        assert_eq!(AiBookCatchupTaskStatus::Canceling.as_str(), "canceling");
        assert_eq!(AiBookCatchupTaskStatus::Canceled.as_str(), "canceled");

        let value = serde_json::to_value(AiBookCatchupTaskView {
            user_ns: "u".to_string(),
            book_url: "b".to_string(),
            status: AiBookCatchupTaskStatus::Canceling.as_str().to_string(),
            current_stage: Some("patch".to_string()),
            start_chapter_index: None,
            target_chapter_index: None,
            total_chapters: 0,
            completed_chapters: 0,
            current_chapter_index: None,
            current_chapter_title: None,
            processed_chapter_index: None,
            processed_chapter_title: None,
            error: None,
            updated_at: 0,
            stats: Some(AiBookCatchupTaskStats::default()),
        })
        .unwrap();

        assert_eq!(value["status"], "canceling");
        assert_eq!(value["currentStage"], "patch");
        assert!(value["stats"].is_object());
    }
}
