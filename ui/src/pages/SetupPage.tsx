import { useMemo, useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { saveSetup, startFullSync, validateSetup } from '../lib/tauri';

export default function SetupPage() {
  const navigate = useNavigate();
  const [siteUrl, setSiteUrl] = useState('');
  const [email, setEmail] = useState('');
  const [jiraToken, setJiraToken] = useState('');
  const [geminiKey, setGeminiKey] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const canContinue = useMemo(
    () =>
      siteUrl.trim().length > 0 &&
      email.trim().length > 0 &&
      jiraToken.trim().length > 0 &&
      geminiKey.trim().length > 0,
    [siteUrl, email, jiraToken, geminiKey],
  );

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (!canContinue || busy) {
      return;
    }

    setBusy(true);
    setError(null);
    try {
      await saveSetup(
        {
          site_url: siteUrl.trim(),
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
        throw new Error(parts.join(' · ') || 'Credential validation failed');
      }
      await startFullSync();
      navigate('/sync');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="page setup-page">
      <h1>Setup</h1>
      <p>Connect Jira and Gemini to start syncing analytics data.</p>
      <form onSubmit={onSubmit}>
        <label htmlFor="site-url">Site URL</label>
        <input
          id="site-url"
          name="siteUrl"
          type="url"
          autoComplete="url"
          value={siteUrl}
          onChange={(e) => setSiteUrl(e.target.value)}
        />

        <label htmlFor="email">Email</label>
        <input
          id="email"
          name="email"
          type="email"
          autoComplete="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
        />

        <label htmlFor="jira-api-token">Jira API token</label>
        <input
          id="jira-api-token"
          name="jiraApiToken"
          type="password"
          autoComplete="off"
          value={jiraToken}
          onChange={(e) => setJiraToken(e.target.value)}
        />

        <label htmlFor="gemini-api-key">Gemini API key</label>
        <input
          id="gemini-api-key"
          name="geminiApiKey"
          type="password"
          autoComplete="off"
          value={geminiKey}
          onChange={(e) => setGeminiKey(e.target.value)}
        />

        {error ? (
          <p role="alert" className="form-error">
            {error}
          </p>
        ) : null}

        <button type="submit" disabled={!canContinue || busy}>
          {busy ? 'Saving…' : 'Save and continue'}
        </button>
      </form>
    </main>
  );
}
