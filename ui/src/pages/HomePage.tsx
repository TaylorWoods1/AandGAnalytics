import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import FilterBar from '../components/FilterBar';
import { formatDuration } from '../lib/format';
import {
  emptyMetricsFilter,
  getEpicRisk,
  getFlowMetrics,
  type FlowMetrics,
  type MetricsFilter,
} from '../lib/tauri';

/** Presentation threshold for the at-risk summary card (score from DTO). */
const AT_RISK_SCORE = 50;

export default function HomePage() {
  const [filter, setFilter] = useState<MetricsFilter>(emptyMetricsFilter);
  const [metrics, setMetrics] = useState<FlowMetrics | null>(null);
  const [atRiskCount, setAtRiskCount] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setError(null);
    void Promise.all([getFlowMetrics(filter), getEpicRisk(filter)])
      .then(([flow, epics]) => {
        if (!active) {
          return;
        }
        setMetrics(flow);
        setAtRiskCount(epics.filter((e) => e.score >= AT_RISK_SCORE).length);
      })
      .catch((err: unknown) => {
        if (active) {
          setMetrics(null);
          setAtRiskCount(null);
          setError(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      active = false;
    };
  }, [filter]);

  const throughputTotal =
    metrics?.throughput.reduce((sum, p) => sum + p.completed_count, 0) ?? null;
  const topBottleneck = metrics?.bottlenecks[0]?.status ?? '—';

  return (
    <main className="page dashboard-page">
      <header className="dashboard-header">
        <h1>AandG Analytics</h1>
        <nav className="dashboard-nav">
          <Link to="/" aria-current="page">
            Home
          </Link>
          <Link to="/flow">Flow</Link>
          <Link to="/sprints">Sprints</Link>
          <Link to="/epics">Epics</Link>
          <Link to="/explore">Explore</Link>
          <Link to="/ask">Ask AI</Link>
        </nav>
      </header>

      <p>Delivery health summary from synced Jira metrics.</p>

      <FilterBar value={filter} onChange={setFilter} />

      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}

      {!error && !metrics ? <p>Loading summary…</p> : null}

      {metrics ? (
        <section className="summary-grid" aria-label="Home summary">
          <article>
            <h2>Throughput</h2>
            <p>{throughputTotal ?? '—'}</p>
          </article>
          <article>
            <h2>Median cycle time</h2>
            <p>{formatDuration(metrics.cycle_p50_secs)}</p>
          </article>
          <article>
            <h2>Top bottleneck</h2>
            <p>{topBottleneck}</p>
          </article>
          <article>
            <h2>At-risk epics</h2>
            <p>{atRiskCount ?? '—'}</p>
          </article>
        </section>
      ) : null}
    </main>
  );
}
