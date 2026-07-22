import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import FilterBar from '../components/FilterBar';
import MetricChart from '../components/MetricChart';
import { formatDuration, formatPercent } from '../lib/format';
import {
  emptyMetricsFilter,
  getFlowMetrics,
  type FlowMetrics,
  type MetricsFilter,
} from '../lib/tauri';

export default function FlowPage() {
  const [filter, setFilter] = useState<MetricsFilter>(emptyMetricsFilter);
  const [metrics, setMetrics] = useState<FlowMetrics | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setError(null);
    void getFlowMetrics(filter)
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
    <main className="page dashboard-page">
      <header className="dashboard-header">
        <h1>Flow</h1>
        <nav className="dashboard-nav">
          <Link to="/">Home</Link>
          <Link to="/flow" aria-current="page">
            Flow
          </Link>
          <Link to="/sprints">Sprints</Link>
          <Link to="/epics">Epics</Link>
        </nav>
      </header>

      <FilterBar value={filter} onChange={setFilter} />

      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}

      {!error && !metrics ? <p>Loading flow metrics…</p> : null}

      {metrics ? (
        <>
          <section className="summary-grid" aria-label="Flow percentiles">
            <article>
              <h2>Cycle p50</h2>
              <p>{formatDuration(metrics.cycle_p50_secs)}</p>
            </article>
            <article>
              <h2>Cycle p85</h2>
              <p>{formatDuration(metrics.cycle_p85_secs)}</p>
            </article>
            <article>
              <h2>Lead p50</h2>
              <p>{formatDuration(metrics.lead_p50_secs)}</p>
            </article>
            <article>
              <h2>Lead p85</h2>
              <p>{formatDuration(metrics.lead_p85_secs)}</p>
            </article>
            <article>
              <h2>Flow efficiency</h2>
              <p>{formatPercent(metrics.flow_efficiency)}</p>
            </article>
            <article>
              <h2>Reopens</h2>
              <p>{metrics.reopens}</p>
            </article>
            <article>
              <h2>Handoffs</h2>
              <p>{metrics.handoffs}</p>
            </article>
          </section>

          <MetricChart
            title="Throughput"
            bars={metrics.throughput.map((p) => ({
              label: p.day,
              value: p.completed_count,
            }))}
            valueLabel={(v) => `${v} done`}
          />

          <MetricChart
            title="Bottlenecks"
            bars={metrics.bottlenecks.map((b) => ({
              label: b.status,
              value: b.total_secs,
            }))}
            valueLabel={(v) => formatDuration(v)}
          />

          <section className="data-table" aria-label="Throughput table">
            <h2>Daily throughput</h2>
            <table>
              <thead>
                <tr>
                  <th scope="col">Day</th>
                  <th scope="col">Completed</th>
                </tr>
              </thead>
              <tbody>
                {metrics.throughput.length === 0 ? (
                  <tr>
                    <td colSpan={2}>No throughput data</td>
                  </tr>
                ) : (
                  metrics.throughput.map((p) => (
                    <tr key={p.day}>
                      <td>{p.day}</td>
                      <td>{p.completed_count}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </section>
        </>
      ) : null}
    </main>
  );
}
