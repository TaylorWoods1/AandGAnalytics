import { useEffect, useState } from 'react';
import DashboardNav from '../components/DashboardNav';
import FilterBar from '../components/FilterBar';
import { formatDuration, formatPercent } from '../lib/format';
import {
  emptyMetricsFilter,
  getPerformanceMetrics,
  type MetricsFilter,
  type PerformanceMetrics,
} from '../lib/tauri';

/** Truncate account id for display; full id stays in title tooltip. */
export function shortAccountId(id: string): string {
  if (id.length <= 12) {
    return id;
  }
  return `${id.slice(0, 6)}…${id.slice(-4)}`;
}

export default function PerformancePage() {
  const [filter, setFilter] = useState<MetricsFilter>(emptyMetricsFilter);
  const [metrics, setMetrics] = useState<PerformanceMetrics | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setError(null);
    void getPerformanceMetrics(filter)
      .then((data) => {
        if (active) {
          setMetrics(data);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setMetrics(null);
          setError(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      active = false;
    };
  }, [filter]);

  return (
    <main className="page dashboard-page performance-page">
      <header className="dashboard-header">
        <h1>Performance</h1>
        <DashboardNav current="performance" />
      </header>

      <p className="page-lede">
        Completions by person and project, blockers, and month-over-month rates.
      </p>

      <FilterBar value={filter} onChange={setFilter} />

      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}

      {!error && !metrics ? <p>Loading performance…</p> : null}

      {metrics ? (
        <>
          <section className="perf-section" aria-labelledby="people-velocity-heading">
            <h2 id="people-velocity-heading">People velocity</h2>
            <p className="section-note">Completed tickets attributed to the finisher.</p>
            {metrics.by_person.length === 0 ? (
              <p>No completions in range.</p>
            ) : (
              <table className="perf-table">
                <thead>
                  <tr>
                    <th scope="col">Person</th>
                    <th scope="col">Tickets</th>
                    <th scope="col">Points</th>
                  </tr>
                </thead>
                <tbody>
                  {metrics.by_person.map((row) => (
                    <tr key={row.account_id}>
                      <td title={row.account_id}>{shortAccountId(row.account_id)}</td>
                      <td>{row.completed_count}</td>
                      <td>{row.points != null ? row.points.toFixed(1) : '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          <section className="perf-section" aria-labelledby="project-breakdown-heading">
            <h2 id="project-breakdown-heading">Project breakdown</h2>
            <p className="section-note">Open work, completions in range, and current blockers.</p>
            {metrics.by_project.length === 0 ? (
              <p>No project data.</p>
            ) : (
              <table className="perf-table">
                <thead>
                  <tr>
                    <th scope="col">Project</th>
                    <th scope="col">Open</th>
                    <th scope="col">Completed</th>
                    <th scope="col">Blockers</th>
                    <th scope="col">Blocked time</th>
                  </tr>
                </thead>
                <tbody>
                  {metrics.by_project.map((row) => (
                    <tr key={row.project_key}>
                      <td>{row.project_key}</td>
                      <td>{row.open_count}</td>
                      <td>{row.completed_in_range}</td>
                      <td>{row.blocker_count}</td>
                      <td>{formatDuration(row.blocked_secs)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          <section className="perf-section" aria-labelledby="person-month-heading">
            <h2 id="person-month-heading">Tickets per person per month</h2>
            <p className="section-note">Rate change vs previous calendar month for the same person.</p>
            {metrics.person_month.length === 0 ? (
              <p>No monthly person data.</p>
            ) : (
              <table className="perf-table">
                <thead>
                  <tr>
                    <th scope="col">Month</th>
                    <th scope="col">Person</th>
                    <th scope="col">Tickets</th>
                    <th scope="col">Points</th>
                    <th scope="col">Rate change</th>
                  </tr>
                </thead>
                <tbody>
                  {metrics.person_month.map((row) => (
                    <tr key={`${row.month}-${row.account_id}`}>
                      <td>{row.month}</td>
                      <td title={row.account_id}>{shortAccountId(row.account_id)}</td>
                      <td>{row.completed_count}</td>
                      <td>{row.points != null ? row.points.toFixed(1) : '—'}</td>
                      <td>
                        {row.rate_change == null ? (
                          '—'
                        ) : (
                          <span
                            className={
                              row.rate_change >= 0 ? 'rate-up' : 'rate-down'
                            }
                          >
                            {formatPercent(row.rate_change)}
                          </span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          <section className="perf-section" aria-labelledby="project-month-heading">
            <h2 id="project-month-heading">Completions by project month</h2>
            {metrics.project_month.length === 0 ? (
              <p>No monthly project data.</p>
            ) : (
              <table className="perf-table">
                <thead>
                  <tr>
                    <th scope="col">Month</th>
                    <th scope="col">Project</th>
                    <th scope="col">Tickets</th>
                  </tr>
                </thead>
                <tbody>
                  {metrics.project_month.map((row) => (
                    <tr key={`${row.month}-${row.project_key}`}>
                      <td>{row.month}</td>
                      <td>{row.project_key}</td>
                      <td>{row.completed_count}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>
        </>
      ) : null}
    </main>
  );
}
