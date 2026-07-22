import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type JiraCredentials = {
  site_url: string;
  email: string;
  api_token: string;
};

export type GeminiCredentials = {
  api_key: string;
};

export type SetupStatus = {
  jira_ok: boolean;
  gemini_ok: boolean;
  jira_message: string;
  gemini_message: string;
};

export type SyncProgress = {
  phase: string;
  projects_done: number;
  projects_total: number;
  issues_synced: number;
  message: string;
};

export async function saveSetup(jira: JiraCredentials, gemini: GeminiCredentials): Promise<void> {
  await tauriInvoke('save_setup', { jira, gemini });
}

export async function validateSetup(): Promise<SetupStatus> {
  return tauriInvoke<SetupStatus>('validate_setup');
}

export async function startFullSync(): Promise<void> {
  await tauriInvoke('start_full_sync');
}

export async function getSyncProgress(): Promise<SyncProgress> {
  return tauriInvoke<SyncProgress>('get_sync_progress');
}

/** True when credentials appear configured (validate_setup succeeds). */
export async function hasCredentials(): Promise<boolean> {
  try {
    await validateSetup();
    return true;
  } catch {
    return false;
  }
}

export function subscribeSyncProgress(
  onProgress: (progress: SyncProgress) => void,
): Promise<UnlistenFn> {
  return listen<SyncProgress>('sync-progress', (event) => {
    onProgress(event.payload);
  });
}
