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
    getStoryPointsMapping: vi.fn(),
    setStoryPointsMapping: vi.fn(),
  };
});

describe('SyncPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tauri.getStoryPointsMapping).mockResolvedValue({
      status: 'resolved',
      jira_field_id: 'customfield_10016',
      jira_field_name: 'Story Points',
      candidates: [],
    });
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

  it('lets the user pick a story points field when mapping is unresolved', async () => {
    vi.mocked(tauri.getSyncProgress).mockResolvedValue({
      phase: 'Idle',
      projects_done: 1,
      projects_total: 1,
      issues_synced: 10,
      message: 'full sync complete',
    });
    vi.mocked(tauri.getStoryPointsMapping).mockResolvedValue({
      status: 'unresolved',
      jira_field_id: null,
      jira_field_name: 'Story Points, Story point estimate',
      candidates: [
        { id: 'customfield_10016', name: 'Story Points' },
        { id: 'customfield_10028', name: 'Story point estimate' },
      ],
    });
    vi.mocked(tauri.setStoryPointsMapping).mockResolvedValue({
      status: 'resolved',
      jira_field_id: 'customfield_10016',
      jira_field_name: 'Story Points',
      candidates: [
        { id: 'customfield_10016', name: 'Story Points' },
        { id: 'customfield_10028', name: 'Story point estimate' },
      ],
    });

    render(
      <MemoryRouter>
        <SyncPage />
      </MemoryRouter>,
    );

    expect(await screen.findByRole('heading', { name: /story points field/i })).toBeInTheDocument();
    const save = screen.getByRole('button', { name: /save story points mapping/i });
    expect(save).toBeEnabled();
    save.click();
    await waitFor(() => {
      expect(tauri.setStoryPointsMapping).toHaveBeenCalledWith('customfield_10016');
    });
  });
});
