import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type JiraCredentials = {
  site_url: string;
  email: string;
  api_token: string;
};

export type BedrockCredentials = {
  api_key: string;
  region?: string;
};

export type SetupStatus = {
  jira_ok: boolean;
  bedrock_ok: boolean;
  jira_message: string;
  bedrock_message: string;
};

export type SyncProgress = {
  phase: string;
  projects_done: number;
  projects_total: number;
  issues_synced: number;
  message: string;
};

export async function saveSetup(
  jira: JiraCredentials,
  bedrock: BedrockCredentials,
): Promise<void> {
  await tauriInvoke('save_setup', { jira, bedrock });
}

export async function validateSetup(): Promise<SetupStatus> {
  return tauriInvoke<SetupStatus>('validate_setup');
}

export type SetupInfo = {
  jira_configured: boolean;
  bedrock_configured: boolean;
  email: string | null;
  site_url: string | null;
  bedrock_region: string | null;
};

export async function getSetupInfo(): Promise<SetupInfo> {
  return tauriInvoke<SetupInfo>('get_setup_info');
}

/** Wipe keychain credentials and local SQLite DB for a fresh onboard. */
export async function resetSetup(): Promise<void> {
  await tauriInvoke('reset_setup');
}

export async function startFullSync(): Promise<void> {
  await tauriInvoke('start_full_sync');
}

/** Rebuild derived analytics tables from raw SQLite (keeps raw issues). */
export async function rebuildDerived(): Promise<void> {
  await tauriInvoke('rebuild_derived');
}

/** Reset sync checkpoints (keeps credentials) and start a full sync. */
export async function fullResync(): Promise<void> {
  await tauriInvoke('full_resync');
}

export type FieldCandidate = {
  id: string;
  name: string;
};

export type StoryPointsMapping = {
  status: string;
  jira_field_id: string | null;
  jira_field_name: string | null;
  candidates: FieldCandidate[];
};

export async function getStoryPointsMapping(): Promise<StoryPointsMapping> {
  return tauriInvoke<StoryPointsMapping>('get_story_points_mapping');
}

export async function setStoryPointsMapping(jiraFieldId: string): Promise<StoryPointsMapping> {
  return tauriInvoke<StoryPointsMapping>('set_story_points_mapping', {
    jira_field_id: jiraFieldId,
  });
}

export async function getSyncProgress(): Promise<SyncProgress> {
  return tauriInvoke<SyncProgress>('get_sync_progress');
}

/** True when Jira credentials are stored locally. */
export async function hasCredentials(): Promise<boolean> {
  try {
    const info = await getSetupInfo();
    return info.jira_configured;
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

export type Page = {
  offset: number;
  limit: number;
};

export type IssueRow = {
  key: string;
  summary: string | null;
  project_key: string;
  status: string | null;
  assignee: string | null;
  story_points: number | null;
  cycle_secs: number | null;
  updated: string;
};

export type IssuePage = {
  items: IssueRow[];
  total: number;
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

export async function listIssues(filter: MetricsFilter, page: Page): Promise<IssuePage> {
  return tauriInvoke<IssuePage>('list_issues', { filter, page });
}

export type PersonVelocity = {
  account_id: string;
  completed_count: number;
  points: number | null;
};

export type ProjectPerf = {
  project_key: string;
  open_count: number;
  completed_in_range: number;
  blocker_count: number;
  blocked_secs: number;
};

export type PersonMonth = {
  month: string;
  account_id: string;
  completed_count: number;
  points: number | null;
  rate_change: number | null;
};

export type ProjectMonth = {
  month: string;
  project_key: string;
  completed_count: number;
};

export type PerformanceMetrics = {
  by_person: PersonVelocity[];
  by_project: ProjectPerf[];
  person_month: PersonMonth[];
  project_month: ProjectMonth[];
};

export async function getPerformanceMetrics(
  filter: MetricsFilter,
): Promise<PerformanceMetrics> {
  return tauriInvoke<PerformanceMetrics>('get_performance_metrics', { filter });
}

export type IssueCite = {
  key: string;
  summary: string | null;
  status: string | null;
  project_key: string;
  cycle_secs: number | null;
};

export type ContextPack = {
  filter_summary: string;
  metrics_markdown: string;
  supporting_issues: IssueCite[];
  approx_tokens: number;
};

export type AiAnswer = {
  text: string;
  citations: string[];
};

export async function previewContextPack(filter: MetricsFilter): Promise<ContextPack> {
  return tauriInvoke<ContextPack>('preview_context_pack', { filter });
}

export async function askAi(filter: MetricsFilter, question: string): Promise<AiAnswer> {
  return tauriInvoke<AiAnswer>('ask_ai', { filter, question });
}

export async function getSuggestedPrompts(): Promise<string[]> {
  const result = await tauriInvoke<{ prompts: string[] }>('get_suggested_prompts');
  return result.prompts;
}
