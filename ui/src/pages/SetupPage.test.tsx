import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import SetupPage from './SetupPage';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => undefined),
}));

function renderSetup() {
  return render(
    <MemoryRouter>
      <SetupPage />
    </MemoryRouter>,
  );
}

describe('SetupPage', () => {
  it('disables continue until jira and gemini fields are filled', () => {
    renderSetup();
    expect(screen.getByRole('button', { name: /save and continue/i })).toBeDisabled();
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
    expect(screen.getByRole('button', { name: /save and continue/i })).toBeEnabled();
  });
});
