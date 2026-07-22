import { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import SyncBanner from '../components/SyncBanner';
import {
  getSyncProgress,
  startFullSync,
  subscribeSyncProgress,
  type SyncProgress,
} from '../lib/tauri';

function etaText(progress: SyncProgress | null): string {
  if (!progress) {
    return 'Estimating remaining time…';
  }
  if (progress.phase === 'Failed') {
    return 'Sync stopped.';
  }
  if (progress.phase === 'Idle') {
    return progress.message || 'Sync complete.';
  }
  // Progress DTO has no dedicated ETA field; surface message as status/ETA line.
  if (progress.message.toLowerCase().includes('eta')) {
    return progress.message;
  }
  return progress.message ? `Status: ${progress.message}` : 'Estimating remaining time…';
}

export default function SyncPage() {
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    async function bootstrap() {
      try {
        const current = await getSyncProgress();
        if (active) {
          setProgress(current);
          if (current.phase === 'Failed') {
            setError(current.message || 'Sync failed');
          }
        }
      } catch (err) {
        if (active) {
          setError(err instanceof Error ? err.message : String(err));
        }
      }

      try {
        unlisten = await subscribeSyncProgress((next) => {
          if (!active) {
            return;
          }
          setProgress(next);
          if (next.phase === 'Failed') {
            setError(next.message || 'Sync failed');
          } else {
            setError(null);
          }
        });
      } catch (err) {
        if (active) {
          setError(err instanceof Error ? err.message : String(err));
        }
      }
    }

    void bootstrap();
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const canBrowse = useMemo(
    () => (progress?.issues_synced ?? 0) > 0 && progress?.phase !== 'Failed',
    [progress],
  );

  async function onRetry() {
    setError(null);
    try {
      await startFullSync();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <main className="page sync-page">
      <h1>Sync</h1>
      <SyncBanner progress={progress} error={error} onRetry={error ? onRetry : undefined} />

      {!error ? (
        <>
          <dl>
            <div>
              <dt>Phase</dt>
              <dd>{progress?.phase ?? '…'}</dd>
            </div>
            <div>
              <dt>Projects</dt>
              <dd>{progress ? `${progress.projects_done} / ${progress.projects_total}` : '…'}</dd>
            </div>
            <div>
              <dt>Issues synced</dt>
              <dd>{progress?.issues_synced ?? '…'}</dd>
            </div>
            <div>
              <dt>ETA</dt>
              <dd>{etaText(progress)}</dd>
            </div>
          </dl>
          {canBrowse ? (
            <p>
              <Link to="/">Browse dashboards while syncing</Link>
            </p>
          ) : null}
        </>
      ) : null}
    </main>
  );
}
