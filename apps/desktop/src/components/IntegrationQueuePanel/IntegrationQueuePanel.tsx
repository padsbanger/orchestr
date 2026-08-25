import { GitMerge, RefreshCw, RotateCcw, Undo2, Wrench, X } from "lucide-react";
import type { IntegrationAttempt, RevertAttempt } from "../../services/integrations";
import type { Task } from "../../services/tasks";
import "./IntegrationQueuePanel.css";

type IntegrationQueuePanelProps = {
  attempts: IntegrationAttempt[];
  reverts: RevertAttempt[];
  tasks: Task[];
  isLoading: boolean;
  isIntegrating: boolean;
  recoveringIntegrationId?: string;
  revertingIntegrationId?: string;
  onClose: () => void;
  onRefresh: () => void;
  onIntegrateNext: () => void;
  onRetry: (attempt: IntegrationAttempt) => void;
  onRetryCleanup: (attempt: IntegrationAttempt) => void;
  onRevert: (attempt: IntegrationAttempt, createRepairTask: boolean) => void;
};

export function IntegrationQueuePanel({ attempts, reverts, tasks, isLoading, isIntegrating, recoveringIntegrationId, revertingIntegrationId, onClose, onRefresh, onIntegrateNext, onRetry, onRetryCleanup, onRevert }: IntegrationQueuePanelProps) {
  const taskTitles = new Map(tasks.map((task) => [task.id, task.title]));
  const queued = attempts.filter((attempt) => attempt.status === "queued");
  const active = attempts.filter((attempt) => attempt.status === "integrating");
  const revertedIntegrations = new Set(reverts.filter((attempt) => attempt.status !== "failed").map((attempt) => attempt.integrationAttemptId));

  return <aside className="board-inspector-panel integration-queue-panel" aria-label="Integration queue">
    <header className="integration-queue-header">
      <div><p className="eyebrow">Serialized delivery</p><h2>Integration queue</h2></div>
      <div className="integration-queue-header-actions"><button className="icon-button" type="button" disabled={isLoading || isIntegrating} onClick={onRefresh} aria-label="Refresh integration queue"><RefreshCw size={16} className={isLoading ? "spin" : undefined} /></button><button className="icon-button" type="button" onClick={onClose} aria-label="Close integration queue"><X size={16} /></button></div>
    </header>
    <div className="integration-queue-content">
      <section className="integration-queue-overview"><div><span>Waiting</span><strong>{queued.length}</strong></div><div><span>Active</span><strong>{active.length}</strong></div><div><span>Strategy</span><code>Squash</code></div></section>
      <p className="integration-queue-hint">One approved task is integrated at a time. Interrupted work is returned to a recoverable state after restart.</p>
      <button className="primary-button" type="button" disabled={isIntegrating || queued.length === 0} onClick={onIntegrateNext}><GitMerge size={15} /> {isIntegrating ? "Integrating..." : "Integrate next"}</button>
      {attempts.length === 0 ? <p className="integration-queue-empty">No integration attempts recorded.</p> : <ol className="integration-attempt-list">{attempts.map((attempt) => {
        const isRecovering = recoveringIntegrationId === attempt.id;
        const isReverting = revertingIntegrationId === attempt.id;
        const canRevert = attempt.status === "merged" && Boolean(attempt.mergeCommit) && !revertedIntegrations.has(attempt.id);
        return <li key={attempt.id}>
          <div className="integration-attempt-main"><span className={`integration-status ${attempt.status}`}>{attempt.status.replaceAll("_", " ")}</span><strong>{taskTitles.get(attempt.taskId) ?? attempt.taskId}</strong><code>{attempt.sourceBranch} → {attempt.targetBranch}</code>{attempt.error && <p>{attempt.error}</p>}</div>
          <div className="integration-attempt-actions">
            {(attempt.status === "conflict" || attempt.status === "failed") && <button className="secondary-button" type="button" disabled={isIntegrating} onClick={() => onRetry(attempt)}><RotateCcw size={14} /> Retry</button>}
            {attempt.status === "merged" && attempt.error && <button className="secondary-button" type="button" disabled={isRecovering} onClick={() => onRetryCleanup(attempt)}><Wrench size={14} /> {isRecovering ? "Cleaning..." : "Retry cleanup"}</button>}
            {canRevert && <><button className="secondary-button" type="button" disabled={isReverting} onClick={() => onRevert(attempt, false)}><Undo2 size={14} /> {isReverting ? "Reverting..." : "Revert"}</button><button className="integration-repair-action" type="button" disabled={isReverting} onClick={() => onRevert(attempt, true)}>Revert + repair task</button></>}
          </div>
        </li>;
      })}</ol>}
      {reverts.length > 0 && <section className="revert-history"><div className="revert-history-heading"><h3>Revert history</h3><span>{reverts.length}</span></div><ol>{reverts.map((revert) => <li key={revert.id}><span className={`revert-status ${revert.status}`}>{revert.status.replaceAll("_", " ")}</span><strong>{taskTitles.get(revert.originalTaskId) ?? revert.originalTaskId}</strong><code>{revert.originalCommit.slice(0, 8)}{revert.revertCommit ? ` → ${revert.revertCommit.slice(0, 8)}` : ""}</code>{revert.repairTaskId && <p>Repair task created: {taskTitles.get(revert.repairTaskId) ?? revert.repairTaskId}</p>}{revert.error && <p>{revert.error}</p>}</li>)}</ol></section>}
    </div>
  </aside>;
}
