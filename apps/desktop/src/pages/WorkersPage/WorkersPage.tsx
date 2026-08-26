import { Activity, Cpu, Play, Plus, RefreshCw, Server, Square, Terminal, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState, type Dispatch, type FormEvent, type SetStateAction } from "react";
import { errorMessage, runConfirmedDestructiveAction } from "../../services/confirmations";
import { listProjects, type Project } from "../../services/projects";
import {
  cancelLocalWorkerRun,
  deleteRemoteWorker,
  getLocalWorkerProfile,
  listenToWorkerRunEvents,
  listRemoteWorkers,
  refreshRemoteWorker,
  registerRemoteWorker,
  runLocalDiagnostic,
  type RemoteWorker,
  type WorkerProfile,
  type WorkerRunEvent,
} from "../../services/workers";
import "./WorkersPage.css";

type RunView = {
  status: "running" | "completed" | "failed" | "cancelled";
  output: Array<{ stream: "stdout" | "stderr"; text: string }>;
  exitCode?: number | null;
};

type RegistrationForm = {
  endpoint: string;
  tokenEnvironmentVariable: string;
  caCertificatePath: string;
  projectId: string;
  workspacePath: string;
};

const emptyRegistration: RegistrationForm = {
  endpoint: "https://",
  tokenEnvironmentVariable: "ORCHESTR_REMOTE_TOKEN",
  caCertificatePath: "",
  projectId: "",
  workspacePath: "",
};

