import type { MetricsFilter } from '../lib/tauri';

type Props = {
  value: MetricsFilter;
  onChange: (next: MetricsFilter) => void;
};

function parseList(raw: string): string[] | null {
  const items = raw
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
  return items.length > 0 ? items : null;
}

export default function FilterBar({ value, onChange }: Props) {
  return (
    <form className="filter-bar" onSubmit={(e) => e.preventDefault()}>
      <label htmlFor="filter-projects">Projects</label>
      <input
        id="filter-projects"
        name="projectKeys"
        type="text"
        placeholder="KEY1, KEY2"
        value={value.project_keys?.join(', ') ?? ''}
        onChange={(e) => onChange({ ...value, project_keys: parseList(e.target.value) })}
      />

      <label htmlFor="filter-from">From</label>
      <input
        id="filter-from"
        name="from"
        type="date"
        value={value.from ?? ''}
        onChange={(e) => onChange({ ...value, from: e.target.value || null })}
      />

      <label htmlFor="filter-to">To</label>
      <input
        id="filter-to"
        name="to"
        type="date"
        value={value.to ?? ''}
        onChange={(e) => onChange({ ...value, to: e.target.value || null })}
      />

      <label htmlFor="filter-issue-types">Issue types</label>
      <input
        id="filter-issue-types"
        name="issueTypes"
        type="text"
        placeholder="Story, Bug"
        value={value.issue_types?.join(', ') ?? ''}
        onChange={(e) => onChange({ ...value, issue_types: parseList(e.target.value) })}
      />

      <label htmlFor="filter-assignees">Assignees</label>
      <input
        id="filter-assignees"
        name="assigneeIds"
        type="text"
        placeholder="account ids"
        value={value.assignee_ids?.join(', ') ?? ''}
        onChange={(e) => onChange({ ...value, assignee_ids: parseList(e.target.value) })}
      />
    </form>
  );
}
