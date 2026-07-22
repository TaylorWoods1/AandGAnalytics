import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import SetupPage from './SetupPage';
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
    saveSetup: vi.fn(),
    validateSetup: vi.fn(),
    startFullSync: vi.fn(),
  };
});

function renderSetup() {
  return render(
    <MemoryRouter>
      <SetupPage />
    </MemoryRouter>,
  );
}

function fillForm() {
  fireEvent.change(screen.getByLabelText(/site url/i), {
    target: { value: 'https://example.atlassian.net' },
  });
  fireEvent.change(screen.getByLabelText(/email/i), {
    target: { value: 'dev@example.com' },
  });
  fireEvent.change(screen.getByLabelText(/jira api token/i), {
    target: { value: 'j-token' },
  });
  fireEvent.change(screen.getByLabelText(/gemini api key/i), {
    target: { value: 'g-key' },
  });
}

describe('SetupPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('disables continue until jira and gemini fields are filled', () => {
    renderSetup();
    expect(screen.getByRole('button', { name: /save and continue/i })).toBeDisabled();
    fillForm();
    expect(screen.getByRole('button', { name: /save and continue/i })).toBeEnabled();
  });

  it('shows 401/403 credential refresh copy when Jira rejects the token', async () => {
    vi.mocked(tauri.saveSetup).mockResolvedValue();
    vi.mocked(tauri.validateSetup).mockResolvedValue({
      jira_ok: false,
      gemini_ok: true,
      jira_message: 'unauthorized (HTTP 401): update your Jira API token',
      gemini_message: 'ok',
    });

    renderSetup();
    fillForm();
    fireEvent.click(screen.getByRole('button', { name: /save and continue/i }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/401\/403/i);
    await waitFor(() => {
      expect(tauri.startFullSync).not.toHaveBeenCalled();
    });
  });
});
