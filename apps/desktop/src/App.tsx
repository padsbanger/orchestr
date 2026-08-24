import { useEffect, useState } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./components/AppShell/AppShell";
import { DashboardPage } from "./pages/DashboardPage/DashboardPage";
import { BoardPage } from "./pages/BoardPage/BoardPage";
import { ProjectProgressPage } from "./pages/ProjectProgressPage/ProjectProgressPage";
import { SettingsPage } from "./pages/SettingsPage/SettingsPage";
import { WorkersPage } from "./pages/WorkersPage/WorkersPage";
import { AgentsPage } from "./pages/AgentsPage/AgentsPage";
import { getSetting, setSetting } from "./services/settings";

const SIDEBAR_SETTING = "ui.sidebar.collapsed";

export function App() {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [persistenceError, setPersistenceError] = useState<string>();

  useEffect(() => {
    void getSetting(SIDEBAR_SETTING).then((value) => {
      setSidebarCollapsed(value === "true");
    }).catch((error: unknown) => {
      setPersistenceError(error instanceof Error ? error.message : "Unable to open local settings.");
    });
  }, []);

  const toggleSidebar = () => {
    const next = !sidebarCollapsed;
    setSidebarCollapsed(next);
    void setSetting(SIDEBAR_SETTING, String(next)).catch((error: unknown) => {
      setSidebarCollapsed(!next);
      setPersistenceError(error instanceof Error ? error.message : "Unable to save local settings.");
    });
  };

  return (
    <AppShell
      sidebarCollapsed={sidebarCollapsed}
      onToggleSidebar={toggleSidebar}
      persistenceError={persistenceError}
      onDismissError={() => setPersistenceError(undefined)}
    >
      <Routes>
        <Route path="/projects" element={<DashboardPage />} />
        <Route path="/projects/:projectId/progress" element={<ProjectProgressPage />} />
        <Route path="/projects/:projectId" element={<BoardPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="/workers" element={<WorkersPage />} />
        <Route path="/agents" element={<AgentsPage />} />
        <Route path="*" element={<Navigate to="/projects" replace />} />
      </Routes>
    </AppShell>
  );
}
