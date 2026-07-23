import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import SetupPage, { JIRA_SITE_URL } from './SetupPage';
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
    getSetupInfo: vi.fn(),
    resetSetup: vi.fn(),
  };
});

function renderSetup(path = '/setup') {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <SetupPage />
    </MemoryRouter>,
  );
}

function fillJiraFields() {
  fireEvent.change(screen.getByLabelText(/^atlassian email$/i), {
    target: { value: 'dev@example.com' },
  });
  fireEvent.change(screen.getByLabelText(/^jira api token$/i), {
    target: { value: 'j-token' },
  });
}

function fillForm() {
  fillJiraFields();
  fireEvent.change(screen.getByLabelText(/^aws bedrock api key \(optional\)$/i), {
    target: { value: 'b-key' },
  });
}

const emptyInfo = {
  jira_configured: false,
  bedrock_configured: false,
  email: null,
  site_url: null,
  bedrock_region: null,
};

describe('SetupPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tauri.getSetupInfo).mockResolvedValue(emptyInfo);
  });

  it('disables continue until jira fields are filled; bedrock is optional', async () => {
    renderSetup();
    expect(await screen.findByRole('heading', { name: /^setup$/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /save and continue/i })).toBeDisabled();
    fillJiraFields();
    expect(screen.getByRole('button', { name: /save and continue/i })).toBeEnabled();
  });

  it('saves and continues without a bedrock key', async () => {
    vi.mocked(tauri.saveSetup).mockResolvedValue();
    vi.mocked(tauri.validateSetup).mockResolvedValue({
      jira_ok: true,
      bedrock_ok: true,
      jira_message: 'ok',
      bedrock_message: 'not configured (optional — Ask AI disabled)',
    });
    vi.mocked(tauri.startFullSync).mockResolvedValue();

    renderSetup();
    await screen.findByRole('heading', { name: /^setup$/i });
    fillJiraFields();
    fireEvent.click(screen.getByRole('button', { name: /save and continue/i }));

    await waitFor(() => {
      expect(tauri.saveSetup).toHaveBeenCalledWith(
        {
          site_url: JIRA_SITE_URL,
          email: 'dev@example.com',
          api_token: 'j-token',
        },
        { api_key: '', region: 'ap-southeast-2' },
      );
      expect(tauri.startFullSync).toHaveBeenCalled();
    });
  });

  it('confirms then clears credentials via in-page reset', async () => {
    vi.mocked(tauri.getSetupInfo).mockResolvedValue({
      jira_configured: true,
      bedrock_configured: false,
      email: 'dev@example.com',
      site_url: JIRA_SITE_URL,
      bedrock_region: null,
    });
    vi.mocked(tauri.validateSetup).mockResolvedValue({
      jira_ok: true,
      bedrock_ok: true,
      jira_message: 'authenticated as Dev User',
      bedrock_message: 'not configured (optional — Ask AI disabled)',
    });
    vi.mocked(tauri.resetSetup).mockResolvedValue();

    renderSetup('/settings');
    expect(await screen.findByRole('heading', { name: /^settings$/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /clear credentials & local data/i }));
    expect(tauri.resetSetup).not.toHaveBeenCalled();
    expect(screen.getByText(/permanently clears saved tokens/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /confirm clear/i }));
    await waitFor(() => {
      expect(tauri.resetSetup).toHaveBeenCalled();
    });
    expect(await screen.findByRole('heading', { name: /^setup$/i })).toBeInTheDocument();
  });

  it('tests jira connection and shows authenticated status', async () => {
    vi.mocked(tauri.saveSetup).mockResolvedValue();
    vi.mocked(tauri.validateSetup).mockResolvedValue({
      jira_ok: true,
      bedrock_ok: true,
      jira_message: 'authenticated as Dev User',
      bedrock_message: 'not configured (optional — Ask AI disabled)',
    });

    renderSetup();
    await screen.findByRole('heading', { name: /^setup$/i });
    fillJiraFields();
    fireEvent.click(screen.getByRole('button', { name: /test connection/i }));

    expect(await screen.findByRole('status', { name: /connection status/i })).toHaveTextContent(
      /Connected — authenticated as Dev User/i,
    );
    expect(tauri.startFullSync).not.toHaveBeenCalled();
  });

  it('hides site URL and hardcodes Auto General AU Jira', async () => {
    vi.mocked(tauri.saveSetup).mockResolvedValue();
    vi.mocked(tauri.validateSetup).mockResolvedValue({
      jira_ok: true,
      bedrock_ok: true,
      jira_message: 'ok',
      bedrock_message: 'ok',
    });
    vi.mocked(tauri.startFullSync).mockResolvedValue();

    renderSetup();
    await screen.findByRole('heading', { name: /^setup$/i });
    expect(screen.queryByLabelText(/site url/i)).not.toBeInTheDocument();
    expect(screen.getByText(/autogeneral-au\.atlassian\.net/i)).toBeInTheDocument();
    expect(
      screen.getByText(/Atlassian account email that owns the API token/i),
    ).toBeInTheDocument();

    fillForm();
    fireEvent.click(screen.getByRole('button', { name: /save and continue/i }));

    await waitFor(() => {
      expect(tauri.saveSetup).toHaveBeenCalledWith(
        {
          site_url: JIRA_SITE_URL,
          email: 'dev@example.com',
          api_token: 'j-token',
        },
        { api_key: 'b-key', region: 'ap-southeast-2' },
      );
    });
  });

  it('toggles jira and bedrock secrets between password and text', async () => {
    renderSetup();
    await screen.findByRole('heading', { name: /^setup$/i });
    const jiraInput = screen.getByLabelText(/^jira api token$/i);
    const bedrockInput = screen.getByLabelText(/^aws bedrock api key \(optional\)$/i);
    expect(jiraInput).toHaveAttribute('type', 'password');
    expect(bedrockInput).toHaveAttribute('type', 'password');

    fireEvent.click(screen.getByRole('button', { name: /show jira api token/i }));
    expect(jiraInput).toHaveAttribute('type', 'text');
    expect(screen.getByRole('button', { name: /hide jira api token/i })).toHaveAttribute(
      'aria-pressed',
      'true',
    );

    fireEvent.click(screen.getByRole('button', { name: /show bedrock api key/i }));
    expect(bedrockInput).toHaveAttribute('type', 'text');

    fireEvent.click(screen.getByRole('button', { name: /hide jira api token/i }));
    expect(jiraInput).toHaveAttribute('type', 'password');
  });

  it('shows 401/403 credential refresh copy when Jira rejects the token', async () => {
    vi.mocked(tauri.saveSetup).mockResolvedValue();
    vi.mocked(tauri.validateSetup).mockResolvedValue({
      jira_ok: false,
      bedrock_ok: true,
      jira_message: 'unauthorized (HTTP 401): update your Jira API token',
      bedrock_message: 'ok',
    });

    renderSetup();
    await screen.findByRole('heading', { name: /^setup$/i });
    fillForm();
    fireEvent.click(screen.getByRole('button', { name: /save and continue/i }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/401\/403/i);
    await waitFor(() => {
      expect(tauri.startFullSync).not.toHaveBeenCalled();
    });
  });
});
