import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import SprintsPage from './SprintsPage';
import * as tauri from '../lib/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../lib/tauri')>('../lib/tauri');
  return {
    ...actual,
    getSprintMetrics: vi.fn(),
  };
});

function mockInvoke(rows: Array<Partial<tauri.SprintMetrics>>) {
  vi.mocked(tauri.getSprintMetrics).mockResolvedValue(
    rows.map((row) => ({
      sprint_id: row.sprint_id ?? 's-1',
      name: row.name ?? null,
      committed: row.committed ?? null,
      completed: row.completed ?? null,
      spillover: row.spillover ?? null,
      scope_added: row.scope_added ?? null,
      scope_removed: row.scope_removed ?? null,
      velocity_points: row.velocity_points ?? null,
    })),
  );
}

describe('SprintsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows spillover count per sprint', async () => {
    mockInvoke([{ name: 'Sprint 42', spillover: 3, committed: 10, completed: 7 }]);
    render(
      <MemoryRouter>
        <SprintsPage />
      </MemoryRouter>,
    );
    expect(await screen.findByText('Sprint 42')).toBeInTheDocument();
    expect(screen.getByText(/spillover:\s*3/i)).toBeInTheDocument();
  });
});
