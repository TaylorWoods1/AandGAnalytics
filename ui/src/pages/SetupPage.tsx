import { useMemo, useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import SecretField from '../components/SecretField';
import { isCredentialError } from '../lib/syncErrors';
import { saveSetup, startFullSync, validateSetup } from '../lib/tauri';

/** Fixed Auto General AU Jira Cloud site — not user-configurable. */
export const JIRA_SITE_URL = 'https://autogeneral-au.atlassian.net';

const CREDENTIAL_SETUP_COPY =
  'Jira returned 401/403 — your email or API token was rejected. Update the fields below and save again.';

export default function SetupPage() {
  const navigate = useNavigate();
  const [email, setEmail] = useState('');
  const [jiraToken, setJiraToken] = useState('');
  const [geminiKey, setGeminiKey] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [credentialRejected, setCredentialRejected] = useState(false);
  const [busy, setBusy] = useState(false);

  const canContinue = useMemo(
    () => email.trim().length > 0 && jiraToken.trim().length > 0 && geminiKey.trim().length > 0,
    [email, jiraToken, geminiKey],
  );

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (!canContinue || busy) {
      return;
    }

    setBusy(true);
    setError(null);
    setCredentialRejected(false);
    try {
      await saveSetup(
        {
          site_url: JIRA_SITE_URL,
          email: email.trim(),
          api_token: jiraToken,
        },
        { api_key: geminiKey },
      );
      const status = await validateSetup();
      if (!status.jira_ok || !status.gemini_ok) {
        const parts = [
          !status.jira_ok ? `Jira: ${status.jira_message}` : null,
          !status.gemini_ok ? `Gemini: ${status.gemini_message}` : null,
        ].filter(Boolean);
        const message = parts.join(' · ') || 'Credential validation failed';
        if (!status.jira_ok && isCredentialError(status.jira_message)) {
          setCredentialRejected(true);
        }
        throw new Error(message);
      }
      await startFullSync();
      navigate('/sync');
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

  return (
    <main className="page setup-page">
      <h1>Setup</h1>
      <p className="setup-lede">
        Connect to Auto General AU Jira (
        <code>{JIRA_SITE_URL.replace(/^https:\/\//, '')}</code>) and a Gemini API key. Credentials
        stay on this Mac in the keychain — nothing is uploaded to a hosted backend.
      </p>
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
          id="gemini-api-key"
          label="Gemini API key"
          name="geminiApiKey"
          value={geminiKey}
          onChange={(e) => setGeminiKey(e.target.value)}
          hintId="gemini-key-hint"
          showLabel="Show Gemini API key"
          hideLabel="Hide Gemini API key"
          hint={
            <>
              Used for Ask AI. Create a key in Google AI Studio at{' '}
              <a href="https://aistudio.google.com/apikey" target="_blank" rel="noreferrer">
                aistudio.google.com/apikey
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

        <button type="submit" className="setup-page__submit" disabled={!canContinue || busy}>
          {busy ? 'Saving…' : 'Save and continue'}
        </button>
      </form>
    </main>
  );
}
