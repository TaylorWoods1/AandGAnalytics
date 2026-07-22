import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import AskAiPage from './AskAiPage';
import * as tauri from '../lib/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../lib/tauri')>('../lib/tauri');
  return {
    ...actual,
    askAi: vi.fn(),
    previewContextPack: vi.fn(),
    getSuggestedPrompts: vi.fn(),
  };
});

function mockAskAi(answer: tauri.GeminiAnswer) {
  vi.mocked(tauri.askAi).mockResolvedValue(answer);
}

describe('AskAiPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tauri.getSuggestedPrompts).mockResolvedValue([
      'What is our biggest flow bottleneck right now?',
    ]);
    // Preview uses a different key so citation PROJ-1 is unique to the answer.
    vi.mocked(tauri.previewContextPack).mockResolvedValue({
      filter_summary: 'projects=all',
      metrics_markdown: '- cycle_p50_secs: 100',
      supporting_issues: [
        {
          key: 'DEMO-9',
          summary: 'Lag',
          status: 'In Progress',
          project_key: 'DEMO',
          cycle_secs: 100,
        },
      ],
      approx_tokens: 40,
    });
  });

  it('shows citations returned by ask_ai', async () => {
    mockAskAi({
      text: 'Review is the bottleneck.',
      citations: ['PROJ-1', 'bottleneck:Code Review'],
    });
    render(
      <MemoryRouter>
        <AskAiPage />
      </MemoryRouter>,
    );

    fireEvent.change(screen.getByLabelText(/question/i), {
      target: { value: 'Where is the bottleneck?' },
    });
    fireEvent.click(screen.getByRole('button', { name: /ask/i }));
    const answer = await screen.findByLabelText(/ai answer/i);
    expect(within(answer).getByText(/PROJ-1/)).toBeInTheDocument();
  });
});
