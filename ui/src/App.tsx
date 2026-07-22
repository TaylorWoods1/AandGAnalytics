import { useEffect, useState, type ReactNode } from 'react';
import { Navigate, Route, Routes, useLocation } from 'react-router-dom';
import { hasCredentials } from './lib/tauri';
import SetupPage from './pages/SetupPage';
import SyncPage from './pages/SyncPage';

function HomePlaceholder() {
  return (
    <main className="page">
      <h1>AandG Analytics</h1>
      <p>Dashboards will appear here after sync.</p>
    </main>
  );
}

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
        path="/"
        element={
          <RequireCredentials>
            <HomePlaceholder />
          </RequireCredentials>
        }
      />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
