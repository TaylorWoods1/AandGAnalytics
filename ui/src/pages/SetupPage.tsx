import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import DashboardNav from '../components/DashboardNav';
import SecretField from '../components/SecretField';
import { isCredentialError } from '../lib/syncErrors';
import {
  getSetupInfo,
  resetSetup,
  saveSetup,
  startFullSync,
  validateSetup,
  type SetupInfo,
  type SetupStatus,
} from '../lib/tauri';

/** Fixed Auto General AU Jira Cloud site — not user-configurable. */
export const JIRA_SITE_URL = 'https://autogeneral-au.atlassian.net';

const CREDENTIAL_SETUP_COPY =
  'Jira blocked this connection (401/403). If the detail mentions an IP allowlist, join company VPN or ask an admin to allowlist your IP. Otherwise use Create API token (not with scopes) and the matching Atlassian email.';

function ConnectionStatus({ status }: { status: SetupStatus }) {
  return (
    <section
      className={`connection-status ${status.jira_ok ? 'connection-status--ok' : 'connection-status--bad'}`}
      aria-label="Connection status"
      role="status"
    >
      <h2>Connection check</h2>
      <dl>
        <div>
          <dt>Jira</dt>
          <dd className={status.jira_ok ? 'ok' : 'bad'}>
            {status.jira_ok ? 'Connected' : 'Failed'} — {status.jira_message}
          </dd>
        </div>
        <div>
          <dt>Bedrock</dt>
          <dd className={status.bedrock_ok ? 'ok' : 'bad'}>
            {status.bedrock_ok ? 'OK' : 'Failed'} — {status.bedrock_message}
          </dd>
        </div>
      </dl>
    </section>
  );
}

