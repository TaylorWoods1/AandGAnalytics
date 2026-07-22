import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import FlowPage from './FlowPage';
import * as tauri from '../lib/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../lib/tauri')>('../lib/tauri');
  return {
    ...actual,
    getFlowMetrics: vi.fn(),
  };
});

function mockInvoke(partial: Partial<tauri.FlowMetrics>) {
  vi.mocked(tauri.getFlowMetrics).mockResolvedValue({
    cycle_p50_secs: null,
    cycle_p85_secs: null,
    lead_p50_secs: null,
    lead_p85_secs: null,
    flow_efficiency: null,
    throughput: [],
    bottlenecks: [],
    reopens: 0,
    handoffs: 0,
    ...partial,
  });
}

describe('FlowPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders bottleneck statuses from metrics payload', async () => {
    mockInvoke({
      bottlenecks: [
        { status: 'Code Review', total_secs: 90000 },
        { status: 'In Progress', total_secs: 40000 },
      ],
    });
    render(
      <MemoryRouter>
        <FlowPage />
      </MemoryRouter>,
    );
    expect(await screen.findByText('Code Review')).toBeInTheDocument();
  });
});
