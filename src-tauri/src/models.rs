use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub key: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub email: Option<String>,
    pub avatar_hue: i64,
    pub is_me: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewUser {
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color_hue: i64,
    pub status: String,
    pub target_date: Option<String>,
    pub lead_id: Option<String>,
    pub sort_order: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub color: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewLabel {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Subtask {
    pub id: String,
    pub issue_id: String,
    pub title: String,
    pub done: i64,
    pub sort_order: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: String,
    pub issue_id: String,
    pub actor_id: Option<String>,
    pub kind: String,
    pub body: Option<String>,
    pub meta: Option<String>,
    pub created_at: String,
}

/// Row shape straight from the issues table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub workspace_id: String,
    pub number: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: i64,
    pub estimate: Option<i64>,
    pub project_id: Option<String>,
    pub assignee_id: Option<String>,
    pub author_id: Option<String>,
    pub branch_ref: Option<String>,
    pub due_date: Option<String>,
    pub sort_order: f64,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Issue enriched with its labels + assignee for the board / list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueView {
    #[serde(flatten)]
    pub issue: Issue,
    pub identifier: String, // e.g. "KAN-123"
    pub labels: Vec<Label>,
    pub assignee: Option<User>,
    pub subtask_total: i64,
    pub subtask_done: i64,
}

/// Full detail bundle for the issue detail panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueDetail {
    #[serde(flatten)]
    pub view: IssueView,
    pub subtasks: Vec<Subtask>,
    pub activities: Vec<ActivityView>,
    pub project: Option<Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityView {
    #[serde(flatten)]
    pub activity: Activity,
    pub actor: Option<User>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewIssue {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i64>,
    pub project_id: Option<String>,
    pub assignee_id: Option<String>,
    pub label_ids: Option<Vec<String>>,
    pub estimate: Option<i64>,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIssue {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i64>,
    pub project_id: Option<Option<String>>,
    pub assignee_id: Option<Option<String>>,
    pub estimate: Option<Option<i64>>,
    pub due_date: Option<Option<String>>,
}