export default function SetupPage() {
  const navigate = useNavigate();
  const [info, setInfo] = useState<SetupInfo | null>(null);
  const [email, setEmail] = useState('');
  const [jiraToken, setJiraToken] = useState('');
  const [bedrockKey, setBedrockKey] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [savedMessage, setSavedMessage] = useState<string | null>(null);
  const [connectionStatus, setConnectionStatus] = useState<SetupStatus | null>(null);
  const [credentialRejected, setCredentialRejected] = useState(false);
  const [busy, setBusy] = useState(false);
  const [testing, setTesting] = useState(false);
  const [resetBusy, setResetBusy] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);

  const alreadyConfigured = Boolean(info?.jira_configured);

  useEffect(() => {
    let active = true;
    void getSetupInfo()
      .then(async (next) => {
        if (!active) {
          return;
        }
        setInfo(next);
        if (next.email) {
          setEmail(next.email);
        }
        if (next.jira_configured) {
          try {
            const status = await validateSetup();
            if (active) {
              setConnectionStatus(status);
            }
          } catch {
            // Probe is best-effort on load.
          }
        }
      })
      .catch(() => {
        if (active) {
          setInfo({
            jira_configured: false,
            bedrock_configured: false,
            email: null,
            site_url: null,
            bedrock_region: null,
          });
        }
      });
    return () => {
      active = false;
    };
  }, []);

  const canSave = useMemo(
    () => email.trim().length > 0 && jiraToken.trim().length > 0,
    [email, jiraToken],
  );

  const canTest = useMemo(() => {
    if (alreadyConfigured) {
      return true;
    }
    return canSave;
  }, [alreadyConfigured, canSave]);

  async function persistIfNeeded() {
    if (!jiraToken.trim() && alreadyConfigured) {
      return;
    }
    if (!canSave) {
      throw new Error('Enter your Atlassian email and Jira API token first.');
    }
    await saveSetup(
      {
        site_url: JIRA_SITE_URL,
        email: email.trim(),
        api_token: jiraToken,
      },
      { api_key: bedrockKey.trim(), region: 'ap-southeast-2' },
    );
  }

  async function runValidate(opts: { continueOnSuccess: boolean }) {
    setError(null);
    setSavedMessage(null);
    setCredentialRejected(false);
    const bedrockProvided = bedrockKey.trim().length > 0;
    await persistIfNeeded();
    const status = await validateSetup();
    setConnectionStatus(status);

    if (!status.jira_ok || (bedrockProvided && !status.bedrock_ok)) {
      const parts = [
        !status.jira_ok ? `Jira: ${status.jira_message}` : null,
        bedrockProvided && !status.bedrock_ok ? `Bedrock: ${status.bedrock_message}` : null,
      ].filter(Boolean);
      const message = parts.join(' · ') || 'Credential validation failed';
      if (!status.jira_ok && isCredentialError(status.jira_message)) {
        setCredentialRejected(true);
      }
      throw new Error(message);
    }

    if (opts.continueOnSuccess && !alreadyConfigured) {
      await startFullSync();
      navigate('/sync');
      return;
    }

    setSavedMessage(
      alreadyConfigured && jiraToken.trim()
        ? `Saved. Jira: ${status.jira_message}`
        : `Jira: ${status.jira_message}`,
    );
    if (jiraToken.trim() || bedrockKey.trim()) {
      setJiraToken('');
      setBedrockKey('');
    }
    setInfo((prev) =>
      prev
        ? {
            ...prev,
            jira_configured: true,
            bedrock_configured: bedrockProvided || prev.bedrock_configured,
            email: email.trim() || prev.email,
          }
        : prev,
    );
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (!canSave || busy) {
      return;
    }
    setBusy(true);
    try {
      await runValidate({ continueOnSuccess: true });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (isCredentialError(message)) {
        setCredentialRejected(true);
      }
      setError(message);
    } finally {
      setBusy(false);
    }
  }

  async function onTestConnection() {
    if (!canTest || testing || busy) {
      return;
    }
    setTesting(true);
    try {
      await runValidate({ continueOnSuccess: false });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (isCredentialError(message)) {
        setCredentialRejected(true);
      }
      setError(message);
    } finally {
      setTesting(false);
    }
  }

  async function onReset() {
    if (resetBusy) {
      return;
    }
    // Tauri/WKWebView often blocks or no-ops `window.confirm` — use in-page confirm instead.
    if (!confirmReset) {
      setConfirmReset(true);
      setError(null);
      return;
    }
    setResetBusy(true);
    setError(null);
    setConnectionStatus(null);
    setSavedMessage(null);
    try {
      await resetSetup();
      setInfo({
        jira_configured: false,
        bedrock_configured: false,
        email: null,
        site_url: null,
        bedrock_region: null,
      });
      setEmail('');
      setJiraToken('');
      setBedrockKey('');
      setConfirmReset(false);
      setSavedMessage('Credentials and local data cleared. Enter setup details to continue.');
      navigate('/setup', { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setConfirmReset(false);
    } finally {
      setResetBusy(false);
    }
  }

  return (
    <main className="page setup-page">
      <header className="dashboard-header">
        <h1>{alreadyConfigured ? 'Settings' : 'Setup'}</h1>
        {alreadyConfigured ? <DashboardNav current="settings" /> : null}
      </header>

      <p className="setup-lede">
        Connect to Auto General AU Jira (
        <code>{JIRA_SITE_URL.replace(/^https:\/\//, '')}</code>). An Amazon Bedrock API key is
        optional for Ask AI. Credentials stay on this Mac in the keychain — nothing is uploaded to a
        hosted backend.
      </p>

      {alreadyConfigured ? (
        <p className="field-hint">
          Currently signed in as <strong>{info?.email ?? 'unknown'}</strong>
          {info?.bedrock_configured ? ' · Bedrock key saved' : ' · Bedrock not configured'}. Use
          Test connection to verify the saved token, or re-enter secrets below to update them.
        </p>
      ) : null}

      {connectionStatus ? <ConnectionStatus status={connectionStatus} /> : null}

      <form onSubmit={onSubmit}>
        <label htmlFor="email">Atlassian email</label>
        <input
          id="email"
          name="email"
          type="email"
          autoComplete="email"
          placeholder="you@company.com"
          aria-describedby="email-hint"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
        />
        <p id="email-hint" className="field-hint">
          The Atlassian account email that owns the API token (the address you use to sign in to
          Jira).
        </p>

        <SecretField
          id="jira-api-token"
          label="Jira API token"
          name="jiraApiToken"
          value={jiraToken}
          onChange={(e) => setJiraToken(e.target.value)}
          hintId="jira-token-hint"
          showLabel="Show Jira API token"
          hideLabel="Hide Jira API token"
          hint={
            <>
              Create a token with <strong>Create API token</strong> (not “with scopes”) at{' '}
              <a
                href="https://id.atlassian.com/manage-profile/security/api-tokens"
                target="_blank"
                rel="noreferrer"
              >
                id.atlassian.com/manage-profile/security/api-tokens
              </a>
              . Pair it with the Atlassian account email above — not a password. Scoped tokens also
              work if they include Jira read scopes.
            </>
          }
        />

        <SecretField
          id="bedrock-api-key"
          label="AWS Bedrock API key (optional)"
          name="bedrockApiKey"
          value={bedrockKey}
          onChange={(e) => setBedrockKey(e.target.value)}
          hintId="bedrock-key-hint"
          showLabel="Show Bedrock API key"
          hideLabel="Hide Bedrock API key"
          hint={
            <>
              Optional — enables Ask AI via Amazon Bedrock (Claude) in{' '}
              <code>ap-southeast-2</code>. Leave blank to keep any existing key. Create a key in the{' '}
              <a
                href="https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys-generate.html"
                target="_blank"
                rel="noreferrer"
              >
                AWS Bedrock console
              </a>
              .
            </>
          }
        />

        {error ? (
          <div role="alert" className="form-error">
            {credentialRejected ? <p>{CREDENTIAL_SETUP_COPY}</p> : null}
            <p>{error}</p>
          </div>
        ) : null}

        {savedMessage ? (
          <p role="status" className="field-hint">
            {savedMessage}
          </p>
        ) : null}

        <div className="setup-page__actions">
          <button
            type="button"
            className="setup-page__secondary"
            disabled={!canTest || testing || busy}
            onClick={() => void onTestConnection()}
          >
            {testing ? 'Testing…' : 'Test connection'}
          </button>
          <button type="submit" className="setup-page__submit" disabled={!canSave || busy || testing}>
            {busy
              ? 'Saving…'
              : alreadyConfigured
                ? 'Save credentials'
                : 'Save and continue'}
          </button>
        </div>
      </form>

      {alreadyConfigured ? (
        <section className="maintenance-actions" aria-label="Reset setup">
          <h2>Start over</h2>
          <p>
            Clear keychain credentials and delete the local analytics database, then return to first
            run setup.
          </p>
          {confirmReset ? (
            <p className="form-error" role="status">
              This permanently clears saved tokens and local sync data. Click Confirm clear to
              continue.
            </p>
          ) : null}
          <div className="maintenance-actions__buttons">
            {confirmReset ? (
              <button
                type="button"
                className="setup-page__secondary"
                onClick={() => setConfirmReset(false)}
                disabled={resetBusy}
              >
                Cancel
              </button>
            ) : null}
            <button type="button" onClick={() => void onReset()} disabled={resetBusy}>
              {resetBusy
                ? 'Resetting…'
                : confirmReset
                  ? 'Confirm clear'
                  : 'Clear credentials & local data'}
            </button>
          </div>
        </section>
      ) : null}
    </main>
  );
}
