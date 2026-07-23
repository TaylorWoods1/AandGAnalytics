import { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import DashboardNav from '../components/DashboardNav';
import SyncBanner from '../components/SyncBanner';
import {
  fullResync,
  getStoryPointsMapping,
  getSyncProgress,
  rebuildDerived,
  setStoryPointsMapping,
  startFullSync,
  subscribeSyncProgress,
  type StoryPointsMapping,
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
  const [mapping, setMapping] = useState<StoryPointsMapping | null>(null);
  const [selectedFieldId, setSelectedFieldId] = useState('');
  const [mappingBusy, setMappingBusy] = useState(false);

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
        const nextMapping = await getStoryPointsMapping();
        if (active) {
          setMapping(nextMapping);
          setSelectedFieldId(nextMapping.jira_field_id ?? nextMapping.candidates[0]?.id ?? '');
        }
      } catch {
        // Mapping is optional until the first sync discovers fields.
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

  const needsStoryPointsPick = useMemo(() => {
    if (!mapping) {
      return false;
    }
    return (
      mapping.status !== 'resolved' &&
      mapping.candidates.some((candidate) => candidate.id.trim().length > 0)
    );
  }, [mapping]);

  async function onRetry() {
    setError(null);
    try {
      await startFullSync();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onRebuildDerived() {
    setError(null);
    try {
      await rebuildDerived();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onFullResync() {
    setError(null);
    try {
      await fullResync();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onSaveStoryPointsMapping() {
    if (!selectedFieldId || mappingBusy) {
      return;
    }
    setMappingBusy(true);
    setError(null);
    try {
      const next = await setStoryPointsMapping(selectedFieldId);
      setMapping(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setMappingBusy(false);
    }
  }

  return (
    <main className="page sync-page">
      <header className="dashboard-header">
        <h1>Sync</h1>
        <DashboardNav current="sync" />
      </header>
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

      {needsStoryPointsPick ? (
        <section className="story-points-mapping" aria-label="Story points field">
          <h2>Story points field</h2>
          <p>Multiple Jira fields matched story points. Choose the field to use for analytics.</p>
          <label htmlFor="story-points-field">Jira field</label>
          <select
            id="story-points-field"
            value={selectedFieldId}
            onChange={(event) => setSelectedFieldId(event.target.value)}
          >
            {mapping?.candidates.map((candidate) => (
              <option key={candidate.id} value={candidate.id}>
                {candidate.name} ({candidate.id})
              </option>
            ))}
          </select>
          <button
            type="button"
            disabled={!selectedFieldId || mappingBusy}
            onClick={() => void onSaveStoryPointsMapping()}
          >
            {mappingBusy ? 'Saving…' : 'Save story points mapping'}
          </button>
        </section>
      ) : null}

      <section className="maintenance-actions" aria-label="Maintenance">
        <h2>Maintenance</h2>
        <p>
          Rebuild analytics from local raw data, or reset sync progress and pull from Jira again.
        </p>
        <div className="maintenance-actions__buttons">
          <button type="button" onClick={() => void onRebuildDerived()}>
            Rebuild derived
          </button>
          <button type="button" onClick={() => void onFullResync()}>
            Full re-sync
          </button>
        </div>
      </section>
    </main>
  );
}
