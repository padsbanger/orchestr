import { GitMerge, RefreshCw, RotateCcw, X } from "lucide-react";
import type { IntegrationAttempt } from "../../services/integrations";
import type { Task } from "../../services/tasks";
import "./IntegrationQueuePanel.css";

type IntegrationQueuePanelProps = {
  attempts: IntegrationAttempt[];
  tasks: Task[];
  isLoading: boolean;
  isIntegrating: boolean;
  onClose: () => void;
  onRefresh: () => void;
  onIntegrateNext: () => void;
  onRetry: (attempt: IntegrationAttempt) => void;
};

export function IntegrationQueuePanel({ attempts, tasks, isLoading, isIntegrating, onClose, onRefresh, onIntegrateNext, onRetry }: IntegrationQueuePanelProps) {
  const taskTitles = new Map(tasks.map((task) => [task.id, task.title]));
  const queued = attempts.filter((attempt) => attempt.status === "queued");
  const active = attempts.filter((attempt) => attempt.status === "integrating");

  return <aside className="integration-queue-panel" aria-label="Integration queue">
    <header className="integration-queue-header">
      <div><p className="eyebrow">Serialized delivery</p><h2>Integration queue</h2></div>
      <div className="integration-queue-header-actions"><button className="icon-button" type="button" disabled={isLoading || isIntegrating} onClick={onRefresh} aria-label="Refresh integration queue"><RefreshCw size={16} className={isLoading ? "spin" : undefined} /></button><button className="icon-button" type="button" onClick={onClose} aria-label="Close integration queue"><X size={16} /></button></div>
    </header>
    <div className="integration-queue-content">
      <section className="integration-queue-overview"><div><span>Waiting</span><strong>{queued.length}</strong></div><div><span>Active</span><strong>{active.length}</strong></div><div><span>Strategy</span><code>Squash</code></div></section>
      <p className="integration-queue-hint">One approved task is integrated at a time. Done means its accepted changes are on the integration branch.</p>
      <button className="primary-button" type="button" disabled={isIntegrating || queued.length === 0} onClick={onIntegrateNext}><GitMerge size={15} /> {isIntegrating ? "Integrating..." : "Integrate next"}</button>
      {attempts.length === 0 ? <p className="integration-queue-empty">No approved tasks are waiting for integration.</p> : <ol className="integration-attempt-list">{attempts.map((attempt) => <li key={attempt.id}><div className="integration-attempt-main"><span className={`integration-status ${attempt.status}`}>{attempt.status.replaceAll("_", " ")}</span><strong>{taskTitles.get(attempt.taskId) ?? attempt.taskId}</strong><code>{attempt.sourceBranch} → {attempt.targetBranch}</code>{attempt.error && <p>{attempt.error}</p>}</div>{(attempt.status === "conflict" || attempt.status === "failed") && <button className="secondary-button" type="button" disabled={isIntegrating} onClick={() => onRetry(attempt)}><RotateCcw size={14} /> Retry</button>}</li>)}</ol>}
    </div>
  </aside>;
}
