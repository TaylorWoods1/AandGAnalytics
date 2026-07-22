import { Link } from 'react-router-dom';
import {
  CREDENTIAL_BANNER_COPY,
  isCredentialError,
  isOfflineError,
  OFFLINE_BANNER_COPY,
} from '../lib/syncErrors';
import type { SyncProgress } from '../lib/tauri';

type SyncBannerProps = {
  progress: SyncProgress | null;
  error?: string | null;
  onRetry?: () => void;
};

export default function SyncBanner({ progress, error, onRetry }: SyncBannerProps) {
  if (error) {
    const credential = isCredentialError(error);
    const offline = !credential && isOfflineError(error);
    return (
      <div role="alert" className="sync-banner sync-banner--error">
        {credential ? <p>{CREDENTIAL_BANNER_COPY}</p> : null}
        {offline ? <p>{OFFLINE_BANNER_COPY}</p> : null}
        {!credential && !offline ? <p>{error}</p> : <p className="sync-banner__detail">{error}</p>}
        {credential ? (
          <p>
            <Link to="/setup">Refresh credentials</Link>
          </p>
        ) : null}
        {onRetry && !credential ? (
          <button type="button" onClick={onRetry}>
            Retry
          </button>
        ) : null}
        {onRetry && credential ? (
          <button type="button" onClick={onRetry}>
            Retry sync
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
