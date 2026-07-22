import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import EpicsPage from './EpicsPage';
import * as tauri from '../lib/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../lib/tauri')>('../lib/tauri');
  return {
    ...actual,
    getEpicRisk: vi.fn(),
    getFinishBy: vi.fn(),
  };
});

function mockInvokeSequence([epics, finishBy]: [
  Array<Partial<tauri.EpicRisk>>,
  Partial<tauri.FinishByResult>,
]) {
  vi.mocked(tauri.getEpicRisk).mockResolvedValue(
    epics.map((e) => ({
      epic_key: e.epic_key ?? 'E-0',
      score: e.score ?? 0,
      finish_by_probability: e.finish_by_probability ?? null,
      drivers: e.drivers ?? [],
      assumptions: e.assumptions ?? [],
    })),
  );
  vi.mocked(tauri.getFinishBy).mockResolvedValue({
    probability: finishBy.probability ?? 0,
    assumptions: finishBy.assumptions ?? [],
  });
}

describe('EpicsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('lists epic risk drivers and finish-by assumptions', async () => {
    mockInvokeSequence([
      [{ epic_key: 'E-1', score: 72, drivers: ['low throughput'] }],
      { probability: 0.41, assumptions: ['throughput ~ last 6 weeks'] },
    ]);
    render(
      <MemoryRouter>
        <EpicsPage />
      </MemoryRouter>,
    );
    expect(await screen.findByText('E-1')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText(/target date/i), {
      target: { value: '2026-12-01' },
    });
    expect(await screen.findByText(/throughput ~ last 6 weeks/i)).toBeInTheDocument();
  });
});
