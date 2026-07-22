# AandG Analytics Desktop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a local-only macOS-first Tauri app that syncs full Jira Cloud history into SQLite, computes flow/sprint/epic/risk analytics, and answers questions via the user’s Gemini API key.

**Architecture:** React UI talks to a Rust core over Tauri IPC. Rust owns Jira sync, SQLite (raw + derived), analytics/risk math, Gemini context packs, and OS keychain credentials. UI never queries Jira live.

**Tech Stack:** Tauri 2, Rust 2021 edition, SQLite (`rusqlite` + migrations), React 18 + Vite + TypeScript, Prettier, ESLint, `rustfmt`, `clippy`, Google Gemini API, Jira Cloud REST v3.

**Spec:** `docs/superpowers/specs/2026-07-23-jira-analytics-desktop-design.md`

## Global Constraints

- Local-only: no product-hosted backend; outbound network is Jira Cloud + Gemini only.
- Each user supplies their own Jira API token and Gemini API key; secrets live in the OS keychain only.
- Sync scope: all projects the token can access; initial sync is full historical; incremental every 5–15 minutes.
- macOS first (signed/notarized `.dmg`); Windows/Linux later via same Tauri project.
- Test-first: failing test → implement → passing test → refactor; CI enforces tests, Prettier, ESLint, `rustfmt`, `clippy`.
- Metrics live once in Rust `analytics` / `risk` crates — UI must not reimplement them.
- Prefer small single-purpose modules; extract duplication on second use.

---

## File structure (lock this in)

```
AandGAnalytics/
├── .github/workflows/ci.yml
├── .prettierrc
├── .eslintrc.cjs
├── package.json                 # workspace scripts: ui lint/format/test
├── Cargo.toml                   # workspace
├── crates/
│   ├── ag_db/                   # schema, migrations, connection
│   ├── ag_jira/                 # REST client + DTOs + fixtures
│   ├── ag_sync/                 # full/incremental sync + checkpoints
│   ├── ag_analytics/            # flow, sprint, throughput, events
│   ├── ag_risk/                 # epic at-risk + finish-by probability
│   ├── ag_gemini/               # Gemini client + context pack builder
│   └── ag_credentials/          # keychain wrapper (testable trait)
├── src-tauri/                   # Tauri app: commands, scheduler, wiring
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── setup.rs
│       │   ├── sync.rs
│       │   ├── metrics.rs
│       │   └── ai.rs
│       └── scheduler.rs
├── ui/                          # React + Vite + TS
│   ├── package.json
│   ├── vite.config.ts
│   ├── src/
│   │   ├── main.tsx
│   │   ├── App.tsx
│   │   ├── lib/tauri.ts
│   │   ├── pages/
│   │   │   ├── SetupPage.tsx
│   │   │   ├── SyncPage.tsx
│   │   │   ├── HomePage.tsx
│   │   │   ├── FlowPage.tsx
│   │   │   ├── SprintsPage.tsx
│   │   │   ├── EpicsPage.tsx
│   │   │   ├── ExplorePage.tsx
│   │   │   └── AskAiPage.tsx
│   │   └── components/
│   │       ├── SyncBanner.tsx
│   │       ├── FilterBar.tsx
│   │       └── MetricChart.tsx
│   └── src/**/*.test.tsx
└── docs/superpowers/...
```

**Phase note:** Tasks 1–8 produce a headless, fully tested analytics core. Tasks 9–14 wire the desktop UI and AI. Task 15 packages macOS. Each task must leave `cargo test` (and UI tests when present) green.

---

### Task 1: Workspace scaffold + CI quality gates

**Files:**
- Create: `Cargo.toml`, `crates/ag_db/Cargo.toml`, `crates/ag_db/src/lib.rs`, `crates/ag_jira/Cargo.toml`, `crates/ag_jira/src/lib.rs`, `crates/ag_sync/Cargo.toml`, `crates/ag_sync/src/lib.rs`, `crates/ag_analytics/Cargo.toml`, `crates/ag_analytics/src/lib.rs`, `crates/ag_risk/Cargo.toml`, `crates/ag_risk/src/lib.rs`, `crates/ag_gemini/Cargo.toml`, `crates/ag_gemini/src/lib.rs`, `crates/ag_credentials/Cargo.toml`, `crates/ag_credentials/src/lib.rs`
- Create: `ui/package.json`, `ui/tsconfig.json`, `ui/vite.config.ts`, `ui/index.html`, `ui/src/main.tsx`, `ui/src/App.tsx`, `.prettierrc`, `.eslintrc.cjs`, `.github/workflows/ci.yml`, `package.json` (root scripts)
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs` (minimal stub ok until Task 9)

**Interfaces:**
- Produces: Cargo workspace members listed above; `npm run lint` / `npm run format:check` in `ui/`; `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`; `cargo fmt --check`

- [ ] **Step 1: Write the failing CI-oriented sanity test in `ag_db`**

```rust
// crates/ag_db/src/lib.rs
//! SQLite schema, migrations, and connection helpers for AandG Analytics.

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles_and_db_crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ag_db");
    }
}
```

Create sibling crates with the same pattern (`ag_jira`, `ag_sync`, `ag_analytics`, `ag_risk`, `ag_gemini`, `ag_credentials`) so the workspace graph is real.

- [ ] **Step 2: Run test to verify workspace fails before `Cargo.toml` exists**

Run: `cargo test -p ag_db`  
Expected: FAIL (no such package / not a Cargo project)

- [ ] **Step 3: Create workspace `Cargo.toml` and crate stubs**

```toml
# Cargo.toml
[workspace]
resolver = "2"
members = [
  "crates/ag_db",
  "crates/ag_jira",
  "crates/ag_sync",
  "crates/ag_analytics",
  "crates/ag_risk",
  "crates/ag_gemini",
  "crates/ag_credentials",
  "src-tauri",
]

