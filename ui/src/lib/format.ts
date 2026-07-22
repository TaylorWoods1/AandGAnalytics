/** Display-only formatting for seconds from the metrics payload. */
export function formatDuration(secs: number | null | undefined): string {
  if (secs == null || Number.isNaN(secs)) {
    return '—';
  }
  const abs = Math.abs(secs);
  if (abs < 60) {
    return `${Math.round(secs)}s`;
  }
  if (abs < 3600) {
    return `${(secs / 60).toFixed(1)}m`;
  }
  if (abs < 86400) {
    return `${(secs / 3600).toFixed(1)}h`;
  }
  return `${(secs / 86400).toFixed(1)}d`;
}

export function formatPercent(ratio: number | null | undefined): string {
  if (ratio == null || Number.isNaN(ratio)) {
    return '—';
  }
  return `${(ratio * 100).toFixed(0)}%`;
}
