import { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import DashboardNav from '../components/DashboardNav';
import FilterBar from '../components/FilterBar';
import { formatDuration, formatPercent } from '../lib/format';
import {
  emptyMetricsFilter,
  getFlowMetrics,
  getPerformanceMetrics,
  type FlowMetrics,
  type MetricsFilter,
  type PerformanceMetrics,
} from '../lib/tauri';
import { shortAccountId } from './PerformancePage';

const TOP_N = 5;

export default function HomePage() {
  const [filter, setFilter] = useState<MetricsFilter>(emptyMetricsFilter);
  const [perf, setPerf] = useState<PerformanceMetrics | null>(null);
  const [flow, setFlow] = useState<FlowMetrics | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setError(null);
    void Promise.all([getPerformanceMetrics(filter), getFlowMetrics(filter)])
      .then(([performance, flowMetrics]) => {
        if (!active) {
          return;
        }
        setPerf(performance);
        setFlow(flowMetrics);
      })
      .catch((err: unknown) => {
        if (active) {
          setPerf(null);
          setFlow(null);
          setError(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      active = false;
    };
  }, [filter]);

  const topMovers = useMemo(() => {
    if (!perf) {
      return [];
    }
    return [...perf.person_month]
      .filter((r) => r.rate_change != null)
      .sort((a, b) => Math.abs(b.rate_change!) - Math.abs(a.rate_change!))
      .slice(0, TOP_N);
  }, [perf]);

  const topProjects = useMemo(() => {
    if (!perf) {
      return [];
    }
    return [...perf.by_project]
      .sort((a, b) => b.completed_in_range - a.completed_in_range)
      .slice(0, TOP_N);
  }, [perf]);

  const blockerHotspots = useMemo(() => {
    if (!perf) {
      return [];
    }
    return [...perf.by_project]
      .filter((p) => p.blocker_count > 0)
      .sort((a, b) => b.blocker_count - a.blocker_count)
      .slice(0, TOP_N);
  }, [perf]);

  const loading = !error && !perf;

  return (
    <main className="page dashboard-page home-page">
      <header className="dashboard-header">
        <h1>Jira Analytics</h1>
        <DashboardNav current="home" />
      </header>

      <p className="page-lede">
        Executive view of throughput, movers, and blocker hotspots. Dive deeper on{' '}
        <Link to="/performance">Performance</Link> or <Link to="/flow">Flow</Link>.
      </p>

      <FilterBar value={filter} onChange={setFilter} />

      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}

      {loading ? <p>Loading summary…</p> : null}

      {perf ? (
        <div className="home-exec">
          <div className="home-exec__strip" aria-label="Quick links and flow snapshot">
            <Link to="/performance">Open Performance</Link>
            <Link to="/flow">Open Flow</Link>
            {flow ? (
              <p className="home-exec__meta">
                Cycle p50 {formatDuration(flow.cycle_p50_secs)} · Throughput{' '}
                {flow.throughput.reduce((s, p) => s + p.completed_count, 0)} tickets
              </p>
            ) : null}
          </div>

          <section aria-labelledby="top-movers-heading">
            <h2 id="top-movers-heading">Top movers</h2>
            <p className="section-note">Largest month-over-month completion rate changes.</p>
            {topMovers.length === 0 ? (
              <p>Need at least two months of completions to show rate change.</p>
            ) : (
              <ul className="home-exec__list">
                {topMovers.map((row) => (
                  <li key={`${row.month}-${row.account_id}`}>
                    <span title={row.account_id}>{shortAccountId(row.account_id)}</span>
                    <span className="muted">{row.month}</span>
                    <span
                      className={
                        (row.rate_change ?? 0) >= 0 ? 'rate-up' : 'rate-down'
                      }
                    >
                      {formatPercent(row.rate_change)}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section aria-labelledby="top-projects-heading">
            <h2 id="top-projects-heading">Top projects by completions</h2>
            <p className="section-note">Tickets completed in the selected range.</p>
            {topProjects.length === 0 ? (
              <p>No completions in range.</p>
            ) : (
              <ul className="home-exec__list">
                {topProjects.map((row) => (
                  <li key={row.project_key}>
                    <span>{row.project_key}</span>
                    <span className="muted">{row.open_count} open</span>
                    <span>{row.completed_in_range}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section aria-labelledby="blocker-hotspots-heading">
            <h2 id="blocker-hotspots-heading">Blocker hotspots</h2>
            <p className="section-note">Projects with issues currently in a blocked/impeded status.</p>
            {blockerHotspots.length === 0 ? (
              <p>No blockers in scope.</p>
            ) : (
              <ul className="home-exec__list">
                {blockerHotspots.map((row) => (
                  <li key={row.project_key}>
                    <span>{row.project_key}</span>
                    <span className="muted">{formatDuration(row.blocked_secs)} blocked</span>
                    <span className="rate-up">{row.blocker_count}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
      ) : null}
    </main>
  );
}
