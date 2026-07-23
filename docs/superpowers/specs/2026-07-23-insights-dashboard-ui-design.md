# Insights Dashboard UI Pass — Design

**Date:** 2026-07-23  
**Status:** Approved for implementation planning  
**Product:** AandG Analytics (Tauri + React) local Jira engineering intelligence  
**Depends on:** `2026-07-23-jira-analytics-desktop-design.md`, `2026-07-23-performance-analytics-design.md`

---

## 1. Problem & goals

The current UI is filter- and table-heavy: free-text project/type/assignee inputs, CSS-only bar charts on Flow, and little visual insight on Home / Performance / Sprints / Epics. Assignees appear as opaque account IDs. Filters do not persist across pages.

**Goals**
- Deliver a full **insights dashboard** across Home, Flow, Performance, Sprints, and Epics: key stats + charts on every insights page.
- Replace free-text filters with **dropdowns** of all filterable criteria from synced SQLite data.
- Use **global shared filters** on every insights surface (including Explore and Ask AI); Sync/Settings remain filter-free.
- Chart stack: **Recharts** for most charts + **@nivo/heatmap** where Recharts is weak (person×month / dense grids).
- Add **WIP trend**, **~CFD** (status-category cumulative flow), and **sprint burndown**.
- Keep metrics computed in Rust/SQL; UI stays presentational.

**Non-goals**
- Redesigning Sync/Setup beyond sharing the nav shell.
- Chat history / markdown rendering overhaul for Ask AI.
- Perfect per-status CFDs with dozens of statuses (category-level first).
- URL-synced filter state (session persistence only).
- New visual brand (keep AG red/grey + IBM Plex).

---

## 2. Decisions locked

| Topic | Decision |
|-------|----------|
| Scope | Full pass: Home + Flow + Performance + Sprints + Epics (+ shared filters on Explore / Ask AI) |
| Filters | Global shared; one selection drives all insights pages |
| Chart libs | Recharts primary; `@nivo/heatmap` specialty |
| Home layout | **A — KPI rail + spotlight** |
| Explore / Ask AI | Same global filters; no chart redesign this pass |
| Flow extras | WIP trend + CFD (To Do / In Progress / Done) |
| Sprint extras | Burndown for selected / latest sprint |

---

## 3. Architecture

```
┌──────────────────────────────────────────────────────────┐
│  DashboardShell (insights routes only)                   │
│    DashboardNav                                          │
│    FilterBar  ←── FilterProvider (sessionStorage)        │
│    <Outlet /> page content                               │
└───────────────────────────┬──────────────────────────────┘
                            │ MetricsFilter
                            ▼
              Tauri commands (SQLite-backed)
   getFilterOptions | getFlowMetrics | getFlowHistory
   getSprintMetrics | getSprintBurndown | getEpicRisk
   getPerformanceMetrics | listIssues | askAi / previewContextPack
```

**Principles**
- Single `FilterProvider` owns `MetricsFilter` for the app session.
- `getFilterOptions()` is the only source of dropdown catalogs.
- New time-series (WIP/CFD/burndown) live in Rust analytics, not in the UI.
- Replace `MetricChart` CSS bars with Recharts/Nivo via shared `ChartCard`.

---

## 4. Shared filters

### Filter model (unchanged shape)
```ts
project_keys: string[] | null
from / to: string | null   // YYYY-MM-DD
issue_types: string[] | null
assignee_ids: string[] | null
```

**Defaults:** last 90 days; all projects / types / people (`null` = no restriction).

**Persistence:** `sessionStorage` key (e.g. `ag.filters.v1`). No URL sync in this pass.

### Catalog API — `getFilterOptions()`
Returns:
- **projects:** `{ key, name, issue_count? }[]` from `projects` (+ optional counts from `issues`)
- **issue_types:** `{ value, count? }[]` distinct `issues.issue_type`
- **assignees:** `{ id, label, count? }[]` distinct `assignee_account_id` with best-effort **display label** from changelog `from_string` / `to_string` or issue `raw_json`; fallback to shortened account id
- Empty catalog → UI prompts user to sync

### FilterBar UX
- Searchable multi-selects: Projects, Issue types, Assignees (Select all / Clear)
- Date range: From / To + presets (Last 7 / 30 / 90 days, This quarter)
- Active-filter chips + Clear all
- Sticky under nav inside `DashboardShell`

