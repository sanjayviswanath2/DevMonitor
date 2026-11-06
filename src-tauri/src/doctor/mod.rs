pub mod deps;
pub mod disk;
pub mod docker;
pub mod envfile;
pub mod git;
pub mod heap;
pub mod node;
pub mod project;
pub mod python;
pub mod scan;
pub mod toolchain;
pub mod watchers;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueAction {
    pub id: String,
    pub label: String,
    pub confirm: bool,
    pub confirm_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorIssue {
    pub id: String,
    pub kind: String,
    pub severity: IssueSeverity,
    pub title: String,
    pub summary: String,
    pub details: Vec<String>,
    pub action: Option<IssueAction>,
    pub meta: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub issues: Vec<DoctorIssue>,
    pub project: Option<project::ProjectInfo>,
    pub node: node::NodeEnvironment,
    pub scanned_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixResult {
    pub success: bool,
    pub message: String,
}
