import { useEffect, useState, type ReactNode } from 'react';
import { Navigate, Route, Routes, useLocation } from 'react-router-dom';
import { hasCredentials } from './lib/tauri';
import EpicsPage from './pages/EpicsPage';
import ExplorePage from './pages/ExplorePage';
import FlowPage from './pages/FlowPage';
import HomePage from './pages/HomePage';
import SetupPage from './pages/SetupPage';
import SprintsPage from './pages/SprintsPage';
import SyncPage from './pages/SyncPage';

function RequireCredentials({ children }: { children: ReactNode }) {
  const location = useLocation();
  const [ready, setReady] = useState(false);
  const [configured, setConfigured] = useState(false);

  useEffect(() => {
    let active = true;
    void hasCredentials().then((ok) => {
      if (active) {
        setConfigured(ok);
        setReady(true);
      }
    });
    return () => {
      active = false;
    };
  }, [location.pathname]);

  if (!ready) {
    return (
      <main className="page">
        <p>Checking setup…</p>
      </main>
    );
  }

  if (!configured) {
    return <Navigate to="/setup" replace />;
  }

  return children;
}

export default function App() {
  return (
    <Routes>
      <Route path="/setup" element={<SetupPage />} />
      <Route
        path="/sync"
        element={
          <RequireCredentials>
            <SyncPage />
          </RequireCredentials>
        }
      />
      <Route
        path="/flow"
        element={
          <RequireCredentials>
            <FlowPage />
          </RequireCredentials>
        }
      />
      <Route
        path="/sprints"
        element={
          <RequireCredentials>
            <SprintsPage />
          </RequireCredentials>
        }
      />
      <Route
        path="/epics"
        element={
          <RequireCredentials>
            <EpicsPage />
          </RequireCredentials>
        }
      />
      <Route
        path="/explore"
        element={
          <RequireCredentials>
            <ExplorePage />
          </RequireCredentials>
        }
      />
      <Route
        path="/"
        element={
          <RequireCredentials>
            <HomePage />
          </RequireCredentials>
        }
      />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
