use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;

use crate::error::AppResult;

pub async fn init_pool(db_path: &str) -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::from_str(db_path)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

pub fn database_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "Could not determine the home directory".to_owned())?;

    #[cfg(target_os = "macos")]
    let directory = home.join("Library/Application Support/com.kankan.app");

    #[cfg(target_os = "windows")]
    let directory = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("AppData/Roaming"))
        .join("com.kankan.app");

    #[cfg(all(unix, not(target_os = "macos")))]
    let directory = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"))
        .join("com.kankan.app");

    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    Ok(directory.join("kankan.db"))
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Seed a demo workspace on first launch so the board is not empty.
pub async fn seed_if_empty(pool: &SqlitePool) -> AppResult<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }

    let ts = now();
    let workspace_id = uuid();
    let me_id = uuid();

    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO workspaces (id, name, key, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&workspace_id)
    .bind("Kankan")
    .bind("KAN")
    .bind(&ts)
    .bind(&ts)
    .execute(&mut *tx)
    .await?;

    // The single local user. Collaborators would simply be more rows here.
    let people: [(&str, &str, i64, i64); 4] = [
        (me_id.as_str(), "Karri", 268, 1),
        ("u-jori", "Jori", 150, 0),
        ("u-mira", "Mira", 60, 0),
        ("u-devon", "Devon", 200, 0),
    ];
    for (id, name, hue, is_me) in people {
        sqlx::query(
            "INSERT INTO users (id, workspace_id, name, email, avatar_hue, is_me, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&workspace_id)
        .bind(name)
        .bind(format!("{}@kankan.app", name.to_lowercase()))
        .bind(hue)
        .bind(is_me)
        .bind(&ts)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }

    let projects: [(&str, &str, &str, &str, i64, &str); 4] = [
        ("p-eng", "Engineering", "Core product engineering", "🧩", 268, "active"),
        ("p-desk", "Desktop App", "Native desktop client on Tauri", "🖥", 200, "active"),
        ("p-mkt", "Marketing Site", "Launch page and assets", "📣", 60, "planned"),
        ("p-growth", "Growth", "Experiments and retention", "📈", 150, "backlog"),
    ];
    for (i, (id, name, desc, icon, hue, status)) in projects.iter().enumerate() {
        sqlx::query(
            "INSERT INTO projects (id, workspace_id, name, description, icon, color_hue, status, lead_id, sort_order, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&workspace_id)
        .bind(name)
        .bind(desc)
        .bind(icon)
        .bind(hue)
        .bind(status)
        .bind(&me_id)
        .bind(i as f64)
        .bind(&ts)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }

    // Label colors sampled to echo the reference UI.
    let labels: [(&str, &str, &str); 6] = [
        ("l-bug", "Bug", "#e5484d"),
        ("l-design", "Design", "#5b8def"),
        ("l-ai", "AI", "#8a8f98"),
        ("l-perf", "Performance", "#30a46c"),
        ("l-ios", "iOS", "#b07cf7"),
        ("l-feat", "Feature", "#d9a441"),
    ];
    for (id, name, color) in labels {
        sqlx::query(
            "INSERT INTO labels (id, workspace_id, name, color, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&workspace_id)
        .bind(name)
        .bind(color)
        .bind(&ts)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }

    // (number, title, status, priority, project, assignee, estimate, labels, branch, description)
    let seed_issues: Vec<(i64, &str, &str, i64, &str, Option<&str>, Option<i64>, Vec<&str>, Option<&str>, &str)> = vec![
        (926, "Remove UI inconsistencies", "todo", 3, "p-eng", Some(&me_id), Some(3), vec!["l-bug", "l-design"], None,
         "Audit the settings surface and unify spacing, radii and control heights between `Settings > Preferences` and `Settings > Account`."),
        (1487, "Remove contentData from GraphQL API", "in_progress", 2, "p-eng", Some("u-jori"), Some(5), vec!["l-perf"], Some("61039"),
         "The `contentData` field is now resolved on the client. Drop it from the schema and delete the resolver to cut payload size."),
        (2088, "TypeError: Cannot read properties of undefined", "todo", 4, "p-eng", Some("u-mira"), Some(2), vec!["l-bug"], None,
         "Crash reported from the board view when a card has no labels. Guard the label render path and add a regression test."),
        (1028, "Launch page assets", "todo", 2, "p-mkt", Some("u-devon"), Some(3), vec!["l-design"], None,
         "Export hero art at 1x/2x and wire the responsive `srcset` for the launch page."),
        (924, "Upgrade to the latest desktop runtime", "todo", 1, "p-desk", Some(&me_id), None, vec!["l-ai"], None,
         "Bump the desktop shell runtime and verify the updater flow still signs releases correctly."),
        (2187, "Prevent duplicate ride requests on poor network", "in_progress", 4, "p-eng", Some("u-jori"), Some(8), vec!["l-bug"], Some("62048"),
         "Requests can be fired twice when the first response is slow. Add an idempotency key and debounce the submit handler."),
        (1882, "Optimize load times", "in_progress", 2, "p-eng", Some("u-mira"), Some(5), vec!["l-perf"], Some("61102"),
         "Cold start is 1.8s. Render the shell before the vehicle state sync completes instead of blocking on a full refresh."),
        (1310, "Offline mode for the desktop app", "backlog", 2, "p-desk", None, Some(8), vec!["l-feat"], None,
         "Queue mutations locally and replay them when connectivity returns."),
        (1198, "Sidebar keyboard navigation", "backlog", 1, "p-desk", Some(&me_id), Some(2), vec!["l-feat"], None,
         "Support arrow keys for moving through the sidebar and folding sections."),
        (640, "Onboarding checklist", "backlog", 0, "p-growth", None, Some(3), vec!["l-feat"], None,
         "First-run checklist that guides a new user through creating a project and their first issue."),
        (512, "Dark theme contrast pass", "done", 2, "p-eng", Some(&me_id), Some(3), vec!["l-design"], Some("60118"),
         "Raised muted text from 3.1:1 to 5.2:1 contrast against elevated surfaces."),
        (480, "Ship the command palette", "done", 3, "p-eng", Some("u-jori"), Some(5), vec!["l-feat"], Some("60011"),
         "⌘K now opens a fuzzy search across issues, projects and actions."),
        (299, "Deprecated sync endpoint", "canceled", 0, "p-eng", None, None, vec![], None,
         "Superseded by the new incremental sync design."),
    ];

    for (i, (number, title, status, priority, project, assignee, estimate, lbls, branch, desc)) in
        seed_issues.iter().enumerate()
    {
        let issue_id = uuid();
        sqlx::query(
            "INSERT INTO issues (id, workspace_id, number, title, description, status, priority, estimate,
                                 project_id, assignee_id, author_id, branch_ref, sort_order, completed_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&issue_id)
        .bind(&workspace_id)
        .bind(number)
        .bind(title)
        .bind(desc)
        .bind(status)
        .bind(priority)
        .bind(estimate)
        .bind(project)
        .bind(assignee)
        .bind(&me_id)
        .bind(branch)
        .bind(i as f64)
        .bind(if *status == "done" { Some(ts.clone()) } else { None })
        .bind(&ts)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;

        for label_id in lbls {
            sqlx::query("INSERT INTO issue_labels (issue_id, label_id) VALUES (?, ?)")
                .bind(&issue_id)
                .bind(label_id)
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query(
            "INSERT INTO activities (id, issue_id, actor_id, kind, body, meta, created_at) VALUES (?, ?, ?, 'created', NULL, NULL, ?)",
        )
        .bind(uuid())
        .bind(&issue_id)
        .bind(&me_id)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }

    // A couple of subtasks on the flagship issue plus a comment thread.
    let flagship: Option<String> =
        sqlx::query_scalar("SELECT id FROM issues WHERE number = 926 AND workspace_id = ?")
            .bind(&workspace_id)
            .fetch_optional(&mut *tx)
            .await?;

    if let Some(issue_id) = flagship {
        for (i, (title, done)) in [
            ("Audit control heights", 1),
            ("Unify spacing scale", 0),
            ("Update snapshots", 0),
        ]
        .iter()
        .enumerate()
        {
            sqlx::query(
                "INSERT INTO subtasks (id, issue_id, title, done, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(uuid())
            .bind(&issue_id)
            .bind(title)
            .bind(done)
            .bind(i as f64)
            .bind(&ts)
            .bind(&ts)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            "INSERT INTO activities (id, issue_id, actor_id, kind, body, meta, created_at) VALUES (?, ?, ?, 'comment', ?, NULL, ?)",
        )
        .bind(uuid())
        .bind(&issue_id)
        .bind("u-jori")
        .bind("Right now the two panes use different paddings, which makes the whole settings area feel inconsistent...")
        .bind(&ts)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO activities (id, issue_id, actor_id, kind, body, meta, created_at) VALUES (?, ?, ?, 'comment', ?, NULL, ?)",
        )
        .bind(uuid())
        .bind(&issue_id)
        .bind(&me_id)
        .bind("Good catch — I'll take a stab at this and post a draft PR for review.")
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
