import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import App from './App';
import * as tauri from './lib/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => undefined),
}));

vi.mock('./lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('./lib/tauri')>('./lib/tauri');
  return {
    ...actual,
    hasCredentials: vi.fn(),
  };
});

describe('App', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('redirects to setup when credentials are missing', async () => {
    vi.mocked(tauri.hasCredentials).mockResolvedValue(false);

    render(
      <MemoryRouter initialEntries={['/']}>
        <App />
      </MemoryRouter>,
    );

    expect(await screen.findByRole('heading', { name: /setup/i })).toBeInTheDocument();
  });

  it('shows home placeholder when credentials exist', async () => {
    vi.mocked(tauri.hasCredentials).mockResolvedValue(true);

    render(
      <MemoryRouter initialEntries={['/']}>
        <App />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /aandg analytics/i })).toBeInTheDocument();
    });
  });
});
