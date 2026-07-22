# AandG Analytics — Local Jira Engineering Intelligence (Desktop)

**Date:** 2026-07-23  
**Status:** Approved for implementation planning  
**Product:** Local-only macOS-first desktop app (Tauri) that syncs Jira Cloud into SQLite, computes flow/sprint/epic analytics and delivery-risk forecasts, and answers questions via the user’s Gemini API key.

---

## 1. Problem & goals

Jira’s built-in reports do not expose deep flow analytics (time-in-status, bottlenecks, true capacity, epic slip risk) without exporting or rebuilding history from changelogs. Teams want those insights without standing up hosted infrastructure.

**Goals**
- Run entirely on the user’s machine: no product-hosted backend.
- Each user installs the app, supplies their own Jira API token and Gemini API key, and syncs their own local copy of data.
- Deliver the full analytics vision in v1: flow, sprints, epics, at-risk / finish-by predictions, and AI Q&A.
- Ship macOS first (signed/notarized installer); design so Windows/Linux follow with the same core.
- Enforce test-first development, formatting, linting, documentation, modularity, and deduplication across the repo.

**Non-goals (v1)**
- Hosted multi-tenant SaaS or shared sync server.
- DORA / GitHub / CI/CD / incident enrichment (Jira-only starting point).
- Sharing a synced database file as the primary collaboration model (each user syncs with their own token).
- Local LLM runtime (cloud Gemini with user key only; local models may be considered later).

---

## 2. Users & constraints

| Constraint | Decision |
|------------|----------|
| Distribution | Personal install; personal credentials; personal SQLite DB |
| Platforms | macOS first; Windows/Linux later via same Tauri project |
| Jira scope | Everything the token can access (all accessible projects) |
| History | Full historical sync on first run (resumable) |
| AI | Google Gemini via user-provided API key; context sent from the device |
| Hosting | None |

---

## 3. Architecture

```
┌─────────────────────────────────────────────┐
│  Tauri 2 app (macOS .dmg → Win/Linux later) │
│                                             │
│  React UI  ←IPC→  Rust core                 │
│    dashboards       • Jira sync engine      │
│    sprint/flow      • Gemini client         │
│    epic risk        • analytics SQL         │
│    AI chat          • credential store      │
│                     • scheduler             │
│                           │                 │
│                     SQLite DB               │
│                  (raw + derived)            │
└─────────────────────────────────────────────┘
         │                        │
         ▼                        ▼
   Jira Cloud REST          Google Gemini API
   (user API token)         (user API key)
```

**Principles**
- **Local-first:** UI always reads SQLite; Jira is only contacted by the sync engine.
- **Credentials on device:** Jira email + API token and Gemini API key stored in the OS keychain; never in the DB or logs.
- **AI over metrics:** Gemini receives curated context packs (aggregates + top supporting issues), not unbounded raw changelogs.
- **Portable core:** Sync, schema, and metrics live in Rust crates with thin Tauri command bindings so later OS ports are mostly packaging.

---

## 4. Components

| Component | Responsibility |
|-----------|----------------|
| `app` (Tauri shell) | Window lifecycle, IPC commands, background sync scheduler, bundling |
| `ui` (React/Vite) | Setup, sync status, dashboards, explore table, AI chat |
| `jira` | REST client, auth, pagination, rate-limit/backoff, field discovery |
| `sync` | Full + incremental sync, checkpoints/watermarks, progress events |
| `db` | Schema migrations, raw + derived tables, rebuild-derived |
| `analytics` | Cycle/lead time, time-in-status, throughput, sprint metrics, handoffs/reopens/scope |
| `risk` | Epic at-risk scores and finish-by probability (assumptions exposed to UI) |
| `gemini` | API client, context-pack builder (token budget), citation-oriented prompts |
| `credentials` | Keychain read/write; validation probes |

---

## 5. Sync & data model

### Onboarding
1. User enters Jira site URL, email, API token, and Gemini API key.
2. App validates both (Jira `myself` + Gemini probe).
3. Starts full historical sync of every project the token can access.

### Sync strategy
- **Initial:** Paginated issue search with changelogs expanded; also sync projects, boards/sprints, and field definitions (story points mapping).
- **Incremental:** Every 5–15 minutes and on demand; issues with `updated >= lastSync` watermark.
- **Resumable:** Per-project/page checkpoints so quitting mid-first-sync does not restart from zero.
- **Rate limits:** Bounded concurrency, backoff on HTTP 429, visible progress (projects, issues, ETA).
- **Reconciliation:** Periodic handling so issues removed from Jira do not linger indefinitely in analytics.

### SQLite layers

| Layer | Contents |
|-------|----------|
| Raw | projects, issues, changelog transitions, sprints, sprint↔issue, worklogs, issue links, comments (as needed), field definitions |
| Derived | time-in-status, cycle/lead time, throughput by period, sprint commitment vs completion, reopen/handoff/scope-change events, epic rollups, bottleneck scores |
| Meta | sync watermarks, story-points field mapping, settings |

**Custom fields:** Discover via Jira fields API; if story points are ambiguous, prompt the user once and persist the mapping.

---

## 6. Product surfaces

Global filters: date range, project(s), issue type, assignee/team (via available Jira metadata).

### Flow
- Cycle time, lead time, time-in-status distributions
- Bottleneck view (status consuming the most calendar time)
- Flow efficiency (active vs waiting — configurable status mapping)
- Throughput (completed issues per week)
- Reopens, handoffs (assignee changes), scope-change events from changelog

