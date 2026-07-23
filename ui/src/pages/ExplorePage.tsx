import { useEffect, useState } from 'react';
import DashboardNav from '../components/DashboardNav';
import FilterBar from '../components/FilterBar';
import { formatDuration } from '../lib/format';
import { emptyMetricsFilter, listIssues, type IssuePage, type MetricsFilter } from '../lib/tauri';

const PAGE_SIZE = 25;

function displayText(value: string | null | undefined): string {
  return value == null || value === '' ? '—' : value;
}

function displayPoints(value: number | null | undefined): string {
  return value == null ? '—' : String(value);
}

export default function ExplorePage() {
  const [filter, setFilter] = useState<MetricsFilter>(emptyMetricsFilter);
  const [offset, setOffset] = useState(0);
  const [page, setPage] = useState<IssuePage | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setError(null);
    void listIssues(filter, { offset, limit: PAGE_SIZE })
      .then((data) => {
        if (active) {
          setPage(data);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setPage(null);
          setError(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      active = false;
    };
  }, [filter, offset]);

  const total = page?.total ?? 0;
  const canPrev = offset > 0;
  const canNext = offset + PAGE_SIZE < total;

  return (
    <main className="page dashboard-page">
      <header className="dashboard-header">
        <h1>Explore</h1>
        <DashboardNav current="explore" />
      </header>

      <FilterBar
        value={filter}
        onChange={(next) => {
          setFilter(next);
          setOffset(0);
        }}
      />

      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}

      {!error && !page ? <p>Loading issues…</p> : null}

      {page ? (
        <section className="data-table" aria-label="Issues">
          <h2>Issues</h2>
          <p>Total: {page.total}</p>
          <table>
            <thead>
              <tr>
                <th scope="col">Key</th>
                <th scope="col">Summary</th>
                <th scope="col">Project</th>
                <th scope="col">Status</th>
                <th scope="col">Assignee</th>
                <th scope="col">Points</th>
                <th scope="col">Cycle time</th>
                <th scope="col">Updated</th>
              </tr>
            </thead>
            <tbody>
              {page.items.length === 0 ? (
                <tr>
                  <td colSpan={8}>No issues</td>
                </tr>
              ) : (
                page.items.map((row) => (
                  <tr key={row.key}>
                    <td>{row.key}</td>
                    <td>{displayText(row.summary)}</td>
                    <td>{row.project_key}</td>
                    <td>{displayText(row.status)}</td>
                    <td>{displayText(row.assignee)}</td>
                    <td>{displayPoints(row.story_points)}</td>
                    <td>{formatDuration(row.cycle_secs)}</td>
                    <td>{row.updated}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>

          <div className="pagination">
            <button
              type="button"
              disabled={!canPrev}
              onClick={() => setOffset((o) => Math.max(0, o - PAGE_SIZE))}
            >
              Previous
            </button>
            <span>
              {total === 0 ? 0 : offset + 1}–{Math.min(offset + PAGE_SIZE, total)} of {total}
            </span>
            <button
              type="button"
              disabled={!canNext}
              onClick={() => setOffset((o) => o + PAGE_SIZE)}
            >
              Next
            </button>
          </div>
        </section>
      ) : null}
    </main>
  );
}
