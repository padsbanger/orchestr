import { Activity, Cpu, Play, RefreshCw, Square, Terminal } from "lucide-react";
import { useCallback, useEffect, useState, type Dispatch, type SetStateAction } from "react";
import {
  cancelLocalWorkerRun,
  getLocalWorkerProfile,
  listenToWorkerRunEvents,
  runLocalDiagnostic,
  type WorkerProfile,
  type WorkerRunEvent,
} from "../../services/workers";
import "./WorkersPage.css";

type RunView = {
  status: "running" | "completed" | "failed" | "cancelled";
  output: Array<{ stream: "stdout" | "stderr"; text: string }>;
  exitCode?: number | null;
};

export function WorkersPage() {
  const [worker, setWorker] = useState<WorkerProfile>();
  const [error, setError] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [activeRunId, setActiveRunId] = useState<string>();
  const [runs, setRuns] = useState<Record<string, RunView>>({});

  const loadWorker = useCallback(async () => {
    setIsLoading(true);
    setError(undefined);
    try {
      setWorker(await getLocalWorkerProfile());
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Unable to inspect the local worker.");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadWorker();
  }, [loadWorker]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToWorkerRunEvents((event) => {
      applyRunEvent(event, setRuns);
      if (event.kind !== "output") void loadWorker();
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [loadWorker]);

  const startDiagnostic = async () => {
    setError(undefined);
    try {
      const { runId } = await runLocalDiagnostic();
      setActiveRunId(runId);
      setRuns((current) => current[runId] ? current : { ...current, [runId]: { status: "running", output: [] } });
      void loadWorker();
    } catch (runError) {
      setError(runError instanceof Error ? runError.message : "Unable to start the local worker diagnostic.");
    }
  };

  const cancelDiagnostic = async () => {
    if (!activeRunId) return;
    setError(undefined);
    try {
      await cancelLocalWorkerRun(activeRunId);
    } catch (cancelError) {
      setError(cancelError instanceof Error ? cancelError.message : "Unable to cancel the local worker diagnostic.");
    }
  };

  const activeRun = activeRunId ? runs[activeRunId] : undefined;

  return (
    <section className="page workers-page">
      <header className="page-header">
        <div>
          <p className="eyebrow">Execution environments</p>
          <h1>Workers</h1>
          <p className="muted">The local worker exposes machine capabilities and executes structured commands.</p>
        </div>
        <div className="header-actions">
          <button className="icon-button" type="button" onClick={() => void loadWorker()} disabled={isLoading} aria-label="Refresh worker capabilities"><RefreshCw size={16} className={isLoading ? "spin" : undefined} /></button>
          <button className="primary-button" type="button" onClick={() => void startDiagnostic()} disabled={activeRun?.status === "running"}><Play size={16} /> Run Git diagnostic</button>
        </div>
      </header>

      {error && <div className="inline-error" role="alert">{error}</div>}
      {isLoading && !worker ? <div className="empty-state"><span className="empty-index">SYNC</span><h2>Inspecting local worker</h2></div> : worker && <>
        <section className="worker-card" aria-label="Local worker">
          <div className="worker-icon"><Cpu size={20} /></div>
          <div className="worker-title"><h2>{worker.name}</h2><p>{worker.os} / {worker.architecture}</p></div>
          <span className={worker.status === "busy" ? "worker-status busy" : "worker-status online"}><i /> {worker.status === "busy" ? "Busy" : "Ready"}</span>
        </section>

        <section className="worker-section">
          <header><div><Activity size={15} /><h2>Detected tools</h2></div><span>{worker.tools.filter((tool) => tool.installed).length} / {worker.tools.length} installed</span></header>
          <div className="tool-grid">
            {worker.tools.map((tool) => <div className={tool.installed ? "tool-capability installed" : "tool-capability"} key={tool.name}><strong>{tool.name}</strong><span>{tool.installed ? tool.version ?? "Installed" : "Not installed"}</span></div>)}
          </div>
        </section>
      </>}

      {activeRun && <section className="worker-section diagnostic-section">
        <header><div><Terminal size={15} /><h2>Diagnostic output</h2></div><span className={`run-status ${activeRun.status}`}>{activeRun.status}</span></header>
        <div className="diagnostic-command"><code>git --version</code>{activeRun.status === "running" && <button className="secondary-button" type="button" onClick={() => void cancelDiagnostic()}><Square size={14} /> Cancel</button>}</div>
        <pre className="worker-output" aria-live="polite">{activeRun.output.length === 0 ? "Waiting for worker output..." : activeRun.output.map((line, index) => `[${line.stream}] ${line.text}`).join("\n")}{activeRun.exitCode !== undefined && activeRun.exitCode !== null ? `\n\nExited with code ${activeRun.exitCode}.` : ""}</pre>
      </section>}
    </section>
  );
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