export function WorkersPage() {
  const [worker, setWorker] = useState<WorkerProfile>();
  const [remoteWorkers, setRemoteWorkers] = useState<RemoteWorker[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [registration, setRegistration] = useState(emptyRegistration);
  const [error, setError] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [isRegistering, setIsRegistering] = useState(false);
  const [refreshingId, setRefreshingId] = useState<string>();
  const [activeRunId, setActiveRunId] = useState<string>();
  const [runs, setRuns] = useState<Record<string, RunView>>({});

  const loadWorkers = useCallback(async () => {
    setIsLoading(true);
    setError(undefined);
    try {
      const [localWorker, registeredWorkers, availableProjects] = await Promise.all([
        getLocalWorkerProfile(),
        listRemoteWorkers(),
        listProjects(),
      ]);
      setWorker(localWorker);
      setRemoteWorkers(registeredWorkers);
      setProjects(availableProjects);
      setRegistration((current) => ({
        ...current,
        projectId: current.projectId || availableProjects[0]?.id || "",
      }));
    } catch (loadError) {
      setError(errorMessage(loadError, "Unable to inspect workers."));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadWorkers();
  }, [loadWorkers]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToWorkerRunEvents((event) => {
      applyRunEvent(event, setRuns);
      if (event.kind !== "output") void loadWorkers();
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [loadWorkers]);

  const submitRegistration = async (event: FormEvent) => {
    event.preventDefault();
    setError(undefined);
    setIsRegistering(true);
    try {
      await registerRemoteWorker({
        endpoint: registration.endpoint,
        tokenEnvironmentVariable: registration.tokenEnvironmentVariable,
        caCertificatePath: registration.caCertificatePath || undefined,
        projectId: registration.projectId,
        workspacePath: registration.workspacePath,
      });
      setRegistration((current) => ({ ...emptyRegistration, projectId: current.projectId }));
      await loadWorkers();
    } catch (registrationError) {
      setError(errorMessage(registrationError, "Unable to register the remote worker."));
    } finally {
      setIsRegistering(false);
    }
  };

  const heartbeatWorker = async (workerId: string) => {
    setRefreshingId(workerId);
    setError(undefined);
    try {
      const refreshed = await refreshRemoteWorker(workerId);
      setRemoteWorkers((current) => current.map((item) => item.id === refreshed.id ? refreshed : item));
    } catch (refreshError) {
      setError(errorMessage(refreshError, "The remote worker did not answer the heartbeat."));
      await loadWorkers();
    } finally {
      setRefreshingId(undefined);
    }
  };

  const removeWorker = async (remoteWorker: RemoteWorker) => {
    setError(undefined);
    try {
      const removed = await runConfirmedDestructiveAction({
        title: "Remove remote worker?",
        message: `Remove ${remoteWorker.name} from Orchestr? This does not delete files on that machine.`,
        confirmLabel: "Remove worker",
      }, () => deleteRemoteWorker(remoteWorker.id));
      if (removed) setRemoteWorkers((current) => current.filter((item) => item.id !== remoteWorker.id));
    } catch (removeError) {
      setError(errorMessage(removeError, "Unable to remove the remote worker."));
    }
  };

  const startDiagnostic = async () => {
    setError(undefined);
    try {
      const { runId } = await runLocalDiagnostic();
      setActiveRunId(runId);
      setRuns((current) => current[runId] ? current : { ...current, [runId]: { status: "running", output: [] } });
      void loadWorkers();
    } catch (runError) {
      setError(errorMessage(runError, "Unable to start the local worker diagnostic."));
    }
  };

  const cancelDiagnostic = async () => {
    if (!activeRunId) return;
    setError(undefined);
    try {
      await cancelLocalWorkerRun(activeRunId);
    } catch (cancelError) {
      setError(errorMessage(cancelError, "Unable to cancel the local worker diagnostic."));
    }
  };

  const activeRun = activeRunId ? runs[activeRunId] : undefined;

  return (
    <section className="page workers-page">
      <header className="page-header">
        <div>
          <p className="eyebrow">Execution environments</p>
          <h1>Workers</h1>
          <p className="muted">Register authenticated machines, inspect capabilities, and route projects to them.</p>
        </div>
        <div className="header-actions">
          <button className="icon-button" type="button" onClick={() => void loadWorkers()} disabled={isLoading} aria-label="Refresh worker capabilities"><RefreshCw size={16} className={isLoading ? "spin" : undefined} /></button>
          <button className="primary-button" type="button" onClick={() => void startDiagnostic()} disabled={activeRun?.status === "running"}><Play size={16} /> Run local diagnostic</button>
        </div>
      </header>

      {error && <div className="inline-error" role="alert">{error}</div>}
      {isLoading && !worker ? <div className="empty-state"><span className="empty-index">SYNC</span><h2>Inspecting workers</h2></div> : worker && <>
        <section className="worker-card" aria-label="Local worker">
          <div className="worker-icon"><Cpu size={20} /></div>
          <div className="worker-title"><h2>{worker.name}</h2><p>Local · {worker.os} / {worker.architecture}</p></div>
          <span className={worker.status === "busy" ? "worker-status busy" : "worker-status online"}><i /> {worker.status === "busy" ? "Busy" : "Ready"}</span>
        </section>

        <section className="worker-section">
          <header><div><Activity size={15} /><h2>Local capabilities</h2></div><span>{worker.tools.filter((tool) => tool.installed).length} / {worker.tools.length} installed</span></header>
          <ToolGrid tools={worker.tools} />
        </section>
      </>}

      <section className="worker-section remote-worker-section">
        <header><div><Server size={15} /><h2>Remote workers</h2></div><span>{remoteWorkers.filter((item) => item.status === "online").length} online</span></header>
        <form className="remote-worker-form" onSubmit={(event) => void submitRegistration(event)}>
          <label>HTTPS endpoint<input required type="url" value={registration.endpoint} onChange={(event) => setRegistration((current) => ({ ...current, endpoint: event.target.value }))} placeholder="https://worker.example:9443" /></label>
          <label>Token environment variable<input required value={registration.tokenEnvironmentVariable} onChange={(event) => setRegistration((current) => ({ ...current, tokenEnvironmentVariable: event.target.value }))} /></label>
          <label>Custom CA certificate path <span>optional</span><input value={registration.caCertificatePath} onChange={(event) => setRegistration((current) => ({ ...current, caCertificatePath: event.target.value }))} placeholder="C:\certs\worker-ca.pem" /></label>
          <label>Project<select required value={registration.projectId} onChange={(event) => setRegistration((current) => ({ ...current, projectId: event.target.value }))}><option value="">Select project</option>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label>
          <label className="workspace-field">Remote workspace path<input required value={registration.workspacePath} onChange={(event) => setRegistration((current) => ({ ...current, workspacePath: event.target.value }))} placeholder="\\worker\projects\my-project" /><small>The path must be inside the worker's allowed roots and reachable by this desktop for Git review and integration.</small></label>
          <button className="primary-button register-worker-button" type="submit" disabled={isRegistering || projects.length === 0}><Plus size={15} /> {isRegistering ? "Connecting…" : "Register worker"}</button>
        </form>

        {remoteWorkers.length === 0 ? <div className="remote-empty">No remote workers registered.</div> : <div className="remote-worker-list">
          {remoteWorkers.map((remoteWorker) => <article className="remote-worker-card" key={remoteWorker.id}>
            <div className="remote-worker-heading">
              <div className="worker-icon remote"><Server size={19} /></div>
              <div className="worker-title"><h2>{remoteWorker.name}</h2><p>{remoteWorker.endpoint} · protocol v{remoteWorker.protocolVersion}</p></div>
              <span className={`worker-status ${remoteWorker.status}`}><i /> {remoteWorker.status}</span>
              <div className="remote-worker-actions">
                <button className="icon-button" type="button" onClick={() => void heartbeatWorker(remoteWorker.id)} disabled={refreshingId === remoteWorker.id} aria-label={`Refresh ${remoteWorker.name}`}><RefreshCw size={15} className={refreshingId === remoteWorker.id ? "spin" : undefined} /></button>
                <button className="icon-button danger" type="button" onClick={() => void removeWorker(remoteWorker)} aria-label={`Remove ${remoteWorker.name}`}><Trash2 size={15} /></button>
              </div>
            </div>
            <div className="remote-worker-meta"><span>{remoteWorker.os} / {remoteWorker.architecture}</span><span>Token: ${remoteWorker.tokenEnvironmentVariable}</span><span>Last seen: {new Date(remoteWorker.lastSeenAt).toLocaleString()}</span></div>
            <div className="remote-workspaces">{remoteWorker.workspaces.map((workspace) => <span key={workspace.projectId}>{projects.find((project) => project.id === workspace.projectId)?.name ?? workspace.projectId}<code>{workspace.workspacePath}</code></span>)}</div>
            <ToolGrid tools={remoteWorker.tools} />
          </article>)}
        </div>}
      </section>

      {activeRun && <section className="worker-section diagnostic-section">
        <header><div><Terminal size={15} /><h2>Diagnostic output</h2></div><span className={`run-status ${activeRun.status}`}>{activeRun.status}</span></header>
        <div className="diagnostic-command"><code>git --version</code>{activeRun.status === "running" && <button className="secondary-button" type="button" onClick={() => void cancelDiagnostic()}><Square size={14} /> Cancel</button>}</div>
        <pre className="worker-output" aria-live="polite">{activeRun.output.length === 0 ? "Waiting for worker output..." : activeRun.output.map((line) => `[${line.stream}] ${line.text}`).join("\n")}{activeRun.exitCode !== undefined && activeRun.exitCode !== null ? `\n\nExited with code ${activeRun.exitCode}.` : ""}</pre>
      </section>}
    </section>
  );
}

function ToolGrid({ tools }: { tools: WorkerProfile["tools"] }) {
  return <div className="tool-grid">{tools.map((tool) => <div className={tool.installed ? "tool-capability installed" : "tool-capability"} key={tool.name}><strong>{tool.name}</strong><span>{tool.installed ? tool.version ?? "Installed" : "Not installed"}</span></div>)}</div>;
}

function applyRunEvent(event: WorkerRunEvent, setRuns: Dispatch<SetStateAction<Record<string, RunView>>>) {
  setRuns((current) => {
    const run = current[event.runId] ?? { status: "running" as const, output: [] };
    if (event.kind === "output" && event.text && event.stream) {
      return { ...current, [event.runId]: { ...run, output: [...run.output, { stream: event.stream, text: event.text }] } };
    }
    if (event.kind !== "output") {
      return { ...current, [event.runId]: { ...run, status: event.kind, exitCode: event.exitCode } };
    }
    return current;
  });
}
