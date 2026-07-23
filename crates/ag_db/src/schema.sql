-- Raw + meta + derived stubs for AandG Analytics (schema version 2).

CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY NOT NULL,
    key TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    raw_json TEXT
);

CREATE TABLE IF NOT EXISTS issues (
    id TEXT PRIMARY KEY NOT NULL,
    key TEXT NOT NULL UNIQUE,
    project_key TEXT NOT NULL,
    summary TEXT,
    issue_type TEXT,
    status TEXT,
    status_category TEXT,
    assignee_account_id TEXT,
    reporter_account_id TEXT,
    story_points REAL,
    parent_key TEXT,
    epic_key TEXT,
    created TEXT NOT NULL,
    updated TEXT NOT NULL,
    resolved TEXT,
    raw_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_issues_project_key ON issues(project_key);
CREATE INDEX IF NOT EXISTS idx_issues_updated ON issues(updated);

CREATE TABLE IF NOT EXISTS issue_changelog (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id TEXT NOT NULL,
    changelog_id TEXT,
    field TEXT NOT NULL,
    from_value TEXT,
    to_value TEXT,
    from_string TEXT,
    to_string TEXT,
    author_account_id TEXT,
    created TEXT NOT NULL,
    FOREIGN KEY (issue_id) REFERENCES issues(id)
);

CREATE INDEX IF NOT EXISTS idx_issue_changelog_issue_id ON issue_changelog(issue_id);

CREATE TABLE IF NOT EXISTS sprints (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT,
    state TEXT,
    board_id TEXT,
    start_date TEXT,
    end_date TEXT,
    complete_date TEXT,
    goal TEXT,
    raw_json TEXT
);

CREATE TABLE IF NOT EXISTS sprint_issues (
    sprint_id TEXT NOT NULL,
    issue_id TEXT NOT NULL,
    PRIMARY KEY (sprint_id, issue_id),
    FOREIGN KEY (sprint_id) REFERENCES sprints(id),
    FOREIGN KEY (issue_id) REFERENCES issues(id)
);

CREATE TABLE IF NOT EXISTS worklogs (
    id TEXT PRIMARY KEY NOT NULL,
    issue_id TEXT NOT NULL,
    author_account_id TEXT,
    started TEXT,
    time_spent_seconds INTEGER,
    raw_json TEXT,
    FOREIGN KEY (issue_id) REFERENCES issues(id)
);

CREATE TABLE IF NOT EXISTS issue_links (
    id TEXT PRIMARY KEY NOT NULL,
    source_issue_id TEXT NOT NULL,
    destination_issue_id TEXT NOT NULL,
    link_type TEXT,
    FOREIGN KEY (source_issue_id) REFERENCES issues(id),
    FOREIGN KEY (destination_issue_id) REFERENCES issues(id)
);

CREATE TABLE IF NOT EXISTS sync_checkpoints (
    scope_key TEXT PRIMARY KEY NOT NULL,
    next_page_token TEXT,
    jql_cursor TEXT,
    last_updated_watermark TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sync_checkpoints_scope_key ON sync_checkpoints(scope_key);

CREATE TABLE IF NOT EXISTS field_map (
    logical_name TEXT PRIMARY KEY NOT NULL,
    jira_field_id TEXT,
    jira_field_name TEXT,
    status TEXT NOT NULL DEFAULT 'unresolved',
    candidates_json TEXT
);

-- Derived stubs (populated by later analytics/risk tasks).

CREATE TABLE IF NOT EXISTS derived_time_in_status (
    issue_id TEXT NOT NULL,
    status TEXT NOT NULL,
    duration_secs INTEGER NOT NULL,
    PRIMARY KEY (issue_id, status)
);

CREATE TABLE IF NOT EXISTS derived_issue_cycle (
    issue_id TEXT PRIMARY KEY NOT NULL,
    cycle_secs INTEGER,
    lead_secs INTEGER,
    completed_at TEXT
);

CREATE TABLE IF NOT EXISTS derived_throughput_daily (
    day TEXT NOT NULL,
    project_key TEXT NOT NULL,
    completed_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (day, project_key)
);

CREATE TABLE IF NOT EXISTS derived_sprint_metrics (
    sprint_id TEXT PRIMARY KEY NOT NULL,
    committed INTEGER,
    completed INTEGER,
    spillover INTEGER,
    scope_added INTEGER,
    scope_removed INTEGER,
    velocity_points REAL
);

CREATE TABLE IF NOT EXISTS derived_epic_risk (
    epic_key TEXT PRIMARY KEY NOT NULL,
    risk_score REAL,
    finish_by_probability REAL,
    assumptions_json TEXT,
    drivers_json TEXT
);

CREATE TABLE IF NOT EXISTS derived_completions (
    issue_id TEXT PRIMARY KEY NOT NULL,
    project_key TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    finisher_account_id TEXT,
    story_points REAL,
    attribution TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_derived_completions_finisher_month
    ON derived_completions (finisher_account_id, completed_at);
CREATE INDEX IF NOT EXISTS idx_derived_completions_project
    ON derived_completions (project_key, completed_at);

CREATE TABLE IF NOT EXISTS derived_person_month (
    month TEXT NOT NULL,
    account_id TEXT NOT NULL,
    completed_count INTEGER NOT NULL,
    points REAL,
    PRIMARY KEY (month, account_id)
);
