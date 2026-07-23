import { useEffect, useState } from 'react';
import DashboardNav from '../components/DashboardNav';
import FilterBar from '../components/FilterBar';
import { formatPercent } from '../lib/format';
import {
  emptyMetricsFilter,
  getEpicRisk,
  getFinishBy,
  type EpicRisk,
  type FinishByResult,
  type MetricsFilter,
} from '../lib/tauri';

export default function EpicsPage() {
  const [filter, setFilter] = useState<MetricsFilter>(emptyMetricsFilter);
  const [epics, setEpics] = useState<EpicRisk[] | null>(null);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [targetDate, setTargetDate] = useState('');
  const [finishBy, setFinishBy] = useState<FinishByResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [finishError, setFinishError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setError(null);
    void getEpicRisk(filter)
      .then((data) => {
        if (!active) {
          return;
        }
        setEpics(data);
        setSelectedKey((prev) => {
          if (prev && data.some((e) => e.epic_key === prev)) {
            return prev;
          }
          return data[0]?.epic_key ?? null;
        });
      })
      .catch((err: unknown) => {
        if (active) {
          setEpics(null);
          setSelectedKey(null);
          setError(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      active = false;
    };
  }, [filter]);

  useEffect(() => {
    if (!selectedKey || !targetDate) {
      setFinishBy(null);
      setFinishError(null);
      return;
    }
    let active = true;
    setFinishError(null);
    void getFinishBy(selectedKey, targetDate)
      .then((data) => {
        if (active) {
          setFinishBy(data);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setFinishBy(null);
          setFinishError(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      active = false;
    };
  }, [selectedKey, targetDate]);

  const selected = epics?.find((e) => e.epic_key === selectedKey) ?? null;

  return (
    <main className="page dashboard-page">
      <header className="dashboard-header">
        <h1>Epics</h1>
        <DashboardNav current="epics" />
      </header>

      <FilterBar value={filter} onChange={setFilter} />

      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}

      {!error && !epics ? <p>Loading epic risk…</p> : null}

      {epics ? (
        <section className="data-table" aria-label="Epic risk">
          <h2>At-risk epics</h2>
          <table>
            <thead>
              <tr>
                <th scope="col">Epic</th>
                <th scope="col">Score</th>
                <th scope="col">Drivers</th>
              </tr>
            </thead>
            <tbody>
              {epics.length === 0 ? (
                <tr>
                  <td colSpan={3}>No epic risk data</td>
                </tr>
              ) : (
                epics.map((e) => (
                  <tr
                    key={e.epic_key}
                    className={e.epic_key === selectedKey ? 'row-selected' : undefined}
                  >
                    <td>
                      <button
                        type="button"
                        className="linkish"
                        onClick={() => setSelectedKey(e.epic_key)}
                      >
                        {e.epic_key}
                      </button>
                    </td>
                    <td>{e.score}</td>
                    <td>{e.drivers.join('; ') || '—'}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </section>
      ) : null}

      {selected ? (
        <section className="finish-by" aria-label="Finish-by forecast">
          <h2>Finish-by for {selected.epic_key}</h2>
          <p className="finish-by__meta">
            Risk score {selected.score}
            {selected.drivers.length > 0 ? ` — ${selected.drivers.join('; ')}` : ''}
          </p>

          <label htmlFor="finish-by-target">Target date</label>
          <input
            id="finish-by-target"
            name="targetDate"
            type="date"
            value={targetDate}
            onChange={(e) => setTargetDate(e.target.value)}
          />

          {finishError ? (
            <p className="form-error" role="alert">
              {finishError}
            </p>
          ) : null}

          {finishBy ? (
            <div className="finish-by__result">
              <p>Probability: {formatPercent(finishBy.probability)}</p>
              <div>
                <h3>Assumptions</h3>
                <ul>
                  {finishBy.assumptions.map((a) => (
                    <li key={a}>{a}</li>
                  ))}
                </ul>
              </div>
            </div>
          ) : null}
        </section>
      ) : null}
    </main>
  );
}
