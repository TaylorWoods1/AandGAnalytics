import { useEffect, useState } from 'react';
import DashboardNav from '../components/DashboardNav';
import ContextPackPreview from '../components/ContextPackPreview';
import FilterBar from '../components/FilterBar';
import {
  askAi,
  emptyMetricsFilter,
  getSuggestedPrompts,
  previewContextPack,
  type ContextPack,
  type AiAnswer,
  type MetricsFilter,
} from '../lib/tauri';

export default function AskAiPage() {
  const [filter, setFilter] = useState<MetricsFilter>(emptyMetricsFilter);
  const [question, setQuestion] = useState('');
  const [prompts, setPrompts] = useState<string[]>([]);
  const [pack, setPack] = useState<ContextPack | null>(null);
  const [packLoading, setPackLoading] = useState(false);
  const [packError, setPackError] = useState<string | null>(null);
  const [answer, setAnswer] = useState<AiAnswer | null>(null);
  const [asking, setAsking] = useState(false);
  const [askError, setAskError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void getSuggestedPrompts()
      .then((list) => {
        if (active) {
          setPrompts(list);
        }
      })
      .catch(() => {
        if (active) {
          setPrompts([]);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    setPackLoading(true);
    setPackError(null);
    void previewContextPack(filter)
      .then((data) => {
        if (active) {
          setPack(data);
          setPackLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setPack(null);
          setPackLoading(false);
          setPackError(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      active = false;
    };
  }, [filter]);

  async function handleAsk() {
    setAsking(true);
    setAskError(null);
    setAnswer(null);
    try {
      const result = await askAi(filter, question);
      setAnswer(result);
    } catch (err: unknown) {
      setAskError(err instanceof Error ? err.message : String(err));
    } finally {
      setAsking(false);
    }
  }

  return (
    <main className="page dashboard-page">
      <header className="dashboard-header">
        <h1>Ask AI</h1>
        <DashboardNav current="ask" />
      </header>

      <p>Ask questions over a local context pack. Dashboards keep working if Bedrock fails.</p>

      <FilterBar value={filter} onChange={setFilter} />

      <ContextPackPreview pack={pack} loading={packLoading} error={packError} />

      {prompts.length > 0 ? (
        <section className="suggested-prompts" aria-label="Suggested prompts">
          <h2>Suggested prompts</h2>
          <ul>
            {prompts.map((prompt) => (
              <li key={prompt}>
                <button type="button" className="linkish" onClick={() => setQuestion(prompt)}>
                  {prompt}
                </button>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <form
        className="ask-ai-form"
        onSubmit={(e) => {
          e.preventDefault();
          void handleAsk();
        }}
      >
        <label htmlFor="ask-question">Question</label>
        <textarea
          id="ask-question"
          rows={3}
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          placeholder="What is our biggest bottleneck?"
        />
        <button type="submit" disabled={asking || question.trim() === ''}>
          Ask
        </button>
      </form>

      {askError ? (
        <p className="form-error" role="alert">
          {askError}
        </p>
      ) : null}

      {asking ? <p>Asking Bedrock…</p> : null}

      {answer ? (
        <section className="ai-answer" aria-label="AI answer">
          <h2>Answer</h2>
          <p>{answer.text}</p>
          {answer.citations.length > 0 ? (
            <>
              <h3>Citations</h3>
              <ul>
                {answer.citations.map((cite) => (
                  <li key={cite}>{cite}</li>
                ))}
              </ul>
            </>
          ) : null}
        </section>
      ) : null}
    </main>
  );
}
