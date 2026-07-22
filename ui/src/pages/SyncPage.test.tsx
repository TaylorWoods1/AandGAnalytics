import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import SyncPage from './SyncPage';
import * as tauri from '../lib/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => undefined),
}));

vi.mock('../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../lib/tauri')>('../lib/tauri');
  return {
    ...actual,
    getSyncProgress: vi.fn(),
    startFullSync: vi.fn(),
    subscribeSyncProgress: vi.fn(async () => () => undefined),
  };
});

describe('SyncPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows phase, counts, and browse link after issues sync', async () => {
    vi.mocked(tauri.getSyncProgress).mockResolvedValue({
      phase: 'Issues',
      projects_done: 2,
      projects_total: 5,
      issues_synced: 50,
      message: 'page 1 complete',
    });

    render(
      <MemoryRouter>
        <SyncPage />
      </MemoryRouter>,
    );

    expect(await screen.findByText('Issues')).toBeInTheDocument();
    expect(screen.getByText('2 / 5')).toBeInTheDocument();
    expect(screen.getByText('50')).toBeInTheDocument();
    expect(screen.getByText(/Status: page 1 complete/i)).toBeInTheDocument();
    expect(
      screen.getByRole('link', { name: /browse dashboards while syncing/i }),
    ).toBeInTheDocument();
  });

  it('shows error banner with retry when sync failed', async () => {
    vi.mocked(tauri.getSyncProgress).mockResolvedValue({
      phase: 'Failed',
      projects_done: 0,
      projects_total: 0,
      issues_synced: 0,
      message: 'network down',
    });

    render(
      <MemoryRouter>
        <SyncPage />
      </MemoryRouter>,
    );

    expect(await screen.findByRole('alert')).toHaveTextContent('network down');
    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();

    await waitFor(async () => {
      screen.getByRole('button', { name: /retry/i }).click();
      expect(tauri.startFullSync).toHaveBeenCalled();
    });
  });
});
