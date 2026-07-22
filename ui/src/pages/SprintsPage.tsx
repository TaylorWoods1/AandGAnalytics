import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import FilterBar from '../components/FilterBar';
import {
  emptyMetricsFilter,
  getSprintMetrics,
  type MetricsFilter,
  type SprintMetrics,
} from '../lib/tauri';

function displayNum(value: number | null | undefined): string {
  return value == null ? '—' : String(value);
}

export default function SprintsPage() {
  const [filter, setFilter] = useState<MetricsFilter>(emptyMetricsFilter);
  const [sprints, setSprints] = useState<SprintMetrics[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setError(null);
    void getSprintMetrics(filter)
      .then((data) => {
        if (active) {
          setSprints(data);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setSprints(null);
          setError(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      active = false;
    };
  }, [filter]);

  return (
    <main className="page dashboard-page">
      <header className="dashboard-header">
        <h1>Sprints</h1>
        <nav className="dashboard-nav">
          <Link to="/">Home</Link>
          <Link to="/flow">Flow</Link>
          <Link to="/sprints" aria-current="page">
            Sprints
          </Link>
          <Link to="/epics">Epics</Link>
          <Link to="/explore">Explore</Link>
        </nav>
      </header>

      <FilterBar value={filter} onChange={setFilter} />

      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}

      {!error && !sprints ? <p>Loading sprint metrics…</p> : null}

      {sprints ? (
        <section className="data-table" aria-label="Sprint metrics">
          <h2>Sprint commitment</h2>
          <table>
            <thead>
              <tr>
                <th scope="col">Sprint</th>
                <th scope="col">Committed</th>
                <th scope="col">Completed</th>
                <th scope="col">Spillover</th>
                <th scope="col">Velocity</th>
              </tr>
            </thead>
            <tbody>
              {sprints.length === 0 ? (
                <tr>
                  <td colSpan={5}>No sprint metrics</td>
                </tr>
              ) : (
                sprints.map((s) => (
                  <tr key={s.sprint_id}>
                    <td>{s.name ?? s.sprint_id}</td>
                    <td>{displayNum(s.committed)}</td>
                    <td>{displayNum(s.completed)}</td>
                    <td>spillover: {displayNum(s.spillover)}</td>
                    <td>{displayNum(s.velocity_points)}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </section>
      ) : null}
    </main>
  );
}
