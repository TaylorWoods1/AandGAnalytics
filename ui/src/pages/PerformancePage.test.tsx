import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import PerformancePage from './PerformancePage';
import * as tauri from '../lib/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../lib/tauri')>('../lib/tauri');
  return {
    ...actual,
    getPerformanceMetrics: vi.fn(),
  };
});

describe('PerformancePage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders section headings and a person velocity row', async () => {
    vi.mocked(tauri.getPerformanceMetrics).mockResolvedValue({
      by_person: [
        { account_id: 'bob-account-id-long', completed_count: 7, points: 12 },
      ],
      by_project: [
        {
          project_key: 'DEMO',
          open_count: 3,
          completed_in_range: 7,
          blocker_count: 1,
          blocked_secs: 3600,
        },
      ],
      person_month: [
        {
          month: '2024-02',
          account_id: 'bob-account-id-long',
          completed_count: 4,
          points: 8,
          rate_change: 0.33,
        },
      ],
      project_month: [{ month: '2024-02', project_key: 'DEMO', completed_count: 4 }],
    });

    render(
      <MemoryRouter>
        <PerformancePage />
      </MemoryRouter>,
    );

    expect(await screen.findByRole('heading', { name: 'People velocity' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Project breakdown' })).toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: 'Tickets per person per month' }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: 'Completions by project month' }),
    ).toBeInTheDocument();
    const personCells = screen.getAllByTitle('bob-account-id-long');
    expect(personCells.length).toBeGreaterThan(0);
    expect(screen.getByText('12.0')).toBeInTheDocument();
  });
});
