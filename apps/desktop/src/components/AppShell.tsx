import { FolderKanban, PanelLeftClose, PanelLeftOpen, Settings2 } from "lucide-react";
import type { PropsWithChildren } from "react";
import { NavLink } from "react-router-dom";

type AppShellProps = PropsWithChildren<{
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
  persistenceError?: string;
  onDismissError: () => void;
}>;

export function AppShell({
  children,
  sidebarCollapsed,
  onToggleSidebar,
  persistenceError,
  onDismissError,
}: AppShellProps) {
  return (
    <div className={sidebarCollapsed ? "app-shell sidebar-collapsed" : "app-shell"}>
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand">
          <img className="brand-mark" src="/icon.png" alt="Orchestr" />
          <span className="brand-name">ORCHESTR</span>
        </div>
        <nav className="navigation">
          <NavLink to="/projects" className={({ isActive }) => `nav-item ${isActive ? "active" : ""}`}>
            <FolderKanban size={16} />
            <span>Projects</span>
          </NavLink>
          <NavLink to="/settings" className={({ isActive }) => `nav-item ${isActive ? "active" : ""}`}>
            <Settings2 size={16} />
            <span>Settings</span>
          </NavLink>
        </nav>
        <div className="sidebar-footer">
          <span className="local-status"><i /> Local control plane</span>
          <button className="icon-button" onClick={onToggleSidebar} aria-label="Toggle sidebar">
            {sidebarCollapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
          </button>
        </div>
      </aside>
      <main className="content">
        {persistenceError && (
          <div className="error-banner" role="alert">
            <span>{persistenceError}</span>
            <button onClick={onDismissError}>Dismiss</button>
          </div>
        )}
        {children}
      </main>
    </div>
  );
}
