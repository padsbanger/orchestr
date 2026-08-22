export function SettingsPage() {
  return (
    <section className="page">
      <header className="page-header">
        <div>
          <p className="eyebrow">Local configuration</p>
          <h1>Settings</h1>
          <p className="muted">Settings are stored in Orchestr's local SQLite database.</p>
        </div>
      </header>
      <div className="settings-list">
        <div className="setting-row">
          <div>
            <h2>Storage</h2>
            <p>Metadata stays on this machine; source repositories remain user-owned.</p>
          </div>
          <code>sqlite / orchestr.db</code>
        </div>
        <div className="setting-row">
          <div>
            <h2>Providers and workers</h2>
            <p>These will be introduced after the local Kanban and Git foundation.</p>
          </div>
          <span className="status-chip">Not configured</span>
        </div>
      </div>
    </section>
  );
}