[workspace.package]
edition = "2021"
license = "UNLICENSED"
```

Each crate `Cargo.toml`:

```toml
[package]
name = "ag_db"
version = "0.1.0"
edition.workspace = true
```

Scaffold `ui/` with Vite React-TS; add Prettier + ESLint; root `package.json`:

```json
{
  "name": "aandg-analytics",
  "private": true,
  "scripts": {
    "ui:dev": "npm --prefix ui run dev",
    "ui:build": "npm --prefix ui run build",
    "ui:test": "npm --prefix ui run test",
    "ui:lint": "npm --prefix ui run lint",
    "ui:format:check": "npm --prefix ui run format:check"
  }
}
```

CI workflow must run: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `npm ci` in `ui`, `npm run lint`, `npm run format:check`, `npm test`.

- [ ] **Step 4: Run tests and quality checks**

Run:

```bash
cargo test --workspace
cargo fmt --check
cargo clippy --workspace -- -D warnings
npm --prefix ui ci
npm --prefix ui run lint
npm --prefix ui run format:check
npm --prefix ui test
```

Expected: all PASS (UI may have a single placeholder test).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates ui src-tauri package.json .prettierrc .eslintrc.cjs .github
git commit -m "chore: scaffold Tauri workspace with CI quality gates"
```

---

### Task 2: Credentials store (keychain trait + in-memory fake)

**Files:**
- Create: `crates/ag_credentials/src/lib.rs`, `crates/ag_credentials/src/store.rs`
- Test: same files (`#[cfg(test)]`)