### Shell routing
Insights (with shell + filters): `/`, `/flow`, `/performance`, `/sprints`, `/epics`, `/explore`, `/ask`  
Without filters: `/sync`, `/setup`, `/settings`

---

## 5. Chart kit & layout

### Components
| Component | Role |
|-----------|------|
| `KpiStrip` | 3–5 KPI cards (label, value, optional delta) |
| `ChartCard` | Title, loading/empty/error, wraps Recharts or Nivo |
| `FilterBar` | Global dropdown filters |
| `PersonLabel` | Resolve account id → catalog label |
| `DashboardShell` | Nav + FilterBar + outlet |

### Libraries
- `recharts` — line, area, bar, composed, scatter
- `@nivo/heatmap` — person×month (and similar dense grids)

### Layout
- Widen dashboard max-width from ~56rem to **~72–80rem**
- Preserve existing CSS variables / IBM Plex; denser dashboard grid for KPI + chart rows
- No purple/dark-mode theme change

---

## 6. Page purposes & visuals

### Home — executive pulse (layout A)
**KPIs:** Cycle p50 · Throughput (period) · Latest sprint velocity · Epics at-risk count (optional small WIP spark)

**Charts / panels:**
- Throughput area (Recharts)
- Top at-risk epics ranked list (deep-link Epics)
- MoM movers bars (display names)
- Bottleneck horizontal bars (deep-link Flow)

**Data:** `getFlowMetrics`, `getPerformanceMetrics`, `getEpicRisk`, `getSprintMetrics` (latest velocity); optional WIP from `getFlowHistory`

### Flow — how work moves
**KPIs:** Cycle p50/p85 · Lead p50/p85 · Flow efficiency · Reopens · Handoffs · current WIP

**Charts:**
- Throughput over time (area/line)
- **CFD** stacked area by status category (To Do / In Progress / Done)
- **WIP** trend (line)
- Bottlenecks by status (bar)
- Cycle vs lead comparison (composed bar)
- Daily throughput table demoted to secondary detail

**Data:** `getFlowMetrics` + **`getFlowHistory(filter)`** (new)

### Performance — who / where delivery happens
**KPIs:** Completions · Points · Open issues · Blocked time

**Charts:**
- Top people by completions (bar, display names)
- Top projects (bar)
- Person×month heatmap (Nivo)
- Project completions over months (multi-line)
- Existing tables remain as drill-down below charts

**Data:** `getPerformanceMetrics` (existing)

### Sprints — commitment vs delivery
**KPIs:** Avg completion % · Avg spillover · Latest velocity · Scope churn (added − removed)

**Charts:**
- Commitment vs completed (grouped bar)
- Velocity trend (line)
- Scope added/removed (stacked bar — surface unused API fields)
- **Burndown** for selected / latest sprint (remaining issues + points if mapped; ideal guideline)

**Table:** per-sprint breakdown with % complete; sprint picker for burndown

**Data:** `getSprintMetrics` + **`getSprintBurndown(sprintId)`** (new)

### Epics — delivery risk
**KPIs:** At-risk count · Avg risk score · Avg finish-by probability

**Charts:**
- Risk score distribution (bar)
- Finish-by probability vs score (scatter)
- Selected epic detail (drivers + assumptions + finish-by date probe)

**Table:** epic list with score, list-level `finish_by_probability`, top drivers

**Data:** `getEpicRisk`, `getFinishBy` (existing; show previously unused list fields)

### Explore / Ask AI
- Same sticky FilterBar and shared state
- Explore: display-name assignees; keep paginated table (no chart pass)
- Ask AI: filters feed existing context pack / ask commands

### Sync / Settings
- Unchanged purpose; no FilterBar

---

## 7. New backend series

### `getFlowHistory(filter) -> FlowHistory`
Daily points over `from`…`to` (respect project / type / assignee filters):

```ts
{
  days: string[]  // YYYY-MM-DD
  wip: number[]   // open / non-done count at end of day
  cfd: {
    todo: number[]
    in_progress: number[]
    done: number[]
  }
}
```

**Construction:** Reconstruct status category per issue over time from `issue_changelog` status transitions + current `issues` snapshot. Use existing `resolve_status_category` / overrides (same as flow efficiency). Prefer derived daily tables refreshed with analytics rebuild if query cost is high; otherwise compute on read with tests locking the algorithm.

