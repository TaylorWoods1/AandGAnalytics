import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import HomePage from './HomePage';
import * as tauri from '../lib/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../lib/tauri')>('../lib/tauri');
  return {
    ...actual,
    getPerformanceMetrics: vi.fn(),
    getFlowMetrics: vi.fn(),
  };
});

describe('HomePage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders executive sections from performance metrics', async () => {
    vi.mocked(tauri.getPerformanceMetrics).mockResolvedValue({
      by_person: [{ account_id: 'bob', completed_count: 5, points: 10 }],
      by_project: [
        {
          project_key: 'ALPHA',
          open_count: 2,
          completed_in_range: 9,
          blocker_count: 2,
          blocked_secs: 7200,
        },
      ],
      person_month: [
        {
          month: '2024-02',
          account_id: 'bob',
          completed_count: 4,
          points: 8,
          rate_change: 1.0,
        },
      ],
      project_month: [],
    });
    vi.mocked(tauri.getFlowMetrics).mockResolvedValue({
      cycle_p50_secs: 86400,
      cycle_p85_secs: null,
      lead_p50_secs: null,
      lead_p85_secs: null,
      flow_efficiency: null,
      throughput: [{ day: '2024-02-01', completed_count: 3 }],
      bottlenecks: [],
      reopens: 0,
      handoffs: 0,
    });

    render(
      <MemoryRouter>
        <HomePage />
      </MemoryRouter>,
    );

    expect(await screen.findByRole('heading', { name: 'Top movers' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Top projects by completions' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Blocker hotspots' })).toBeInTheDocument();
    expect(screen.getAllByText('ALPHA').length).toBeGreaterThan(0);
    expect(screen.getByText('100%')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open Performance' })).toHaveAttribute(
      'href',
      '/performance',
    );
  });
});
