# Performance Analytics Design (2026-07-23)

## Goal

Surface developer and project throughput, blockers, and tickets-per-person-per-month (with rate change) on a new **Performance** page, with **Home** as an executive summary. Flow / Sprints / Epics remain deep-dive delivery views.

## Attribution (hybrid finisher)

For each issue with `derived_issue_cycle.completed_at`:

1. Load assignee changelog entries for that issue.
2. Take the last assignee `to` value at or before `completed_at` → attribution `changelog`.
3. If none, use `issues.assignee_account_id` → attribution `current`.
4. Unassigned finishers are omitted from person rollups.

**Caveat:** Changelog assignee fields store Jira account ids when available; display names are not synced in this release. The UI shows a truncated account id with the full id in a tooltip.

## Derived tables

| Table | Contents |
|-------|----------|
| `derived_completions` | One row per completed issue: project, `completed_at`, finisher, points, attribution |
| `derived_person_month` | UTC `YYYY-MM` × finisher completed counts / points |

Open ticket and blocker counts are **not** materialized; they are queried live from `issues` / `derived_time_in_status` so snapshots stay current.

Rebuild via existing `rebuild_all_derived` (Sync → rebuild derived / after sync).

## Metric definitions

| Metric | Definition |
|--------|------------|
| Completions | Issues with `completed_at` in the filter date range |
| Velocity / throughput | Count of completions (story points secondary) |
| Tickets per project | Open (not done / unresolved) + completions in range |
| Blockers per project | Current `status` matching `%block%` / `%imped%` (case-insensitive) |
| Blocked time | Sum of `derived_time_in_status` for those status names |
| Tickets/person/month | Completions by finisher, bucketed by UTC month of `completed_at` |
| Rate change | `(this_month - prev_month) / prev_month` when prev &gt; 0; else null |

Primary unit is **completed tickets**. Filters reuse `MetricsFilter` (`project_keys`, `from`, `to`, `issue_types`, `assignee_ids`). Date bounds apply to `completed_at` for completion queries; open/blocker snapshots ignore date bounds but respect project / type / assignee.

## UI

- **Home** — top movers (by \|rate change\|), top projects by completions, blocker hotspots; one-line Flow snapshot; links to Performance and Flow.
- **Performance** (`/performance`) — people velocity, project breakdown, person×month with rate change, project×month.
- Ask AI context pack includes top finisher / project completion lines when `derived_completions` is populated.

## Out of scope

- Jira display-name / avatar sync
- Formal `blocks` issue-link type
- Full redesign of Flow / Sprints / Epics / Explore
- Story-point-only velocity as the primary metric
