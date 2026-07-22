import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { OFFLINE_BANNER_COPY } from '../lib/syncErrors';
import SyncBanner from './SyncBanner';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('SyncBanner', () => {
  it('shows offline banner copy when sync fails with a network error', () => {
    render(
      <MemoryRouter>
        <SyncBanner
          progress={null}
          error="HTTP error: connection refused"
          onRetry={() => undefined}
        />
      </MemoryRouter>,
    );

    expect(screen.getByRole('alert')).toHaveTextContent(OFFLINE_BANNER_COPY);
    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
  });

  it('prompts credential refresh on 401/403', () => {
    render(
      <MemoryRouter>
        <SyncBanner progress={null} error="jira error: unauthorized" onRetry={() => undefined} />
      </MemoryRouter>,
    );

    expect(screen.getByRole('alert')).toHaveTextContent(/401\/403/i);
    expect(screen.getByRole('link', { name: /refresh credentials/i })).toHaveAttribute(
      'href',
      '/setup',
    );
  });
});
