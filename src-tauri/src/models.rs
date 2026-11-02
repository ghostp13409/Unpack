use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Todo,
    InProgress,
    Done,
    Dropped,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::InProgress => "in_progress",
            Status::Done => "done",
            Status::Dropped => "dropped",
        }
    }
}

impl FromStr for Status {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "todo" => Ok(Status::Todo),
            "in_progress" => Ok(Status::InProgress),
            "done" => Ok(Status::Done),
            "dropped" => Ok(Status::Dropped),
            other => Err(format!("unknown status {other:?}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Urgent,
    High,
    #[default]
    Medium,
    Low,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Urgent => "urgent",
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }
}

impl FromStr for Priority {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "urgent" => Ok(Priority::Urgent),
            "high" => Ok(Priority::High),
            "medium" => Ok(Priority::Medium),
            "low" => Ok(Priority::Low),
            other => Err(format!("unknown priority {other:?}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: Status,
    pub priority: Priority,
    pub category: Option<String>,
    pub due_date: Option<String>,
    pub parent_id: Option<String>,
    pub pinned: bool,
    pub snoozed_until: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTask {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// A partial update. `None` leaves a field untouched; for nullable fields,
/// `Some(None)` clears the value and `Some(Some(v))` sets it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaskPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "serde_with::rust::double_option")]
    pub category: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "serde_with::rust::double_option")]
    pub due_date: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "serde_with::rust::double_option")]
    pub parent_id: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "serde_with::rust::double_option")]
    pub snoozed_until: Option<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub task_id: String,
    pub body: String,
    pub source_capture_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureStatus {
    Proposed,
    Applied,
    Discarded,
    Failed,
}

impl CaptureStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CaptureStatus::Proposed => "proposed",
            CaptureStatus::Applied => "applied",
            CaptureStatus::Discarded => "discarded",
            CaptureStatus::Failed => "failed",
        }
    }
}

impl FromStr for CaptureStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "proposed" => Ok(CaptureStatus::Proposed),
            "applied" => Ok(CaptureStatus::Applied),
            "discarded" => Ok(CaptureStatus::Discarded),
            "failed" => Ok(CaptureStatus::Failed),
            other => Err(format!("unknown capture status {other:?}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Paste,
    File,
}

impl InputKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            InputKind::Paste => "paste",
            InputKind::File => "file",
        }
    }
}

impl FromStr for InputKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "paste" => Ok(InputKind::Paste),
            "file" => Ok(InputKind::File),
            other => Err(format!("unknown input kind {other:?}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Create,
    Update,
    Complete,
    AddNote,
}

impl ChangeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeKind::Create => "create",
            ChangeKind::Update => "update",
            ChangeKind::Complete => "complete",
            ChangeKind::AddNote => "add_note",
        }
    }
}

impl FromStr for ChangeKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "create" => Ok(ChangeKind::Create),
            "update" => Ok(ChangeKind::Update),
            "complete" => Ok(ChangeKind::Complete),
            "add_note" => Ok(ChangeKind::AddNote),
            other => Err(format!("unknown change kind {other:?}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capture {
    pub id: String,
    pub created_at: String,
    pub summary: Option<String>,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub status: CaptureStatus,
    pub proposal: Option<crate::ai::ops::Proposal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureInput {
    pub id: String,
    pub capture_id: String,
    pub kind: InputKind,
    pub filename: Option<String>,
    pub extracted_text: String,
    pub char_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub id: String,
    pub capture_id: String,
    pub task_id: Option<String>,
    pub kind: ChangeKind,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
    pub reason: String,
    pub evidence: Option<String>,
    pub applied: bool,
    pub undone: bool,
    pub created_at: String,
}

pub fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_roundtrip_snake_case() {
        assert_eq!(serde_json::to_string(&Status::InProgress).unwrap(), "\"in_progress\"");
        assert_eq!(Status::from_str("in_progress").unwrap(), Status::InProgress);
        assert_eq!(Priority::from_str("urgent").unwrap().as_str(), "urgent");
        assert!(Priority::from_str("critical").is_err());
    }
}
