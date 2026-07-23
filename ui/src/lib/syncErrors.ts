/** True when the error looks like a Jira 401/403 credential failure. */
export function isCredentialError(message: string): boolean {
  const m = message.toLowerCase();
  return (
    m.includes('unauthorized') ||
    m.includes('forbidden') ||
    m.includes('401') ||
    m.includes('403') ||
    m.includes('allowlist') ||
    m.includes('ip address')
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
  'Jira rejected the connection (401/403). Often this is the org IP allowlist — use company VPN, or update the token on Settings.';