### Sprint
- Commitment vs delivered, spillover, velocity trend
- Mid-sprint scope added/removed
- Per-sprint issue breakdown

### Epics & delivery risk
- Progress (issues/points done vs remaining)
- **At-risk score** from: throughput vs remaining work, age of open issues, blocked time, recent scope growth, sprint spillover history
- **Finish-by probability** for a target date using historical throughput and remaining work; UI shows assumptions

### AI Q&A (Gemini)
- Chat over a context pack: active filters + precomputed metrics + top-N supporting issues
- Grounded answers with citations (issue keys / metric names); refuse when data is not yet synced
- Suggested prompts for bottlenecks, at-risk epics, capacity, velocity changes

### App shell navigation
Setup / credentials → Sync status → Home → Flow / Sprints / Epics / Explore / Ask AI

---

## 7. Data flow

1. Scheduler or user triggers sync → `jira` fetches pages → `sync` writes raw rows + advances checkpoints.
2. After raw batch commit → `analytics` / `risk` refresh derived tables for affected projects/date ranges.
3. UI queries derived (and sometimes raw) tables via Tauri commands.
4. Ask AI → `gemini` builds context pack from derived + top issues → Gemini API → response with citations rendered in UI.

---

## 8. Errors, security, packaging

### Errors & resilience
- Sync failures are non-fatal: retain last good data; banner + retry; never wipe derived tables on partial failure.
- Long first sync: progress UI, pause/resume, browse already-synced projects while sync continues.
- Jira `401/403`: stop sync and prompt for credential refresh.
- Gemini failures: chat errors only; dashboards remain usable offline from SQLite.
- Corrupt/inconsistent derived data: “rebuild derived from raw” vs “full re-sync” as separate actions.

### Security & privacy
- Secrets only in OS keychain; redacted from logs.
- Outbound network limited to Jira Cloud and Gemini (no product telemetry backend in v1).
- User can inspect the context pack sent for a given AI question.
- Per-machine DB; no multi-user server mode in v1.

### Packaging
- macOS: signed and notarized `.dmg` (or `.pkg`) via Tauri bundler.
- Auto-update optional later; v1 may use manual distribution of new builds.
- Windows/Linux installers deferred until macOS path is solid.

---

## 9. Testing strategy

### Test-first (repo-wide)
- Default workflow: **failing test → implementation → passing test → refactor**.
- No feature lands without tests for the behavior it introduces.
- Jira and Gemini HTTP interactions covered with recorded fixtures (VCR-style) in CI; no dependency on live APIs in automated runs.

### Unit (Rust)
- Changelog → transitions → time-in-status / cycle time (same-day moves, reopens, missing fields)
- Incremental watermarks and resume checkpoints
- At-risk and finish-by probability with fixed fixtures
- Context-pack builder token-budget limits

### Unit (UI)
- Setup validation, sync progress states, filter → chart wiring
- AI chat empty / error / cited-answer states

### Integration
- Sync pipeline against Jira fixtures
- Gemini client mocked; optional manual live smoke checklist

### Manual / release
- Fresh install → credentials → real-site sync sample → dashboards + one Gemini question
- Quit/resume mid-sync; offline dashboards; bad-token recovery
- Notarized `.dmg` on a clean Mac

### Out of scope for v1 CI
- Full live org-sized sync; pixel-level visual regression

---

## 10. Engineering standards

### Formatting & lint
- UI: **Prettier** for formatting and **ESLint** for linting; CI fails on drift.
- Rust: `rustfmt` + `clippy` (deny warnings in CI as appropriate).
- Pre-commit hooks optional but recommended so local matches CI.

### Documentation & modularity
- Public Rust APIs and non-obvious modules get doc comments; UI modules get short purpose notes where helpful.
- Single-purpose modules with clear boundaries (see §4).
- Prefer shared helpers over duplication; extract when a second use appears.
- Derived metrics implemented once in `analytics` / SQL — not reimplemented in the UI.
- If a file grows too large to reason about in one pass, split it in the same change.

### Maintainability bar
For each module, it must be easy to answer: what it does, how to use it, and what it depends on.

---

## 11. Success criteria (v1)

- User can install on macOS, enter Jira + Gemini credentials, and complete (or resume) a full-history sync of all accessible projects.
- Flow, sprint, and epic dashboards answer the core questions (bottlenecks, capacity, spillover, epic risk) from local data without live Jira queries.
- Finish-by / at-risk views show scores and explicit assumptions.
- Gemini Q&A returns grounded answers with citations against the current filter context.
- Dashboards work offline after sync; credential and API failures degrade gracefully.
- CI enforces tests, format, and lint; core metrics are developed test-first.

---

## 12. Future (explicitly later)

- Windows/Linux installers
- Optional local models (e.g. Ollama) as Gemini alternative
- GitHub/GitLab, CI/CD, incidents for DORA and richer engineering intelligence
- Auto-update channel
- Optional shared snapshot export/import (read-only viewers without Jira tokens)

---

## 13. Open decisions resolved in this spec

| Topic | Resolution |
|-------|------------|
| Sharing model | A — each user syncs with own token |
| Platform | B — macOS first, portable core |
| Scope | Full vision in v1 including AI + risk |
| AI | Cloud Gemini with user API key |
| Sync breadth | All projects token can access |
| History | Full historical initial sync |
| Stack | Tauri + Rust + SQLite + React UI |
