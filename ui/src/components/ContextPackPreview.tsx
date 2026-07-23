import type { ContextPack } from '../lib/tauri';

type Props = {
  pack: ContextPack | null;
  loading?: boolean;
  error?: string | null;
};

/** Collapsible preview of the context pack that leaves the machine for Bedrock. */
export default function ContextPackPreview({ pack, loading = false, error = null }: Props) {
  return (
    <details className="context-pack-preview">
      <summary>Context pack preview</summary>
      {loading ? <p>Loading context pack…</p> : null}
      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}
      {pack ? (
        <div className="context-pack-preview__body">
          <p className="context-pack-preview__meta">
            Approx. tokens: {pack.approx_tokens} · Supporting issues:{' '}
            {pack.supporting_issues.length}
          </p>
          <h3>Filters</h3>
          <pre>{pack.filter_summary}</pre>
          <h3>Metrics</h3>
          <pre>{pack.metrics_markdown}</pre>
          <h3>Supporting issues</h3>
          {pack.supporting_issues.length === 0 ? (
            <p>(none)</p>
          ) : (
            <ul>
              {pack.supporting_issues.map((issue) => (
                <li key={issue.key}>
                  <strong>{issue.key}</strong>
                  {issue.summary ? ` — ${issue.summary}` : ''}
                  {issue.status ? ` (${issue.status})` : ''}
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : null}
      {!loading && !error && !pack ? <p>No context pack loaded yet.</p> : null}
    </details>
  );
}
