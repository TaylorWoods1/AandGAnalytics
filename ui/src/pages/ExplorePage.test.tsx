import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import ExplorePage from './ExplorePage';
import * as tauri from '../lib/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../lib/tauri')>('../lib/tauri');
  return {
    ...actual,
    listIssues: vi.fn(),
  };
});

function mockInvoke(page: Partial<tauri.IssuePage>) {
  vi.mocked(tauri.listIssues).mockResolvedValue({
    total: page.total ?? 0,
    items: (page.items ?? []).map((row) => ({
      key: row.key ?? 'X-0',
      summary: row.summary ?? null,
      project_key: row.project_key ?? 'X',
      status: row.status ?? null,
      assignee: row.assignee ?? null,
      story_points: row.story_points ?? null,
      cycle_secs: row.cycle_secs ?? null,
      updated: row.updated ?? '',
    })),
  });
}

describe('ExplorePage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders issue rows and page total', async () => {
    mockInvoke({
      total: 2,
      items: [
        {
          key: 'PROJ-1',
          summary: 'Wire sync',
          project_key: 'PROJ',
          status: 'Done',
          assignee: 'Ada',
          story_points: 3,
          cycle_secs: 86400,
          updated: '2026-01-02T00:00:00Z',
        },
      ],
    });
    render(
      <MemoryRouter>
        <ExplorePage />
      </MemoryRouter>,
    );
    expect(await screen.findByText('PROJ-1')).toBeInTheDocument();
    expect(screen.getByText(/total:\s*2/i)).toBeInTheDocument();
  });
});
