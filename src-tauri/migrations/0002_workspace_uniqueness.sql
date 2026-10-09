CREATE UNIQUE INDEX idx_users_workspace_name
    ON users(workspace_id, lower(trim(name)));

CREATE UNIQUE INDEX idx_users_workspace_email
    ON users(workspace_id, lower(trim(email)))
    WHERE email IS NOT NULL AND trim(email) <> '';

CREATE UNIQUE INDEX idx_users_one_current_profile
    ON users(workspace_id)
    WHERE is_me = 1;

CREATE UNIQUE INDEX idx_labels_workspace_name
    ON labels(workspace_id, lower(trim(name)));
