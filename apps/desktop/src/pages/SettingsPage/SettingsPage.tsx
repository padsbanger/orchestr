import { CodexProviderSettings } from "../../components/CodexProviderSettings/CodexProviderSettings";
import "./SettingsPage.css";

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
      </div>
      <CodexProviderSettings />
    </section>
  );
}
