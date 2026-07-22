export type ChartBar = {
  label: string;
  value: number;
};

type Props = {
  title: string;
  bars: ChartBar[];
  valueLabel?: (value: number) => string;
  emptyMessage?: string;
};

export default function MetricChart({
  title,
  bars,
  valueLabel = (v) => String(v),
  emptyMessage = 'No data',
}: Props) {
  const max = Math.max(...bars.map((b) => b.value), 0);

  return (
    <section className="metric-chart" aria-label={title}>
      <h2>{title}</h2>
      {bars.length === 0 ? (
        <p className="metric-chart__empty">{emptyMessage}</p>
      ) : (
        <ul className="metric-chart__list">
          {bars.map((bar) => {
            const widthPct = max > 0 ? (bar.value / max) * 100 : 0;
            return (
              <li key={bar.label} className="metric-chart__row">
                <span className="metric-chart__label">{bar.label}</span>
                <div className="metric-chart__track" aria-hidden="true">
                  <div className="metric-chart__fill" style={{ width: `${widthPct}%` }} />
                </div>
                <span className="metric-chart__value">{valueLabel(bar.value)}</span>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