**CFD semantics:** Category-level stacks (not every Jira status). For each day end, count issues whose resolved status category that day is To Do, In Progress, or Done (classic CFD snapshot stacks — not a cumulative-completions-only series). WIP for that day = To Do + In Progress (non-done).

### `getSprintBurndown(sprintId) -> SprintBurndown`
```ts
{
  sprint_id: string
  name: string | null
  days: string[]
  remaining_issues: number[]
  remaining_points: (number | null)[]
  ideal_remaining_issues: number[]
}
```

**Construction:** Sprint start/end from `sprints`; membership from `sprint_issues`; remaining each day = committed scope still not completed by that day (completions from `derived_issue_cycle` / `derived_completions`). Ideal line = linear from committed count on start date to 0 on end date.

**UI default:** Latest sprint with both start and end dates; user can pick another from sprint metrics list.

---

## 8. Data flow

1. Credentials gate → insights routes mount `DashboardShell` + `FilterProvider`.
2. On shell mount (and after sync completes): load `getFilterOptions()`.
3. Filter changes update context → pages refetch metrics with the same `MetricsFilter`.
4. Charts render from command results; empty series show “No data for current filters”.
5. Failures: page-level error + retry; filter catalog empty → CTA to Sync.

People labels: any UI showing an account id must use the assignee catalog map from `getFilterOptions`.

---

## 9. Errors, empty states, performance

| Case | Behavior |
|------|----------|
| No synced data / empty options | FilterBar disabled messaging + link to Sync |
| Metric command error | Inline error on page, Retry |
| Zero chart series | ChartCard empty message |
| Long CFD/WIP range | Cap or downsample daily points if needed (e.g. >180 days → weekly buckets) — document in implementation |
| Burndown missing dates | Show explanation; fall back to table-only sprint view |

---

## 10. Testing

### Rust
- `getFilterOptions`: projects, types, assignee labels from changelog strings / fallbacks
- Flow history: fixture changelog → expected WIP/CFD daily series
- Sprint burndown: fixture sprint + completions → remaining + ideal line

### UI
- `FilterProvider`: update + sessionStorage round-trip
- `FilterBar`: selecting options updates shared filter (no free-text project/type/assignee fields)
- Page smoke: KPI strip + chart containers render with mocked metrics (Home, Flow, Performance, Sprints, Epics)
- Person labels appear instead of raw ids where catalog has labels

### Manual
- Change filters on Home → navigate to Flow/Epics → same selection
- Confirm CFD/WIP/burndown with a real synced project sample

---

## 11. Success criteria

- Free-text project/type/assignee filters are gone; dropdowns show synced criteria with people labels.
- One global filter selection applies on Home, Flow, Performance, Sprints, Epics, Explore, Ask AI.
- Home uses KPI rail + spotlight with charts and deep links.
- Flow shows throughput, CFD, WIP, bottlenecks, and cycle/lead KPIs.
- Performance shows chart-first people/project views + heatmap.
- Sprints show commitment/velocity/scope charts + burndown; scope added/removed visible.
- Epics show risk visuals and list-level finish-by probability.
- Metrics remain Rust-backed; Recharts + Nivo heatmap only in UI.
- CI tests cover filter options, flow history, burndown, and filter/chart smoke UI.

---

## 12. Implementation order (for planning)

1. `getFilterOptions` + FilterProvider + FilterBar dropdowns + shell wiring  
2. Chart kit (`KpiStrip`, `ChartCard`) + layout width  
3. Home redesign (layout A)  
4. Flow redesign + `getFlowHistory` (WIP/CFD)  
5. Performance redesign (Recharts + Nivo)  
6. Sprints redesign + `getSprintBurndown`  
7. Epics redesign (surface unused fields)  
8. Explore/Ask AI filter shell + person labels  
9. Tests + manual QA pass  

---

## 13. Open items resolved in this spec

| Topic | Resolution |
|-------|------------|
| Approach | Shell + page modules (global filters, catalog API, chart kit) |
| Specialty lib | `@nivo/heatmap` |
| CFD granularity | Status category (To Do / In Progress / Done) |
| Burndown | Selected/latest sprint; issues + points; ideal line |
| Filter persistence | sessionStorage; no URL sync |
| Ask AI / Explore charts | Filters only this pass |
