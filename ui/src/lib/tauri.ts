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

export type MetricsFilter = {
  project_keys: string[] | null;
  from: string | null;
  to: string | null;
  issue_types: string[] | null;
  assignee_ids: string[] | null;
};

export type Bottleneck = {
  status: string;
  total_secs: number;
};

export type ThroughputPoint = {
  day: string;
  completed_count: number;
};

export type FlowMetrics = {
  cycle_p50_secs: number | null;
  cycle_p85_secs: number | null;
  lead_p50_secs: number | null;
  lead_p85_secs: number | null;
  flow_efficiency: number | null;
  throughput: ThroughputPoint[];
  bottlenecks: Bottleneck[];
  reopens: number;
  handoffs: number;
};

export type SprintMetrics = {
  sprint_id: string;
  name: string | null;
  committed: number | null;
  completed: number | null;
  spillover: number | null;
  scope_added: number | null;
  scope_removed: number | null;
  velocity_points: number | null;
};

export type EpicRisk = {
  epic_key: string;
  score: number;
  finish_by_probability: number | null;
  drivers: string[];
  assumptions: string[];
};

export type FinishByResult = {
  probability: number;
  assumptions: string[];
};

export const emptyMetricsFilter = (): MetricsFilter => ({
  project_keys: null,
  from: null,
  to: null,
  issue_types: null,
  assignee_ids: null,
});

export async function getFlowMetrics(filter: MetricsFilter): Promise<FlowMetrics> {
  return tauriInvoke<FlowMetrics>('get_flow_metrics', { filter });
}

export async function getSprintMetrics(filter: MetricsFilter): Promise<SprintMetrics[]> {
  return tauriInvoke<SprintMetrics[]>('get_sprint_metrics', { filter });
}

export async function getEpicRisk(filter: MetricsFilter): Promise<EpicRisk[]> {
  return tauriInvoke<EpicRisk[]>('get_epic_risk', { filter });
}

export async function getFinishBy(epicKey: string, targetDate: string): Promise<FinishByResult> {
  return tauriInvoke<FinishByResult>('get_finish_by', {
    epic_key: epicKey,
    target_date: targetDate,
  });
}
