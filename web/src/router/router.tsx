import { Suspense, useEffect, useState } from 'react';
import { Navigate, Route, Routes } from 'react-router-dom';
import Layout from '../components/layout/Layout';
import { Spinner } from '../components/ui/spinner';
import {
  AcpConsole,
  AgentChat,
  AgentWorkspaceExplorer,
  AgentsList,
  Canvas,
  Config,
  Cron,
  Dashboard,
  Doctor,
  Integrations,
  Logs,
  Pairing,
  Quickstart,
  RunDetail,
  Roles,
  Runs,
  Skills,
  SopEditor,
  SopView,
  SopsList,
  Tools,
  Users,
} from './lazyPages';

// Lazy route chunks usually resolve in a few ms, so a spinner shown
// immediately just flickers on every section switch. Hold it back: render
// nothing for the first 250ms and only surface the bolt if the load is
// genuinely slow (a cold chunk, a throttled network).
function RouteFallback() {
  const [show, setShow] = useState(false);
  useEffect(() => {
    const id = window.setTimeout(() => setShow(true), 250);
    return () => window.clearTimeout(id);
  }, []);
  if (!show) return null;
  return (
    <div className="min-h-[60vh] flex items-center justify-center">
      <Spinner size={32} className="text-muted-foreground" />
    </div>
  );
}

export const Router = () => (
  <Suspense fallback={<RouteFallback />}>
    <Routes>
      <Route element={<Layout />}>
        <Route path="/" element={<Dashboard />} />
        <Route path="/agent" element={<Navigate to="/agents" replace />} />
        <Route path="/agents" element={<AgentsList />} />
        <Route path="/agent/:alias" element={<AgentChat />} />
        <Route path="/agent/:alias/workspace" element={<AgentWorkspaceExplorer />} />
        <Route path="/tools" element={<Tools />} />
        <Route path="/cron" element={<Cron />} />
        <Route path="/skills" element={<Skills />} />
        <Route path="/sops" element={<SopsList />} />
        <Route path="/sops/new" element={<SopEditor />} />
        <Route path="/sops/:name" element={<SopView />} />
        <Route path="/sops/:name/edit" element={<SopEditor />} />
        <Route path="/runs" element={<Runs />} />
        <Route path="/runs/:sop/:runId" element={<RunDetail />} />
        <Route path="/integrations" element={<Integrations />} />
        <Route path="/memory" element={<Navigate to="/?tab=memories" replace />} />
        <Route path="/config" element={<Config />} />
        <Route path="/config/:section" element={<Config />} />
        <Route path="/config/:section/:type" element={<Config />} />
        <Route path="/config/:section/:type/:alias" element={<Config />} />
        <Route path="/setup/:section" element={<Config />} />
        <Route path="/logs" element={<Logs />} />
        <Route path="/doctor" element={<Doctor />} />
        <Route path="/pairing" element={<Pairing />} />
        <Route path="/roles" element={<Roles />} />
        <Route path="/users" element={<Users />} />
        <Route path="/canvas" element={<Canvas />} />
        <Route path="/acp-console" element={<AcpConsole />} />
        <Route path="/quickstart" element={<Quickstart />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  </Suspense>
)
