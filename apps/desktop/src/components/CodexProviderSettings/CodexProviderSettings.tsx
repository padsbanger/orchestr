import { KeyRound, LogIn, LogOut, RefreshCw, ShieldCheck, Square, Terminal } from "lucide-react";
import { useCallback, useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { getCodexProviderStatus, logoutCodex, startCodexLogin, testCodexConnection, type ProviderStatus } from "../../services/providers";
import { cancelLocalWorkerRun, listenToWorkerRunEvents, type WorkerRunEvent } from "../../services/workers";
import "./CodexProviderSettings.css";

type ProviderRunState = {
  status: "running" | "completed" | "failed" | "cancelled";
  output: Array<{ stream: "stdout" | "stderr"; text: string }>;
  exitCode?: number | null;
};

export function CodexProviderSettings() {
  const [provider, setProvider] = useState<ProviderStatus>();
  const [error, setError] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [activeRunId, setActiveRunId] = useState<string>();
  const [runs, setRuns] = useState<Record<string, ProviderRunState>>({});

  const refresh = useCallback(async () => {
    setIsLoading(true);
    setError(undefined);
    try {
      setProvider(await getCodexProviderStatus());
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Unable to inspect Codex.");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenToWorkerRunEvents((event) => {
      applyProviderRunEvent(event, setRuns);
      if (event.kind !== "output") void refresh();
    }).then((stopListening) => { unlisten = stopListening; });
    return () => unlisten?.();
  }, [refresh]);

  const start = async (action: "login" | "logout" | "check") => {
    setError(undefined);
    try {
      const run = action === "login" ? await startCodexLogin() : action === "logout" ? await logoutCodex() : await testCodexConnection();
      setActiveRunId(run.runId);
      setRuns((current) => current[run.runId] ? current : { ...current, [run.runId]: { status: "running", output: [] } });
    } catch (runError) {
      setError(runError instanceof Error ? runError.message : "Unable to start the Codex command.");
    }
  };

  const cancelActiveRun = async () => {
    if (!activeRunId) return;
    setError(undefined);
    try {
      await cancelLocalWorkerRun(activeRunId);
    } catch (cancelError) {
      setError(cancelError instanceof Error ? cancelError.message : "Unable to cancel the Codex command.");
    }
  };

  const activeRun = activeRunId ? runs[activeRunId] : undefined;

  return <section className="provider-settings" aria-labelledby="codex-provider-title">
    <header className="provider-settings-header">
      <div><p className="eyebrow">AI provider / local worker</p><h2 id="codex-provider-title">Codex</h2><p>{provider?.detail ?? "Inspecting Codex CLI on this worker..."}</p></div>
      <button className="icon-button" type="button" onClick={() => void refresh()} disabled={isLoading} aria-label="Refresh Codex status"><RefreshCw size={16} className={isLoading ? "spin" : undefined} /></button>
    </header>

    {error && <p className="provider-error" role="alert">{error}</p>}
    {provider && <>
      <div className="provider-status-grid">
        <StatusItem label="Installed" value={provider.installed ? "Yes" : "No"} tone={provider.installed ? "ready" : "unavailable"} />
        <StatusItem label="Version" value={provider.version ?? "Not detected"} />
        <StatusItem label="Authenticated" value={labelForAuthentication(provider.authentication)} tone={provider.authentication === "authenticated" ? "ready" : provider.authentication === "unauthenticated" ? "warning" : "unavailable"} />
        <StatusItem label="Status" value={labelForReadiness(provider.readiness)} tone={provider.readiness === "ready" ? "ready" : provider.readiness === "needs_authentication" ? "warning" : "unavailable"} />
      </div>
      <div className="provider-actions">
        {provider.installed && provider.authentication !== "authenticated" && <button className="primary-button" type="button" onClick={() => void start("login")} disabled={activeRun?.status === "running"}><LogIn size={16} /> Sign in with Codex</button>}
        {provider.installed && <button className="secondary-button" type="button" onClick={() => void start("check")} disabled={activeRun?.status === "running"}><ShieldCheck size={16} /> Test connection</button>}
        {provider.installed && provider.authentication === "authenticated" && <button className="secondary-button" type="button" onClick={() => void start("logout")} disabled={activeRun?.status === "running"}><LogOut size={16} /> Sign out</button>}
      </div>
      <p className="provider-privacy"><KeyRound size={14} /> Orchestr does not read or store Codex credentials. Sign-in is performed by the Codex CLI on this worker.</p>
    </>}

    {activeRun && <section className="provider-run" aria-label="Codex command output">
      <header><div><Terminal size={15} /><h3>Codex command</h3></div><span className={`provider-run-status ${activeRun.status}`}>{activeRun.status}</span></header>
      <div className="provider-run-actions">{activeRun.status === "running" && <button className="secondary-button" type="button" onClick={() => void cancelActiveRun()}><Square size={14} /> Cancel</button>}</div>
      <pre>{activeRun.output.length === 0 ? "Waiting for Codex output..." : activeRun.output.map((line) => `[${line.stream}] ${line.text}`).join("\n")}{activeRun.exitCode !== undefined && activeRun.exitCode !== null ? `\n\nExited with code ${activeRun.exitCode}.` : ""}</pre>
    </section>}
  </section>;
}

function StatusItem({ label, value, tone }: { label: string; value: string; tone?: "ready" | "warning" | "unavailable" }) {
  return <div className="provider-status-item"><span>{label}</span><strong className={tone ? `provider-tone ${tone}` : undefined}>{value}</strong></div>;
}

function labelForAuthentication(value: ProviderStatus["authentication"]) {
  return value === "authenticated" ? "Yes" : value === "unauthenticated" ? "No" : value === "unavailable" ? "Unavailable" : "Unknown";
}

function labelForReadiness(value: ProviderStatus["readiness"]) {
  return value === "ready" ? "Ready" : value === "needs_authentication" ? "Needs sign-in" : value === "unavailable" ? "Unavailable" : "Unknown";
}

function applyProviderRunEvent(event: WorkerRunEvent, setRuns: Dispatch<SetStateAction<Record<string, ProviderRunState>>>) {
  setRuns((current) => {
    const run = current[event.runId] ?? { status: "running" as const, output: [] };
    if (event.kind === "output" && event.stream && event.text) {
      const lastLine = run.output[run.output.length - 1];
      if (lastLine?.text === event.text) return current;
      return { ...current, [event.runId]: { ...run, output: [...run.output, { stream: event.stream, text: event.text }] } };
    }
    if (event.kind !== "output") return { ...current, [event.runId]: { ...run, status: event.kind, exitCode: event.exitCode } };
    return current;
  });
}
