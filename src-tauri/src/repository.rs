use sqlx::SqlitePool;

use crate::db::{now, uuid};
use crate::error::{AppError, AppResult};
use crate::models::{
    Activity, ActivityView, Issue, IssueDetail, IssueView, Label, NewIssue, NewLabel, NewUser,
    Project, Subtask, UpdateIssue, User, Workspace,
};

#[derive(Default, Clone)]
pub struct IssueFilter {
    pub project_id: Option<String>,
    pub assignee_id: Option<String>,
    pub search: String,
}

#[derive(Clone)]
pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn workspace(&self) -> AppResult<Workspace> {
        sqlx::query_as::<_, Workspace>("SELECT * FROM workspaces ORDER BY created_at LIMIT 1")
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("workspace".into()))
    }

    pub async fn users(&self) -> AppResult<Vec<User>> {
        Ok(sqlx::query_as::<_, User>(
            "SELECT * FROM users ORDER BY is_me DESC, name",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn current_user(&self) -> AppResult<User> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE is_me = 1 LIMIT 1")
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("current user".into()))
    }

    pub async fn create_user(&self, data: NewUser) -> AppResult<User> {
        let name = validate_name(&data.name, "Person")?;
        let email = normalize_email(data.email.as_deref())?;
        let workspace = self.workspace().await?;
        let current = self.current_user().await?;
        let id = uuid();
        let timestamp = now();
        let avatar_hue = (current.avatar_hue + 47).rem_euclid(360);
        sqlx::query(
            "INSERT INTO users (id, workspace_id, name, email, avatar_hue, is_me, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, 0, ?, ?)",
        )
        .bind(&id)
        .bind(&workspace.id)
        .bind(&name)
        .bind(&email)
        .bind(avatar_hue)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&self.pool)
        .await
        .map_err(|error| unique_error(error, "That name or email is already in use."))?;
        Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn update_user(&self, id: &str, data: NewUser) -> AppResult<User> {
        let name = validate_name(&data.name, "Person")?;
        let email = normalize_email(data.email.as_deref())?;
        let workspace = self.workspace().await?;
        let result = sqlx::query(
            "UPDATE users SET name = ?, email = ?, updated_at = ?
             WHERE id = ? AND workspace_id = ?",
        )
        .bind(&name)
        .bind(&email)
        .bind(now())
        .bind(id)
        .bind(&workspace.id)
        .execute(&self.pool)
        .await
        .map_err(|error| unique_error(error, "That name or email is already in use."))?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("person".into()));
        }
        Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn set_current_user(&self, id: &str) -> AppResult<User> {
        let workspace = self.workspace().await?;
        let mut transaction = self.pool.begin().await?;
        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ? AND workspace_id = ?")
                .bind(id)
                .bind(&workspace.id)
                .fetch_one(&mut *transaction)
                .await?;
        if exists == 0 {
            return Err(AppError::NotFound("person".into()));
        }
        sqlx::query("UPDATE users SET is_me = 0 WHERE workspace_id = ?")
            .bind(&workspace.id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE users SET is_me = 1, updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.current_user().await
    }

    pub async fn delete_user(&self, id: &str) -> AppResult<()> {
        let workspace = self.workspace().await?;
        let mut transaction = self.pool.begin().await?;
        let person = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE id = ? AND workspace_id = ?",
        )
        .bind(id)
        .bind(&workspace.id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("person".into()))?;
        if person.is_me == 1 {
            return Err(AppError::Other(
                "Switch to another profile before removing this one.".into(),
            ));
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE workspace_id = ?")
            .bind(&workspace.id)
            .fetch_one(&mut *transaction)
            .await?;
        if count <= 1 {
            return Err(AppError::Other(
                "At least one local profile must remain.".into(),
            ));
        }
        sqlx::query("DELETE FROM users WHERE id = ? AND workspace_id = ?")
            .bind(id)
            .bind(&workspace.id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn projects(&self) -> AppResult<Vec<Project>> {
        Ok(sqlx::query_as::<_, Project>(
            "SELECT * FROM projects ORDER BY sort_order, name",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn labels(&self) -> AppResult<Vec<Label>> {
        Ok(sqlx::query_as::<_, Label>("SELECT * FROM labels ORDER BY name")
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn create_label(&self, data: NewLabel) -> AppResult<Label> {
        let name = validate_name(&data.name, "Category")?;
        let color = validate_color(&data.color)?;
        let workspace = self.workspace().await?;
        let id = uuid();
        let timestamp = now();
        sqlx::query(
            "INSERT INTO labels (id, workspace_id, name, color, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&workspace.id)
        .bind(name)
        .bind(color)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&self.pool)
        .await
        .map_err(|error| unique_error(error, "A category with that name already exists."))?;
        Ok(sqlx::query_as::<_, Label>("SELECT * FROM labels WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn delete_label(&self, id: &str) -> AppResult<()> {
        let workspace = self.workspace().await?;
        let result = sqlx::query("DELETE FROM labels WHERE id = ? AND workspace_id = ?")
            .bind(id)
            .bind(workspace.id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("category".into()));
        }
        Ok(())
    }

    pub async fn issues(&self, filter: &IssueFilter) -> AppResult<Vec<IssueView>> {
        let workspace = self.workspace().await?;
        let mut query = String::from("SELECT i.* FROM issues i WHERE i.workspace_id = ?");
        if filter.project_id.is_some() {
            query.push_str(" AND i.project_id = ?");
        }
        if filter.assignee_id.is_some() {
            query.push_str(" AND i.assignee_id = ?");
        }
        if !filter.search.trim().is_empty() {
            query.push_str(" AND (i.title LIKE ? OR CAST(i.number AS TEXT) LIKE ?)");
        }
        query.push_str(" ORDER BY i.sort_order, i.number DESC");

        let mut statement = sqlx::query_as::<_, Issue>(&query).bind(&workspace.id);
        if let Some(project_id) = &filter.project_id {
            statement = statement.bind(project_id);
        }
        if let Some(assignee_id) = &filter.assignee_id {
            statement = statement.bind(assignee_id);
        }
        if !filter.search.trim().is_empty() {
            let pattern = format!("%{}%", filter.search.trim());
            statement = statement.bind(pattern.clone()).bind(pattern);
        }

        let issues = statement.fetch_all(&self.pool).await?;
        let mut views = Vec::with_capacity(issues.len());
        for issue in issues {
            views.push(self.build_view(issue, &workspace.key).await?);
        }
        Ok(views)
    }

    pub async fn issue(&self, id: &str) -> AppResult<IssueDetail> {
        let workspace = self.workspace().await?;
        let issue = sqlx::query_as::<_, Issue>("SELECT * FROM issues WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("issue".into()))?;

        let view = self.build_view(issue.clone(), &workspace.key).await?;
        let subtasks = sqlx::query_as::<_, Subtask>(
            "SELECT * FROM subtasks WHERE issue_id = ? ORDER BY sort_order",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let activities = sqlx::query_as::<_, Activity>(
            "SELECT * FROM activities WHERE issue_id = ? ORDER BY created_at",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;

        let mut activity_views = Vec::with_capacity(activities.len());
        for activity in activities {
            let actor = match &activity.actor_id {
                Some(actor_id) => {
                    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
                        .bind(actor_id)
                        .fetch_optional(&self.pool)
                        .await?
                }
                None => None,
            };
            activity_views.push(ActivityView { activity, actor });
        }

        let project = match &issue.project_id {
            Some(project_id) => {
                sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = ?")
                    .bind(project_id)
                    .fetch_optional(&self.pool)
                    .await?
            }
            None => None,
        };

        Ok(IssueDetail {
            view,
            subtasks,
            activities: activity_views,
            project,
        })
    }

    pub async fn create_issue(&self, data: NewIssue) -> AppResult<IssueView> {
        let workspace = self.workspace().await?;
        let me = self.current_user().await?;
        let title = validate_name(&data.title, "Issue title")?;
        let status = data.status.as_deref().unwrap_or("todo");
        let priority = data.priority.unwrap_or(0);
        validate_status(status)?;
        if !(0..=4).contains(&priority) {
            return Err(AppError::Other("Priority must be between 0 and 4.".into()));
        }
        let estimate = validate_estimate(data.estimate)?;
        let due_date = normalize_due_date(data.due_date)?;
        let mut label_ids = Vec::new();
        for label_id in data.label_ids.unwrap_or_default() {
            if !label_ids.contains(&label_id) {
                label_ids.push(label_id);
            }
        }
        self.validate_issue_references(
            &workspace.id,
            data.project_id.as_deref(),
            data.assignee_id.as_deref(),
            &label_ids,
        )
        .await?;
        let timestamp = now();
        let id = uuid();
        let number: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM issues WHERE workspace_id = ?",
        )
        .bind(&workspace.id)
        .fetch_one(&self.pool)
        .await?;

        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO issues (id, workspace_id, number, title, description, status, priority, estimate,
                                 project_id, assignee_id, author_id, branch_ref, due_date, sort_order, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&workspace.id)
        .bind(number)
        .bind(title)
        .bind(data.description.unwrap_or_default())
        .bind(status)
        .bind(priority)
        .bind(estimate)
        .bind(&data.project_id)
        .bind(&data.assignee_id)
        .bind(&me.id)
        .bind(&due_date)
        .bind(-(number as f64))
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&mut *transaction)
        .await?;

        for label_id in label_ids {
            sqlx::query("INSERT INTO issue_labels (issue_id, label_id) VALUES (?, ?)")
                .bind(&id)
                .bind(label_id)
                .execute(&mut *transaction)
                .await?;
        }
        Self::log_activity(&mut transaction, &id, Some(&me.id), "created", None, None).await?;
        transaction.commit().await?;

        let issue = sqlx::query_as::<_, Issue>("SELECT * FROM issues WHERE id = ?")
            .bind(&id)
            .fetch_one(&self.pool)
            .await?;
        self.build_view(issue, &workspace.key).await
    }

    pub async fn update_issue(&self, id: &str, data: UpdateIssue) -> AppResult<IssueDetail> {
        let me = self.current_user().await?;
        let existing = sqlx::query_as::<_, Issue>("SELECT * FROM issues WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("issue".into()))?;

        let title = validate_name(
            data.title.as_deref().unwrap_or(&existing.title),
            "Issue title",
        )?;
        let description = data
            .description
            .unwrap_or_else(|| existing.description.clone());
        let status = data.status.unwrap_or_else(|| existing.status.clone());
        let priority = data.priority.unwrap_or(existing.priority);
        let project_id = data.project_id.unwrap_or_else(|| existing.project_id.clone());
        let assignee_id = data
            .assignee_id
            .unwrap_or_else(|| existing.assignee_id.clone());
        let estimate = validate_estimate(data.estimate.unwrap_or(existing.estimate))?;
        let due_date = normalize_due_date(
            data.due_date
                .unwrap_or_else(|| existing.due_date.clone()),
        )?;
        validate_status(&status)?;
        if !(0..=4).contains(&priority) {
            return Err(AppError::Other("Priority must be between 0 and 4.".into()));
        }
        self.validate_issue_references(
            &existing.workspace_id,
            project_id.as_deref(),
            assignee_id.as_deref(),
            &[],
        )
        .await?;
        let completed_at = if status == "done" {
            existing.completed_at.clone().or_else(|| Some(now()))
        } else {
            None
        };

        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE issues SET title = ?, description = ?, status = ?, priority = ?, project_id = ?,
                               assignee_id = ?, estimate = ?, due_date = ?, completed_at = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&title)
        .bind(&description)
        .bind(&status)
        .bind(priority)
        .bind(&project_id)
        .bind(&assignee_id)
        .bind(estimate)
        .bind(&due_date)
        .bind(&completed_at)
        .bind(now())
        .bind(id)
        .execute(&mut *transaction)
        .await?;

        if existing.status != status {
            let meta = serde_json::json!({ "from": existing.status, "to": status }).to_string();
            Self::log_activity(
                &mut transaction,
                id,
                Some(&me.id),
                "status_changed",
                None,
                Some(&meta),
            )
            .await?;
        }
        if existing.assignee_id != assignee_id {
            Self::log_activity(&mut transaction, id, Some(&me.id), "assigned", None, None).await?;
        }
        if existing.priority != priority {
            let meta =
                serde_json::json!({ "from": existing.priority, "to": priority }).to_string();
            Self::log_activity(
                &mut transaction,
                id,
                Some(&me.id),
                "priority_changed",
                None,
                Some(&meta),
            )
            .await?;
        }
        if existing.title != title {
            Self::log_activity(&mut transaction, id, Some(&me.id), "renamed", None, None).await?;
        }
        transaction.commit().await?;
        self.issue(id).await
    }

    pub async fn move_issue(&self, id: &str, status: &str, sort_order: f64) -> AppResult<()> {
        validate_status(status)?;
        if !sort_order.is_finite() {
            return Err(AppError::Other("Issue order must be finite.".into()));
        }
        let existing: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT status, completed_at FROM issues WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        let (previous, prior_completed_at) =
            existing.ok_or_else(|| AppError::NotFound("issue".into()))?;
        let me = self.current_user().await?;
        let completed_at = if status == "done" {
            prior_completed_at.or_else(|| Some(now()))
        } else {
            None
        };
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE issues SET status = ?, sort_order = ?, completed_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(sort_order)
        .bind(completed_at)
        .bind(now())
        .bind(id)
        .execute(&mut *transaction)
        .await?;

        if previous != status {
            let meta = serde_json::json!({ "from": previous, "to": status }).to_string();
            Self::log_activity(
                &mut transaction,
                id,
                Some(&me.id),
                "status_changed",
                None,
                Some(&meta),
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn set_labels(&self, issue_id: &str, label_ids: &[String]) -> AppResult<()> {
        let issue = sqlx::query_as::<_, Issue>("SELECT * FROM issues WHERE id = ?")
            .bind(issue_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("issue".into()))?;
        self.validate_issue_references(
            &issue.workspace_id,
            issue.project_id.as_deref(),
            issue.assignee_id.as_deref(),
            label_ids,
        )
        .await?;
        let mut normalized_ids = Vec::new();
        for label_id in label_ids {
            if !normalized_ids.contains(label_id) {
                normalized_ids.push(label_id.clone());
            }
        }
        let previous: Vec<String> =
            sqlx::query_scalar("SELECT label_id FROM issue_labels WHERE issue_id = ?")
                .bind(issue_id)
                .fetch_all(&self.pool)
                .await?;
        let me = self.current_user().await?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query("DELETE FROM issue_labels WHERE issue_id = ?")
            .bind(issue_id)
            .execute(&mut *transaction)
            .await?;
        for label_id in &normalized_ids {
            sqlx::query("INSERT INTO issue_labels (issue_id, label_id) VALUES (?, ?)")
                .bind(issue_id)
                .bind(label_id)
                .execute(&mut *transaction)
                .await?;
        }
        for label_id in normalized_ids.iter().filter(|id| !previous.contains(id)) {
            let meta = serde_json::json!({ "label_id": label_id }).to_string();
            Self::log_activity(
                &mut transaction,
                issue_id,
                Some(&me.id),
                "label_added",
                None,
                Some(&meta),
            )
            .await?;
        }
        for label_id in previous.iter().filter(|id| !normalized_ids.contains(id)) {
            let meta = serde_json::json!({ "label_id": label_id }).to_string();
            Self::log_activity(
                &mut transaction,
                issue_id,
                Some(&me.id),
                "label_removed",
                None,
                Some(&meta),
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn delete_issue(&self, id: &str) -> AppResult<()> {
        let result = sqlx::query("DELETE FROM issues WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("issue".into()));
        }
        Ok(())
    }

    pub async fn add_comment(&self, issue_id: &str, body: &str) -> AppResult<ActivityView> {
        let body = body.trim();
        if body.is_empty() {
            return Err(AppError::Other("Comment cannot be empty.".into()));
        }
        if body.chars().count() > 10_000 {
            return Err(AppError::Other(
                "Comments must be 10,000 characters or fewer.".into(),
            ));
        }
        let me = self.current_user().await?;
        let id = uuid();
        let timestamp = now();
        sqlx::query(
            "INSERT INTO activities (id, issue_id, actor_id, kind, body, meta, created_at)
             VALUES (?, ?, ?, 'comment', ?, NULL, ?)",
        )
        .bind(&id)
        .bind(issue_id)
        .bind(&me.id)
        .bind(body)
        .bind(&timestamp)
        .execute(&self.pool)
        .await?;
        let activity = sqlx::query_as::<_, Activity>("SELECT * FROM activities WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(ActivityView {
            activity,
            actor: Some(me),
        })
    }

    pub async fn add_subtask(&self, issue_id: &str, title: &str) -> AppResult<Subtask> {
        let title = validate_name(title, "Subtask")?;
        let id = uuid();
        let timestamp = now();
        let sort_order: f64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sort_order), 0.0) + 1.0 FROM subtasks WHERE issue_id = ?",
        )
        .bind(issue_id)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO subtasks (id, issue_id, title, done, sort_order, created_at, updated_at)
             VALUES (?, ?, ?, 0, ?, ?, ?)",
        )
        .bind(&id)
        .bind(issue_id)
        .bind(title)
        .bind(sort_order)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&self.pool)
        .await?;
        Ok(sqlx::query_as::<_, Subtask>("SELECT * FROM subtasks WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn toggle_subtask(&self, id: &str) -> AppResult<()> {
        let result =
            sqlx::query("UPDATE subtasks SET done = 1 - done, updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("subtask".into()));
        }
        Ok(())
    }

    async fn validate_issue_references(
        &self,
        workspace_id: &str,
        project_id: Option<&str>,
        assignee_id: Option<&str>,
        label_ids: &[String],
    ) -> AppResult<()> {
        if let Some(project_id) = project_id {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM projects WHERE id = ? AND workspace_id = ?",
            )
            .bind(project_id)
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
            if exists == 0 {
                return Err(AppError::Other(
                    "The selected project is not in this workspace.".into(),
                ));
            }
        }
        if let Some(assignee_id) = assignee_id {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM users WHERE id = ? AND workspace_id = ?",
            )
            .bind(assignee_id)
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
            if exists == 0 {
                return Err(AppError::Other(
                    "The selected person is not in this workspace.".into(),
                ));
            }
        }
        for label_id in label_ids {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM labels WHERE id = ? AND workspace_id = ?",
            )
            .bind(label_id)
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
            if exists == 0 {
                return Err(AppError::Other(
                    "A selected category is not in this workspace.".into(),
                ));
            }
        }
        Ok(())
    }

    async fn build_view(&self, issue: Issue, workspace_key: &str) -> AppResult<IssueView> {
        let labels = sqlx::query_as::<_, Label>(
            "SELECT l.* FROM labels l
             JOIN issue_labels il ON il.label_id = l.id
             WHERE il.issue_id = ? ORDER BY l.name",
        )
        .bind(&issue.id)
        .fetch_all(&self.pool)
        .await?;
        let assignee = match &issue.assignee_id {
            Some(id) => sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
            None => None,
        };
        let (subtask_total, subtask_done): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(done), 0) FROM subtasks WHERE issue_id = ?",
        )
        .bind(&issue.id)
        .fetch_one(&self.pool)
        .await?;
        Ok(IssueView {
            identifier: format!("{workspace_key}-{}", issue.number),
            issue,
            labels,
            assignee,
            subtask_total,
            subtask_done,
        })
    }

    async fn log_activity(
        transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        issue_id: &str,
        actor_id: Option<&str>,
        kind: &str,
        body: Option<&str>,
        meta: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO activities (id, issue_id, actor_id, kind, body, meta, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid())
        .bind(issue_id)
        .bind(actor_id)
        .bind(kind)
        .bind(body)
        .bind(meta)
        .bind(now())
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

}

fn validate_name(value: &str, kind: &str) -> AppResult<String> {
    let name = value.trim();
    if name.is_empty() {
        return Err(AppError::Other(format!("{kind} name cannot be empty.")));
    }
    if name.chars().count() > 100 {
        return Err(AppError::Other(format!(
            "{kind} names must be 100 characters or fewer."
        )));
    }
    Ok(name.to_owned())
}

fn normalize_email(email: Option<&str>) -> AppResult<Option<String>> {
    let Some(email) = email.map(str::trim).filter(|email| !email.is_empty()) else {
        return Ok(None);
    };
    let valid = email.len() <= 254
        && !email.chars().any(char::is_whitespace)
        && email
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'));
    if !valid {
        return Err(AppError::Other(
            "Enter a valid email address or leave the field blank.".into(),
        ));
    }
    Ok(Some(email.to_owned()))
}

fn validate_color(color: &str) -> AppResult<String> {
    let color = color.trim();
    if color.len() != 7
        || !color.starts_with('#')
        || !color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(AppError::Other(
            "Category colors must use a six-digit hex value.".into(),
        ));
    }
    Ok(color.to_owned())
}

fn unique_error(error: sqlx::Error, message: &str) -> AppError {
    if error
        .as_database_error()
        .is_some_and(|database_error| database_error.is_unique_violation())
    {
        AppError::Other(message.to_owned())
    } else {
        AppError::Sqlx(error)
    }
}

fn validate_status(status: &str) -> AppResult<()> {
    if matches!(
        status,
        "backlog" | "todo" | "in_progress" | "done" | "canceled"
    ) {
        Ok(())
    } else {
        Err(AppError::Other("Unknown issue status.".into()))
    }
}

fn validate_estimate(estimate: Option<i64>) -> AppResult<Option<i64>> {
    if estimate.is_none_or(|value| [1, 2, 3, 5, 8, 13].contains(&value)) {
        Ok(estimate)
    } else {
        Err(AppError::Other(
            "Estimate must be a supported point value.".into(),
        ))
    }
}

fn normalize_due_date(due_date: Option<String>) -> AppResult<Option<String>> {
    let Some(due_date) = due_date.map(|value| value.trim().to_owned()) else {
        return Ok(None);
    };
    if due_date.is_empty() {
        return Ok(None);
    }
    if chrono::NaiveDate::parse_from_str(&due_date, "%Y-%m-%d").is_err() {
        return Err(AppError::Other(
            "Due date must use YYYY-MM-DD and be a real calendar date.".into(),
        ));
    }
    Ok(Some(due_date))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tokio::runtime::Builder;

    use super::*;

    #[test]
    fn sqlite_repository_persists_issue_workflows() {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let options = SqliteConnectOptions::from_str("sqlite::memory:")
                .unwrap()
                .foreign_keys(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .unwrap();
            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .unwrap();
            crate::db::seed_if_empty(&pool).await.unwrap();

            let repository = Repository::new(pool);
            let issue = repository
                .create_issue(NewIssue {
                    title: "Native app test issue".into(),
                    description: Some("Created by the repository test".into()),
                    status: Some("todo".into()),
                    priority: Some(2),
                    project_id: Some("p-eng".into()),
                    assignee_id: None,
                    label_ids: Some(vec!["l-bug".into()]),
                    estimate: Some(3),
                    due_date: None,
                })
                .await
                .unwrap();

            repository
                .move_issue(&issue.issue.id, "in_progress", 1.0)
                .await
                .unwrap();
            repository
                .update_issue(
                    &issue.issue.id,
                    UpdateIssue {
                        title: Some("Updated native issue".into()),
                        status: Some("done".into()),
                        assignee_id: Some(Some("u-jori".into())),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            repository
                .add_comment(&issue.issue.id, "Looks good")
                .await
                .unwrap();
            let subtask = repository
                .add_subtask(&issue.issue.id, "Verify persistence")
                .await
                .unwrap();
            repository.toggle_subtask(&subtask.id).await.unwrap();

            let detail = repository.issue(&issue.issue.id).await.unwrap();
            assert_eq!(detail.view.issue.title, "Updated native issue");
            assert_eq!(detail.view.issue.status, "done");
            assert_eq!(detail.view.subtask_total, 1);
            assert_eq!(detail.view.subtask_done, 1);
            assert!(detail
                .activities
                .iter()
                .any(|activity| activity.activity.kind == "comment"));

            repository.delete_issue(&issue.issue.id).await.unwrap();
            assert!(repository.issue(&issue.issue.id).await.is_err());
        });
    }

    #[test]
    fn workspace_management_enforces_profile_and_category_rules() {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let options = SqliteConnectOptions::from_str("sqlite::memory:")
                .unwrap()
                .foreign_keys(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .unwrap();
            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .unwrap();
            crate::db::seed_if_empty(&pool).await.unwrap();
            let repository = Repository::new(pool);

            let original_user = repository.current_user().await.unwrap();
            let person = repository
                .create_user(NewUser {
                    name: "  Riley Park  ".into(),
                    email: Some("riley@example.com".into()),
                })
                .await
                .unwrap();
            assert_eq!(person.name, "Riley Park");
            assert_eq!(person.email.as_deref(), Some("riley@example.com"));

            assert!(repository
                .create_user(NewUser {
                    name: "rIlEy pArK".into(),
                    email: None,
                })
                .await
                .is_err());
            assert!(repository.delete_user(&original_user.id).await.is_err());

            repository.set_current_user(&person.id).await.unwrap();
            repository.delete_user(&person.id).await.unwrap_err();
            repository.delete_user(&original_user.id).await.unwrap();
            assert_eq!(repository.current_user().await.unwrap().id, person.id);
            let remaining_references: i64 = sqlx::query_scalar(
                "SELECT
                    (SELECT COUNT(*) FROM issues
                     WHERE assignee_id = ? OR author_id = ?)
                    +
                    (SELECT COUNT(*) FROM activities WHERE actor_id = ?)",
            )
            .bind(&original_user.id)
            .bind(&original_user.id)
            .bind(&original_user.id)
            .fetch_one(&repository.pool)
            .await
            .unwrap();
            assert_eq!(remaining_references, 0);

            let category = repository
                .create_label(NewLabel {
                    name: "Release blocker".into(),
                    color: "#e5484d".into(),
                })
                .await
                .unwrap();
            assert!(repository
                .create_label(NewLabel {
                    name: " release BLOCKER ".into(),
                    color: "#e5484d".into(),
                })
                .await
                .is_err());

            let issue = repository
                .create_issue(NewIssue {
                    title: "Category cascade test".into(),
                    description: None,
                    status: Some("todo".into()),
                    priority: Some(1),
                    project_id: None,
                    assignee_id: None,
                    label_ids: Some(vec![category.id.clone()]),
                    estimate: None,
                    due_date: None,
                })
                .await
                .unwrap();
            assert_eq!(issue.labels.len(), 1);
            repository.delete_label(&category.id).await.unwrap();
            assert!(repository
                .issue(&issue.issue.id)
                .await
                .unwrap()
                .view
                .labels
                .is_empty());
        });
    }

    #[test]
    fn issue_inputs_validate_dates_and_workspace_references() {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let options = SqliteConnectOptions::from_str("sqlite::memory:")
                .unwrap()
                .foreign_keys(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .unwrap();
            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .unwrap();
            crate::db::seed_if_empty(&pool).await.unwrap();
            let repository = Repository::new(pool);

            for invalid_date in ["2026-02-30", "10/09/2026", "not-a-date"] {
                assert!(repository
                    .create_issue(NewIssue {
                        title: "Invalid date".into(),
                        description: None,
                        status: None,
                        priority: None,
                        project_id: None,
                        assignee_id: None,
                        label_ids: None,
                        estimate: None,
                        due_date: Some(invalid_date.into()),
                    })
                    .await
                    .is_err());
            }
            assert!(repository
                .create_issue(NewIssue {
                    title: "Unsupported estimate".into(),
                    description: None,
                    status: None,
                    priority: None,
                    project_id: None,
                    assignee_id: None,
                    label_ids: None,
                    estimate: Some(4),
                    due_date: None,
                })
                .await
                .is_err());
            assert!(repository
                .create_issue(NewIssue {
                    title: "Unknown category".into(),
                    description: None,
                    status: None,
                    priority: None,
                    project_id: None,
                    assignee_id: None,
                    label_ids: Some(vec!["not-in-this-workspace".into()]),
                    estimate: None,
                    due_date: None,
                })
                .await
                .is_err());

            let issue = repository
                .create_issue(NewIssue {
                    title: "Valid due date".into(),
                    description: None,
                    status: None,
                    priority: None,
                    project_id: None,
                    assignee_id: None,
                    label_ids: None,
                    estimate: Some(5),
                    due_date: Some(" 2026-10-31 ".into()),
                })
                .await
                .unwrap();
            assert_eq!(issue.issue.due_date.as_deref(), Some("2026-10-31"));

            repository
                .update_issue(
                    &issue.issue.id,
                    UpdateIssue {
                        due_date: Some(Some("".into())),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            assert_eq!(
                repository
                    .issue(&issue.issue.id)
                    .await
                    .unwrap()
                    .view
                    .issue
                    .due_date,
                None
            );
        });
    }
}