**Interfaces:**
- Produces:
  - `pub struct JiraCredentials { pub site_url: String, pub email: String, pub api_token: String }`
  - `pub struct GeminiCredentials { pub api_key: String }`
  - `pub trait CredentialStore: Send + Sync { fn save_jira(&self, creds: &JiraCredentials) -> Result<(), CredentialError>; fn load_jira(&self) -> Result<Option<JiraCredentials>, CredentialError>; fn save_gemini(&self, creds: &GeminiCredentials) -> Result<(), CredentialError>; fn load_gemini(&self) -> Result<Option<GeminiCredentials>, CredentialError>; fn clear_all(&self) -> Result<(), CredentialError>; }`
  - `pub struct MemoryCredentialStore` (for tests)
  - `pub struct KeychainCredentialStore` (OS-backed; thin wrapper, mocked in unit tests via trait)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn memory_store_round_trips_jira_and_gemini() {
    let store = MemoryCredentialStore::default();
    assert!(store.load_jira().unwrap().is_none());

    store
        .save_jira(&JiraCredentials {
            site_url: "https://example.atlassian.net".into(),
            email: "dev@example.com".into(),
            api_token: "secret-jira".into(),
        })
        .unwrap();
    store
        .save_gemini(&GeminiCredentials {
            api_key: "secret-gemini".into(),
        })
        .unwrap();

    let jira = store.load_jira().unwrap().unwrap();
    assert_eq!(jira.api_token, "secret-jira");
    assert_eq!(store.load_gemini().unwrap().unwrap().api_key, "secret-gemini");

    store.clear_all().unwrap();
    assert!(store.load_jira().unwrap().is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ag_credentials memory_store_round_trips -- --nocapture`  
Expected: FAIL (types/store missing)

- [ ] **Step 3: Implement `CredentialStore` + `MemoryCredentialStore`**

Use `std::sync::Mutex` for the memory store. Document that production uses keychain and must never log token values. Implement `KeychainCredentialStore` behind `#[cfg(not(test))]` or always compile but only integration-tested on macOS — unit tests use `MemoryCredentialStore` only.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ag_credentials`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ag_credentials
git commit -m "feat: add credential store trait with in-memory backend"
```

---

### Task 3: SQLite schema + migrations

**Files:**
- Create: `crates/ag_db/src/lib.rs`, `crates/ag_db/src/migrate.rs`, `crates/ag_db/src/schema.sql`, `crates/ag_db/src/connection.rs`
- Test: `crates/ag_db/src/migrate.rs` tests using tempfile DB

**Interfaces:**
- Produces:
  - `pub fn open_db(path: &Path) -> Result<Connection, DbError>`
  - `pub fn migrate(conn: &Connection) -> Result<(), DbError>`
  - Tables (minimum): `meta`, `projects`, `issues`, `issue_changelog`, `sprints`, `sprint_issues`, `worklogs`, `issue_links`, `sync_checkpoints`, `field_map`, plus derived stubs: `derived_time_in_status`, `derived_issue_cycle`, `derived_throughput_daily`, `derived_sprint_metrics`, `derived_epic_risk` (created empty; filled in later tasks)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn migrate_creates_core_tables() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let conn = open_db(&path).unwrap();
    migrate(&conn).unwrap();

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
        .unwrap();
    for table in [
        "projects",
        "issues",
        "issue_changelog",
        "sprints",
        "sync_checkpoints",
        "derived_issue_cycle",
    ] {
        let found: Option<String> = stmt.query_row([table], |r| r.get(0)).ok();
        assert_eq!(found.as_deref(), Some(table), "missing table {table}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ag_db migrate_creates_core_tables -- --nocapture`  
Expected: FAIL

- [ ] **Step 3: Implement `schema.sql` + `migrate`**

Apply schema via `include_str!("schema.sql")` in a single versioned migration (`PRAGMA user_version`). Include indexes on `issues(project_key)`, `issues(updated)`, `issue_changelog(issue_id)`, `sync_checkpoints(scope_key)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ag_db`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ag_db
git commit -m "feat: add SQLite schema and migrations"
```

---

### Task 4: Jira HTTP client with fixtures (auth, fields, paginated search)

**Files:**
- Create: `crates/ag_jira/src/lib.rs`, `crates/ag_jira/src/client.rs`, `crates/ag_jira/src/types.rs`, `crates/ag_jira/src/error.rs`
- Create: `crates/ag_jira/tests/fixtures/myself.json`, `fields.json`, `search_page1.json`, `search_page2.json`
- Test: `crates/ag_jira/src/client.rs` using `httpmock` or `wiremock`

**Interfaces:**
- Consumes: `JiraCredentials`
- Produces:
  - `pub struct JiraClient<H: HttpDoer> { ... }`
  - `pub trait HttpDoer { async fn request(&self, req: HttpRequest) -> Result<HttpResponse, JiraError>; }`
  - `impl JiraClient { pub async fn get_myself(&self) -> Result<Myself, JiraError>; pub async fn list_fields(&self) -> Result<Vec<JiraField>, JiraError>; pub async fn search_issues_page(&self, jql: &str, next_page_token: Option<&str>, expand_changelog: bool) -> Result<IssueSearchPage, JiraError>; pub async fn list_projects(&self) -> Result<Vec<Project>, JiraError>; }`
  - Types: `Issue`, `Changelog`, `ChangelogItem`, `Project`, `Sprint` (as needed from Agile API in Task 5)

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn get_myself_parses_fixture() {
    let server = httpmock::MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET").path("/rest/api/3/myself");
        then.status(200)
            .header("content-type", "application/json")
            .body(include_str!("../tests/fixtures/myself.json"));
    });

    let client = JiraClient::new_for_test(
        &server.base_url(),
        "dev@example.com",
        "token",
    );
    let me = client.get_myself().await.unwrap();
    assert_eq!(me.account_id, "abc123");
    mock.assert();
}
```

Add a second test that `search_issues_page` follows pagination tokens from fixtures.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ag_jira get_myself_parses_fixture -- --nocapture`  
Expected: FAIL

- [ ] **Step 3: Implement client**

Use `reqwest` in production `HttpDoer`. Basic auth with email + API token. On `429`, read `Retry-After` when present and return `JiraError::RateLimited { retry_after_ms }`. Never log Authorization headers.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ag_jira`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ag_jira
git commit -m "feat: add Jira Cloud client with fixture-backed tests"
```

---

### Task 5: Sync engine (full + incremental + checkpoints)

**Files:**
- Create: `crates/ag_sync/src/lib.rs`, `crates/ag_sync/src/engine.rs`, `crates/ag_sync/src/checkpoint.rs`, `crates/ag_sync/src/progress.rs`
- Test: `crates/ag_sync/src/engine.rs` with mock `JiraClient` / fixtures writing into temp SQLite

**Interfaces:**
- Consumes: `open_db`, `migrate`, `JiraClient`, tables from Task 3
- Produces:
  - `pub struct SyncEngine<'a> { ... }`
  - `pub struct SyncProgress { pub phase: SyncPhase, pub projects_done: u32, pub projects_total: u32, pub issues_synced: u64, pub message: String }`
  - `pub enum SyncPhase { Projects, Issues, Sprints, Derived, Idle, Failed }`
  - `impl SyncEngine { pub async fn run_full(&mut self, on_progress: impl Fn(SyncProgress)) -> Result<(), SyncError>; pub async fn run_incremental(&mut self, on_progress: impl Fn(SyncProgress)) -> Result<(), SyncError>; pub fn pause(&mut self); pub fn resume_full(&mut self, on_progress: ...) -> ...; }`
  - Checkpoint rows keyed by `scope_key` (e.g. `issues:PROJECT` or `issues:global`) storing `next_page_token` / `jql_cursor` / `last_updated_watermark`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn full_sync_is_resumable_after_interrupt() {
    let db = open_temp_migrated_db();
    let jira = FakeJira::from_fixtures_two_pages();
    let mut engine = SyncEngine::new(&db, jira);

    engine.fail_after_issues(1); // test hook: stop after first page
    let err = engine.run_full(|_| {}).await;
    assert!(err.is_err() || engine.checkpoint_exists("issues:global"));

    engine.clear_fail_hook();
    engine.run_full(|_| {}).await.unwrap();

    let count: i64 = db.query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 3); // fixture issue count
}
```

Also add: `incremental_sync_only_fetches_updated_since_watermark`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ag_sync full_sync_is_resumable -- --nocapture`  
Expected: FAIL

- [ ] **Step 3: Implement sync**

Algorithm:
1. List projects → upsert `projects`.
2. JQL `order by updated asc` (or project-scoped loops) with `expand=changelog`; upsert issues + changelog rows; save checkpoint after each page.
3. Pull boards/sprints via Agile API; upsert `sprints` / `sprint_issues`.
4. On success, set meta `last_incremental_watermark` to max `issues.updated`.
5. Incremental: JQL `updated >= "watermark"` with changelog expand.

Persist story-points field id into `field_map` when discovered (heuristic: field name/clause matching `story points` / known `customfield_`); if multiple candidates, leave unresolved for UI confirmation in Task 10.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ag_sync`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ag_sync
git commit -m "feat: add resumable full and incremental Jira sync"
```

---

### Task 6: Changelog → transitions + time-in-status / cycle / lead time

**Files:**
- Create: `crates/ag_analytics/src/lib.rs`, `crates/ag_analytics/src/changelog.rs`, `crates/ag_analytics/src/flow.rs`, `crates/ag_analytics/src/rebuild.rs`
- Test: `crates/ag_analytics/src/changelog.rs`, `flow.rs`

**Interfaces:**
- Produces:
  - `pub struct StatusTransition { pub issue_id: String, pub from_status: Option<String>, pub to_status: String, pub at: DateTime<Utc> }`
  - `pub fn transitions_from_changelog(issue_id: &str, histories: &[ChangelogHistory]) -> Vec<StatusTransition>`
  - `pub struct TimeInStatus { pub issue_id: String, pub status: String, pub duration_secs: i64 }`
  - `pub fn time_in_status(transitions: &[StatusTransition], done_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Vec<TimeInStatus>`
  - `pub struct CycleLeadTimes { pub issue_id: String, pub cycle_secs: Option<i64>, pub lead_secs: Option<i64> }`
  - `pub fn cycle_and_lead(created: DateTime<Utc>, first_in_progress: Option<DateTime<Utc>>, completed: Option<DateTime<Utc>>) -> CycleLeadTimes`
  - `pub fn rebuild_flow_derived(conn: &Connection, now: DateTime<Utc>) -> Result<(), AnalyticsError>`
  - Config: status category mapping table/meta for “active” vs “waiting” (defaults: In Progress/active; To Do/waiting; Done terminal)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn time_in_status_handles_reopen_and_same_day_moves() {
    let t0 = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
    let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap(); // To Do -> In Progress
    let t2 = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(); // In Progress -> Done
    let t3 = Utc.with_ymd_and_hms(2024, 1, 2, 9, 0, 0).unwrap(); // Done -> In Progress (reopen)
    let t4 = Utc.with_ymd_and_hms(2024, 1, 2, 11, 0, 0).unwrap(); // In Progress -> Done

    let transitions = vec![
        StatusTransition { issue_id: "1".into(), from_status: Some("To Do".into()), to_status: "In Progress".into(), at: t1 },
        StatusTransition { issue_id: "1".into(), from_status: Some("In Progress".into()), to_status: "Done".into(), at: t2 },
        StatusTransition { issue_id: "1".into(), from_status: Some("Done".into()), to_status: "In Progress".into(), at: t3 },
        StatusTransition { issue_id: "1".into(), from_status: Some("In Progress".into()), to_status: "Done".into(), at: t4 },
    ];

    let tis = time_in_status(&transitions, Some(t4), t4);
    let in_progress: i64 = tis.iter().filter(|r| r.status == "In Progress").map(|r| r.duration_secs).sum();
    assert_eq!(in_progress, 2 * 3600 + 2 * 3600); // 10-12 and 09-11
}
```

Add: `cycle_time_is_first_in_progress_to_done`; `lead_time_is_created_to_done`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ag_analytics time_in_status_handles_reopen -- --nocapture`  
Expected: FAIL

- [ ] **Step 3: Implement pure functions + `rebuild_flow_derived`**

Write derived rows into `derived_time_in_status` and `derived_issue_cycle`. Pure functions stay free of SQLite for easy testing.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ag_analytics`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ag_analytics
git commit -m "feat: compute time-in-status, cycle time, and lead time"
```

---

### Task 7: Throughput, bottlenecks, reopens, handoffs, scope changes, sprint metrics

**Files:**
- Create: `crates/ag_analytics/src/throughput.rs`, `bottleneck.rs`, `events.rs`, `sprint.rs`
- Modify: `crates/ag_analytics/src/rebuild.rs`
- Test: each new module

**Interfaces:**
- Produces:
  - `pub fn daily_throughput(completed_at_by_issue: &[(String, DateTime<Utc>)]) -> BTreeMap<NaiveDate, u64>`
  - `pub fn bottleneck_by_status(time_in_status: &[TimeInStatus]) -> Vec<(String, i64)>` // status, total secs desc
  - `pub fn detect_reopens(transitions: &[StatusTransition]) -> u32` // Done -> non-Done
  - `pub fn detect_handoffs(assignee_changes: &[(DateTime<Utc>, Option<String>, Option<String>)]) -> u32`
  - `pub fn detect_scope_changes(field_changes: &[FieldChange]) -> ScopeChangeStats`
  - `pub struct SprintMetrics { pub sprint_id: String, pub committed: u32, pub completed: u32, pub spillover: u32, pub scope_added: u32, pub scope_removed: u32, pub velocity_points: Option<f64> }`
  - `pub fn compute_sprint_metrics(...) -> SprintMetrics`
  - `pub fn rebuild_sprint_derived(conn: &Connection) -> Result<(), AnalyticsError>`
  - `pub fn rebuild_event_derived(conn: &Connection) -> Result<(), AnalyticsError>`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn sprint_metrics_counts_commitment_completion_and_spillover() {
    let m = compute_sprint_metrics(
        "s1",
        /* committed keys */ &["A-1", "A-2", "A-3"],
        /* completed in sprint */ &["A-1", "A-2"],
        /* added mid */ &["A-4"],
        /* removed mid */ &["A-3"],
        /* points completed */ Some(5.0),
    );
    assert_eq!(m.committed, 3);
    assert_eq!(m.completed, 2);
    assert_eq!(m.spillover, 1); // A-3 removed or unfinished per definition in docstring
    assert_eq!(m.scope_added, 1);
    assert_eq!(m.scope_removed, 1);
}
```

Document spillover precisely in the function docstring: issues committed at sprint start that are not Done at sprint end.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p ag_analytics sprint_metrics_counts -- --nocapture`  
Expected: FAIL

- [ ] **Step 3: Implement metrics + rebuild hooks**

Call these from sync completion (`SyncPhase::Derived`) via `rebuild_all_derived(conn, now)`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p ag_analytics`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ag_analytics
git commit -m "feat: add throughput, sprint, and flow-event analytics"
```

---

### Task 8: Epic at-risk score + finish-by probability

**Files:**
- Create: `crates/ag_risk/src/lib.rs`, `crates/ag_risk/src/score.rs`, `crates/ag_risk/src/forecast.rs`
- Test: `score.rs`, `forecast.rs`

**Interfaces:**
- Produces:
  - `pub struct EpicRiskInput { pub epic_key: String, pub remaining_issues: u32, pub remaining_points: Option<f64>, pub avg_weekly_throughput_issues: f64, pub avg_weekly_throughput_points: Option<f64>, pub blocked_secs_total: i64, pub open_issue_age_secs_p50: i64, pub recent_scope_growth: f64, pub recent_spillover_rate: f64 }`
  - `pub struct EpicRiskResult { pub epic_key: String, pub score: f64, /* 0=safe .. 100=high risk */ pub drivers: Vec<String> }`
  - `pub fn score_epic(input: &EpicRiskInput) -> EpicRiskResult`
  - `pub struct FinishByInput { pub remaining_work_issues: f64, pub weekly_throughput_issues: f64, pub weeks_until_target: f64, pub throughput_stddev: f64 }`
  - `pub struct FinishByResult { pub probability: f64, pub assumptions: Vec<String> }`
  - `pub fn finish_by_probability(input: &FinishByInput) -> FinishByResult` // normal/approx using mean throughput; clamp 0..1; list assumptions
  - `pub fn rebuild_epic_risk(conn: &Connection, now: DateTime<Utc>) -> Result<(), RiskError>`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn finish_by_probability_is_high_when_throughput_covers_remaining() {
    let r = finish_by_probability(&FinishByInput {
        remaining_work_issues: 10.0,
        weekly_throughput_issues: 10.0,
        weeks_until_target: 2.0,
        throughput_stddev: 1.0,
    });
    assert!(r.probability > 0.8);
    assert!(!r.assumptions.is_empty());
}

#[test]
fn epic_risk_rises_with_low_throughput_and_scope_growth() {
    let low = score_epic(&EpicRiskInput {
        epic_key: "E-1".into(),
        remaining_issues: 40,
        remaining_points: None,
        avg_weekly_throughput_issues: 2.0,
        avg_weekly_throughput_points: None,
        blocked_secs_total: 0,
        open_issue_age_secs_p50: 0,
        recent_scope_growth: 0.5,
        recent_spillover_rate: 0.4,
    });
    let high_capacity = score_epic(&EpicRiskInput {
        epic_key: "E-2".into(),
        remaining_issues: 4,
        remaining_points: None,
        avg_weekly_throughput_issues: 10.0,
        avg_weekly_throughput_points: None,
        blocked_secs_total: 0,
        open_issue_age_secs_p50: 0,
        recent_scope_growth: 0.0,
        recent_spillover_rate: 0.0,
    });
    assert!(low.score > high_capacity.score);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p ag_risk -- --nocapture`  
Expected: FAIL

- [ ] **Step 3: Implement scoring**

Keep formulas documented in module-level rustdoc (weights summing to a 0–100 score). Persist into `derived_epic_risk`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p ag_risk`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ag_risk
git commit -m "feat: add epic at-risk scoring and finish-by probability"
```

---

### Task 9: Tauri commands + scheduler wiring

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, `src-tauri/src/main.rs`
- Create: `src-tauri/src/commands/mod.rs`, `setup.rs`, `sync.rs`, `metrics.rs`, `state.rs`, `scheduler.rs`
- Test: command handlers via thin pure wrappers tested in crates; add `src-tauri` unit tests for filter DTO validation

**Interfaces:**
- Produces Tauri commands (serde types):
  - `save_setup(jira: JiraCredentialsDto, gemini: GeminiCredentialsDto) -> Result<(), String>`
  - `validate_setup() -> Result<SetupStatus, String>` // probes Jira myself + Gemini list/models or simple generate
  - `start_full_sync() -> Result<(), String>`
  - `start_incremental_sync() -> Result<(), String>`
  - `get_sync_progress() -> Result<SyncProgress, String>`
  - `get_flow_metrics(filter: MetricsFilter) -> Result<FlowMetricsDto, String>`
  - `get_sprint_metrics(filter: MetricsFilter) -> Result<Vec<SprintMetricsDto>, String>`
  - `get_epic_risk(filter: MetricsFilter) -> Result<Vec<EpicRiskDto>, String>`
  - `get_finish_by(epic_key: String, target_date: String) -> Result<FinishByResultDto, String>`
  - `list_issues(filter: MetricsFilter, page: Page) -> Result<IssuePageDto, String>`
  - Events: `sync-progress` emitted to UI

`MetricsFilter { project_keys: Option<Vec<String>>, from: Option<String /* ISO date */>, to: Option<String>, issue_types: Option<Vec<String>>, assignee_ids: Option<Vec<String>> }`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn metrics_filter_rejects_inverted_date_range() {
    let err = MetricsFilter {
        project_keys: None,
        from: Some("2025-01-10".into()),
        to: Some("2025-01-01".into()),
        issue_types: None,
        assignee_ids: None,
    }
    .validate();
    assert!(err.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aandg-analytics-tauri metrics_filter_rejects -- --nocapture`  
(Use actual package name from `src-tauri/Cargo.toml`.)  
Expected: FAIL

- [ ] **Step 3: Implement commands + 10-minute incremental scheduler**

On app start: if credentials exist and DB exists, spawn scheduler (`tokio` interval 10 minutes) calling `run_incremental`. Store `AppState { db_path, credentials: Arc<dyn CredentialStore>, sync: Mutex<SyncHandle> }`.

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri
git commit -m "feat: wire Tauri commands and incremental sync scheduler"
```

---

### Task 10: Setup + Sync UI pages

**Files:**
- Create: `ui/src/pages/SetupPage.tsx`, `SyncPage.tsx`, `ui/src/lib/tauri.ts`, `ui/src/components/SyncBanner.tsx`
- Test: `ui/src/pages/SetupPage.test.tsx`, `SyncPage.test.tsx` (Vitest + Testing Library; mock `@tauri-apps/api/core` `invoke`)

**Interfaces:**
- Consumes: `save_setup`, `validate_setup`, `start_full_sync`, `get_sync_progress`, `sync-progress` event
- Produces: routes `/setup`, `/sync`; redirect to `/setup` when credentials missing

- [ ] **Step 1: Write the failing test**

```tsx
it("disables continue until jira and gemini fields are filled", () => {
  render(<SetupPage />);
  expect(screen.getByRole("button", { name: /save and continue/i })).toBeDisabled();
  fireEvent.change(screen.getByLabelText(/site url/i), {
    target: { value: "https://example.atlassian.net" },
  });
  fireEvent.change(screen.getByLabelText(/email/i), {
    target: { value: "dev@example.com" },
  });
  fireEvent.change(screen.getByLabelText(/jira api token/i), {
    target: { value: "j-token" },
  });
  fireEvent.change(screen.getByLabelText(/gemini api key/i), {
    target: { value: "g-key" },
  });
  expect(screen.getByRole("button", { name: /save and continue/i })).toBeEnabled();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test -- SetupPage`  
Expected: FAIL

- [ ] **Step 3: Implement Setup + Sync pages**

Sync page shows phase, counts, ETA text from progress; Pause/Resume if exposed; error banner with Retry. After first successful page of issues, show link “Browse dashboards while syncing”.

- [ ] **Step 4: Run tests**

Run: `npm --prefix ui test` && `npm --prefix ui run lint` && `npm --prefix ui run format:check`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui
git commit -m "feat: add setup and sync progress UI"
```

---

### Task 11: Flow + Home dashboard UI

**Files:**
- Create: `ui/src/pages/HomePage.tsx`, `FlowPage.tsx`, `ui/src/components/FilterBar.tsx`, `MetricChart.tsx`
- Test: `FlowPage.test.tsx`, `FilterBar.test.tsx`

**Interfaces:**
- Consumes: `get_flow_metrics(filter)` returning cycle/lead percentiles, throughput series, bottleneck bars, flow efficiency

- [ ] **Step 1: Write the failing test**

```tsx
it("renders bottleneck statuses from metrics payload", async () => {
  mockInvoke({
    bottlenecks: [
      { status: "Code Review", total_secs: 90000 },
      { status: "In Progress", total_secs: 40000 },
    ],
  });
  render(<FlowPage />);
  expect(await screen.findByText("Code Review")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test -- FlowPage`  
Expected: FAIL

- [ ] **Step 3: Implement Home + Flow**

Home: summary cards (throughput, median cycle time, top bottleneck, at-risk epic count). Flow: charts + tables. Use one chart wrapper (`MetricChart`) to avoid duplication.

- [ ] **Step 4: Run tests + lint/format**

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui
git commit -m "feat: add home and flow analytics dashboards"
```

---

### Task 12: Sprints + Epics risk UI

**Files:**
- Create: `ui/src/pages/SprintsPage.tsx`, `EpicsPage.tsx`
- Test: `SprintsPage.test.tsx`, `EpicsPage.test.tsx`

**Interfaces:**
- Consumes: `get_sprint_metrics`, `get_epic_risk`
- Epics page shows score, drivers, finish-by probability + assumptions list for selected epic/target date via `get_finish_by`

- [ ] **Step 1: Write failing tests for sprint table and epic risk list**

```tsx
it("shows spillover count per sprint", async () => {
  mockInvoke([{ name: "Sprint 42", spillover: 3, committed: 10, completed: 7 }]);
  render(<SprintsPage />);
  expect(await screen.findByText("Sprint 42")).toBeInTheDocument();
  expect(screen.getByText(/spillover:\s*3/i)).toBeInTheDocument();
});

it("lists epic risk drivers and finish-by assumptions", async () => {
  mockInvokeSequence([
    [{ epic_key: "E-1", score: 72, drivers: ["low throughput"] }],
    { probability: 0.41, assumptions: ["throughput ~ last 6 weeks"] },
  ]);
  render(<EpicsPage />);
  expect(await screen.findByText("E-1")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText(/target date/i), {
    target: { value: "2026-12-01" },
  });
  expect(await screen.findByText(/throughput ~ last 6 weeks/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- SprintsPage EpicsPage`  
Expected: FAIL (pages missing)

- [ ] **Step 3: Implement pages (assumptions visible beside probability)**

Reuse `FilterBar`. Do not recompute risk in the UI — only render DTOs from Tauri.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm --prefix ui test -- SprintsPage EpicsPage` && `npm --prefix ui run lint`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui src-tauri
git commit -m "feat: add sprint and epic risk dashboards"
```

---

### Task 13: Explore issues table

**Files:**
- Create: `ui/src/pages/ExplorePage.tsx`
- Test: `ui/src/pages/ExplorePage.test.tsx`
- Modify: `src-tauri/src/commands/metrics.rs` if `list_issues` needs extra columns (cycle time, current status)

**Interfaces:**
- Consumes: `list_issues(filter, page)` → `{ items: IssueRowDto[], total: number }`
- `IssueRowDto { key, summary, project_key, status, assignee, story_points, cycle_secs, updated }`
- `Page { offset: number, limit: number }`

- [ ] **Step 1: Write failing test for pagination label and row render**

```tsx
it("renders issue rows and page total", async () => {
  mockInvoke({
    total: 2,
    items: [
      {
        key: "PROJ-1",
        summary: "Wire sync",
        project_key: "PROJ",
        status: "Done",
        assignee: "Ada",
        story_points: 3,
        cycle_secs: 86400,
        updated: "2026-01-02T00:00:00Z",
      },
    ],
  });
  render(<ExplorePage />);
  expect(await screen.findByText("PROJ-1")).toBeInTheDocument();
  expect(screen.getByText(/total:\s*2/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test -- ExplorePage`  
Expected: FAIL

- [ ] **Step 3: Implement table with FilterBar reuse**

Columns: key, summary, project, status, assignee, points, cycle time, updated. Pagination controls call `list_issues` with new offsets.

- [ ] **Step 4: Run test to verify it passes**

Run: `npm --prefix ui test -- ExplorePage` && `npm --prefix ui run lint`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui src-tauri
git commit -m "feat: add issue explore table"
```

---

### Task 14: Gemini client, context pack, Ask AI UI

**Files:**
- Create: `crates/ag_gemini/src/lib.rs`, `client.rs`, `context.rs`
- Create: `src-tauri/src/commands/ai.rs`
- Create: `ui/src/pages/AskAiPage.tsx`, `ui/src/components/ContextPackPreview.tsx`
- Test: `crates/ag_gemini/src/context.rs`, `client.rs` (httpmock), `AskAiPage.test.tsx`

**Interfaces:**
- Produces:
  - `pub struct ContextPack { pub filter_summary: String, pub metrics_markdown: String, pub supporting_issues: Vec<IssueCite>, pub approx_tokens: usize }`
  - `pub fn build_context_pack(conn: &Connection, filter: &MetricsFilter, token_budget: usize) -> Result<ContextPack, GeminiError>`
  - `pub struct GeminiClient { ... } pub async fn ask(&self, pack: &ContextPack, question: &str) -> Result<GeminiAnswer, GeminiError>`
  - `GeminiAnswer { text: String, citations: Vec<String> }`
  - Commands: `preview_context_pack(filter)`, `ask_ai(filter, question)`, `get_suggested_prompts()`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn context_pack_respects_token_budget() {
    let pack = build_context_pack_from_fixture(1_000);
    assert!(pack.approx_tokens <= 1_000);
    assert!(!pack.supporting_issues.is_empty());
}
```

```tsx
it("shows citations returned by ask_ai", async () => {
  mockAskAi({ text: "Review is the bottleneck.", citations: ["PROJ-1", "bottleneck:Code Review"] });
  render(<AskAiPage />);
  fireEvent.click(screen.getByRole("button", { name: /ask/i }));
  expect(await screen.findByText(/PROJ-1/)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run tests — expect FAIL**

Run: `cargo test -p ag_gemini` and `npm --prefix ui test -- AskAi`  
Expected: FAIL

- [ ] **Step 3: Implement Gemini client + UI**

Use Gemini generateContent HTTP API with user key from `CredentialStore`. System instruction: answer only from context pack; cite issue keys and metric names; say you don’t know if insufficient data. Show `ContextPackPreview` collapsible so users see what leaves the machine. Dashboards must keep working if Gemini fails.

- [ ] **Step 4: Run all tests — expect PASS**

Run: `cargo test --workspace && npm --prefix ui test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ag_gemini src-tauri ui
git commit -m "feat: add Gemini Q&A with local context packs"
```

---

### Task 15: Resilience UX + macOS packaging

**Files:**
- Create: `src-tauri/src/commands/maintenance.rs` (`rebuild_derived`, `full_resync`)
- Modify: `ui/src/components/SyncBanner.tsx`, Setup error states for 401/403
- Modify: `src-tauri/tauri.conf.json` bundle identifiers, DMG config
- Create: `docs/release/macos.md` (signing, notarization checklist)
- Test: unit test that rebuild_derived does not delete raw issues; UI test for offline banner copy

**Interfaces:**
- Produces: `rebuild_derived() -> Result<(), String>`, `full_resync() -> Result<(), String>` (resets checkpoints, keeps credentials)
- Bundle: `productName` AandG Analytics; identifier `com.aandganalytics.app`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn rebuild_derived_keeps_raw_issue_rows() {
    let db = db_with_one_issue_and_stale_derived();
    rebuild_derived(&db, Utc::now()).unwrap();
    let n: i64 = db.query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}
```

- [ ] **Step 2: Run — expect FAIL**
- [ ] **Step 3: Implement maintenance commands + release docs; configure Tauri bundle**
- [ ] **Step 4: Run workspace tests — expect PASS; manually note `cargo tauri build` for DMG on a Mac with certs**
- [ ] **Step 5: Commit**

```bash
git add src-tauri ui docs/release
git commit -m "feat: add maintenance actions and macOS bundle config"
```

---

## Spec coverage checklist (self-review)

| Spec requirement | Task(s) |
|------------------|---------|
| Local Tauri + Rust + SQLite + React | 1, 9 |
| Personal Jira + Gemini tokens in keychain | 2, 10, 14 |
| Full-site full-history sync + incremental + resume | 5 |
| Changelog-based flow metrics | 6–7 |
| Sprint metrics | 7, 12 |
| Epic at-risk + finish-by | 8, 12 |
| AI Q&A with citations + context visibility | 14 |
| Error resilience / rebuild derived | 5, 15 |
| macOS installer path | 15 |
| Test-first, Prettier, ESLint, clippy, modularity | 1 + every task |

**Deferred (explicit in spec):** Windows/Linux installers, local LLMs, DORA/VCS enrichment, auto-update, shared DB snapshots.

**Placeholder scan:** none intentional.  
**Type consistency:** `MetricsFilter`, `SyncProgress`, credential structs shared across Tasks 2, 5, 9–14; finish-by exposed via `get_finish_by` by Task 12.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-23-jira-analytics-desktop.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration  
2. **Inline Execution** — execute tasks in this session using executing-plans, with checkpoints for review  

Which approach?
