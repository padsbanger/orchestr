import { Plus, Server, TerminalSquare } from "lucide-react";

export function DashboardPage() {
  return (
    <section className="page dashboard-page">
      <header className="page-header">
        <div>
          <p className="eyebrow">Control plane / local</p>
          <h1>Projects</h1>
          <p className="muted">Repositories and task boards will appear here.</p>
        </div>
        <button className="primary-button" disabled title="Available in M1">
          <Plus size={16} /> New project
        </button>
      </header>

      <div className="foundation-grid">
        <article className="foundation-card">
          <TerminalSquare size={18} />
          <div>
            <h2>Desktop foundation online</h2>
            <p>React, Tauri, and the local SQLite store are connected.</p>
          </div>
        </article>
        <article className="foundation-card">
          <Server size={18} />
          <div>
            <h2>Local-first by default</h2>
            <p>No worker, provider, or remote connection has been configured.</p>
          </div>
        </article>
      </div>

      <div className="empty-state">
        <span className="empty-index">M1</span>
        <h2>Your project registry is empty</h2>
        <p>Create or register a local Git repository in the next milestone.</p>
      </div>
    </section>
  );
}

