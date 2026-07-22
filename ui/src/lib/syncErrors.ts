/** True when the error looks like a Jira 401/403 credential failure. */
export function isCredentialError(message: string): boolean {
  const m = message.toLowerCase();
  return (
    m.includes('unauthorized') || m.includes('forbidden') || m.includes('401') || m.includes('403')
  );
}

/** True when the error looks like offline / network failure. */
export function isOfflineError(message: string): boolean {
  const m = message.toLowerCase();
  return (
    m.includes('offline') ||
    m.includes('network') ||
    m.includes('connection') ||
    m.includes('timed out') ||
    m.includes('timeout') ||
    m.includes('dns') ||
    m.includes('unreachable') ||
    m.includes('http error')
  );
}

export const OFFLINE_BANNER_COPY =
  'You appear to be offline or Jira is unreachable. Dashboards still work from local SQLite data.';

export const CREDENTIAL_BANNER_COPY =
  'Jira rejected your credentials (401/403). Update your token on the Setup page, then retry sync.';
