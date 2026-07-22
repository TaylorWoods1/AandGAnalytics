import type { SyncProgress } from '../lib/tauri';

type SyncBannerProps = {
  progress: SyncProgress | null;
  error?: string | null;
  onRetry?: () => void;
};

export default function SyncBanner({ progress, error, onRetry }: SyncBannerProps) {
  if (error) {
    return (
      <div role="alert" className="sync-banner sync-banner--error">
        <p>{error}</p>
        {onRetry ? (
          <button type="button" onClick={onRetry}>
            Retry
          </button>
        ) : null}
      </div>
    );
  }

  if (!progress || progress.phase === 'Idle') {
    return null;
  }

  return (
    <div role="status" className="sync-banner">
      Syncing ({progress.phase}): {progress.issues_synced} issues — {progress.message}
    </div>
  );
}
