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
} from '../lib/tauri';

/** Fixed Auto General AU Jira Cloud site — not user-configurable. */
export const JIRA_SITE_URL = 'https://autogeneral-au.atlassian.net';

const CREDENTIAL_SETUP_COPY =
  'Jira returned 401/403 — your email or API token was rejected. Update the fields below and save again.';

export default function SetupPage() {
  const navigate = useNavigate();
  const [info, setInfo] = useState<SetupInfo | null>(null);
  const [email, setEmail] = useState('');
  const [jiraToken, setJiraToken] = useState('');
  const [bedrockKey, setBedrockKey] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [savedMessage, setSavedMessage] = useState<string | null>(null);
  const [credentialRejected, setCredentialRejected] = useState(false);
  const [busy, setBusy] = useState(false);
  const [resetBusy, setResetBusy] = useState(false);

  const alreadyConfigured = Boolean(info?.jira_configured);

  useEffect(() => {
    let active = true;
    void getSetupInfo()
      .then((next) => {
        if (!active) {
          return;
        }
        setInfo(next);
        if (next.email) {
          setEmail(next.email);
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

  const canContinue = useMemo(
    () => email.trim().length > 0 && jiraToken.trim().length > 0,
    [email, jiraToken],
  );

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (!canContinue || busy) {
      return;
    }

    setBusy(true);
    setError(null);
    setSavedMessage(null);
    setCredentialRejected(false);
    try {
      const bedrockProvided = bedrockKey.trim().length > 0;
      await saveSetup(
        {
          site_url: JIRA_SITE_URL,
          email: email.trim(),
          api_token: jiraToken,
        },
        { api_key: bedrockKey.trim(), region: 'ap-southeast-2' },
      );
      const status = await validateSetup();
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

      if (alreadyConfigured) {
        setSavedMessage('Credentials saved.');
        setJiraToken('');
        setBedrockKey('');
        setInfo((prev) =>
          prev
            ? {
                ...prev,
                jira_configured: true,
                bedrock_configured: bedrockProvided || prev.bedrock_configured,
                email: email.trim(),
              }
            : prev,
        );
      } else {
        await startFullSync();
        navigate('/sync');
      }
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

  async function onReset() {
    if (resetBusy) {
      return;
    }
    const ok = window.confirm(
      'Clear saved credentials and local analytics data? You will start onboarding from scratch.',
    );
    if (!ok) {
      return;
    }
    setResetBusy(true);
    setError(null);
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
      setSavedMessage(null);
      navigate('/setup', { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
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
          {info?.bedrock_configured ? ' · Bedrock key saved' : ' · Bedrock not configured'}. Re-enter
          secrets below to update them.
        </p>
      ) : null}

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
              Create a token at{' '}
              <a
                href="https://id.atlassian.com/manage-profile/security/api-tokens"
                target="_blank"
                rel="noreferrer"
              >
                id.atlassian.com/manage-profile/security/api-tokens
              </a>
              . Pair it with the email above — not a password.
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

        <button type="submit" className="setup-page__submit" disabled={!canContinue || busy}>
          {busy ? 'Saving…' : alreadyConfigured ? 'Save credentials' : 'Save and continue'}
        </button>
      </form>

      {alreadyConfigured ? (
        <section className="maintenance-actions" aria-label="Reset setup">
          <h2>Start over</h2>
          <p>
            Clear keychain credentials and delete the local analytics database, then return to first
            run setup.
          </p>
          <div className="maintenance-actions__buttons">
            <button type="button" onClick={() => void onReset()} disabled={resetBusy}>
              {resetBusy ? 'Resetting…' : 'Clear credentials & local data'}
            </button>
          </div>
        </section>
      ) : null}
    </main>
  );
}
