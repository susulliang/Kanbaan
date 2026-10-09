-- Kankan initial schema
-- Designed single-user for the prototype, but collaboration-ready:
--   * every row is scoped by workspace_id
--   * users table + author/assignee foreign keys already exist
--   * created_at / updated_at everywhere for future sync/CRDT layers
-- All ids are text UUIDs so records can be created offline without collisions.

PRAGMA foreign_keys = ON;

CREATE TABLE workspaces (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    key         TEXT NOT NULL,            -- short prefix used for issue ids, e.g. "KAN"
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE users (
    id          TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    email       TEXT,
    avatar_hue  INTEGER NOT NULL DEFAULT 250, -- OKLCH hue for the generated avatar
    is_me       INTEGER NOT NULL DEFAULT 0,    -- the single local user for the prototype
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT,
    icon        TEXT,                     -- emoji / glyph
    color_hue   INTEGER NOT NULL DEFAULT 250,
    status      TEXT NOT NULL DEFAULT 'active', -- backlog | planned | active | completed | canceled
    target_date TEXT,
    lead_id     TEXT REFERENCES users(id) ON DELETE SET NULL,
    sort_order  REAL NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE labels (
    id          TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    color       TEXT NOT NULL,            -- hex or oklch string used for the dot
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE issues (
    id           TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    number       INTEGER NOT NULL,        -- per-workspace sequential number -> KAN-123
    title        TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    status       TEXT NOT NULL DEFAULT 'todo', -- backlog | todo | in_progress | done | canceled
    priority     INTEGER NOT NULL DEFAULT 0,   -- 0 none, 1 low, 2 medium, 3 high, 4 urgent
    estimate     INTEGER,                 -- story points
    project_id   TEXT REFERENCES projects(id) ON DELETE SET NULL,
    assignee_id  TEXT REFERENCES users(id) ON DELETE SET NULL,
    author_id    TEXT REFERENCES users(id) ON DELETE SET NULL,
    branch_ref   TEXT,                    -- e.g. "61039" shown as a git ref on the card
    due_date     TEXT,
    sort_order   REAL NOT NULL DEFAULT 0, -- ordering within a status column
    completed_at TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    UNIQUE (workspace_id, number)
);

CREATE TABLE issue_labels (
    issue_id  TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    label_id  TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    PRIMARY KEY (issue_id, label_id)
);

CREATE TABLE subtasks (
    id          TEXT PRIMARY KEY,
    issue_id    TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    done        INTEGER NOT NULL DEFAULT 0,
    sort_order  REAL NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Unified activity feed: system events + user comments (Linear-style).
CREATE TABLE activities (
    id          TEXT PRIMARY KEY,
    issue_id    TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    actor_id    TEXT REFERENCES users(id) ON DELETE SET NULL,
    kind        TEXT NOT NULL,            -- created | status_changed | assigned | label_added | label_removed | priority_changed | comment | renamed
    body        TEXT,                     -- comment text
    meta        TEXT,                     -- json blob for structured events (from/to values)
    created_at  TEXT NOT NULL
);

CREATE INDEX idx_issues_workspace_status ON issues(workspace_id, status);
CREATE INDEX idx_issues_project ON issues(project_id);
CREATE INDEX idx_issues_assignee ON issues(assignee_id);
CREATE INDEX idx_activities_issue ON activities(issue_id, created_at);
CREATE INDEX idx_issue_labels_label ON issue_labels(label_id);
CREATE INDEX idx_subtasks_issue ON subtasks(issue_id);
