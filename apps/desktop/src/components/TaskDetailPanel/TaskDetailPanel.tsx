import { Bot, CheckSquare, Code2, FileCode2, GitBranch, Pencil, Play, Square, Terminal, X } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Agent } from "../../services/agents";
import type { TaskRun } from "../../services/runs";
import type { Task } from "../../services/tasks";
import "./TaskDetailPanel.css";

type TaskDetailPanelProps = {
  task: Task;
  assignedAgent?: Agent;
  runs: TaskRun[];
  isStartingRun: boolean;
  onClose: () => void;
  onEdit: (task: Task) => void;
  onStartRun: () => void;
  onCancelRun: (runId: string) => void;
};

export function TaskDetailPanel({ task, assignedAgent, runs, isStartingRun, onClose, onEdit, onStartRun, onCancelRun }: TaskDetailPanelProps) {
  const activeRun = runs.find((run) => run.status === "running");
  const latestRun = activeRun ?? runs[0];
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!activeRun) return undefined;
    const interval = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [activeRun]);

  const canStart = Boolean(assignedAgent) && task.status === "todo" && !activeRun;
  return (
    <aside className="task-detail-panel" aria-label={`Task details for ${task.title}`}>
      <header className="task-detail-header">
        <div><p className="eyebrow">Task specification</p><h2>{task.title}</h2><code>{task.id}</code></div>
        <div className="task-detail-actions">
          <button className="icon-button" type="button" onClick={() => onEdit(task)} aria-label={`Edit ${task.title}`}><Pencil size={16} /></button>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close task details"><X size={16} /></button>
        </div>
      </header>
      <div className="task-detail-content">
        <TaskSection title="Context">
          <p className="task-detail-copy">{task.description || "No additional context has been recorded."}</p>
        </TaskSection>
        <TaskSection title="Acceptance criteria" icon={<CheckSquare size={14} />} count={task.acceptanceCriteria.length}>
          {task.acceptanceCriteria.length === 0 ? <p className="task-detail-empty">No acceptance criteria recorded.</p> : <ul className="criteria-list">{task.acceptanceCriteria.map((criterion) => <li key={criterion}>{criterion}</li>)}</ul>}
        </TaskSection>
        <TaskSection title="Implementation notes" icon={<Code2 size={14} />}>
          <p className="task-detail-copy">{task.implementationNotes || "No implementation notes recorded."}</p>
        </TaskSection>
        <TaskSection title="Relevant paths / context" icon={<FileCode2 size={14} />} count={task.relevantPaths.length}>
          {task.relevantPaths.length === 0 ? <p className="task-detail-empty">No relevant paths recorded.</p> : <ul className="token-list">{task.relevantPaths.map((path) => <li key={path}><code>{path}</code></li>)}</ul>}
        </TaskSection>
        <TaskSection title="Dependencies" icon={<GitBranch size={14} />} count={task.dependencyIds.length}>
          {task.dependencyIds.length === 0 ? <p className="task-detail-empty">No task dependencies recorded.</p> : <><ul className="token-list">{task.dependencyIds.map((dependency) => <li key={dependency}><code>{dependency}</code></li>)}</ul><p className="task-detail-hint">Dependency blocking will be activated in a later workflow milestone.</p></>}
        </TaskSection>
        <TaskSection title="Assigned agent" icon={<Bot size={14} />}>
          {assignedAgent ? <p className="task-detail-copy"><strong>{assignedAgent.name}</strong><br />{assignedAgent.role}{assignedAgent.model ? ` / ${assignedAgent.model}` : ""}</p> : <p className="task-detail-empty">No agent assigned.</p>}
        </TaskSection>
        <TaskSection title="Execution" icon={<Terminal size={14} />}>
          <div className="task-run-actions">
            <button className="primary-button" type="button" disabled={!canStart || isStartingRun} onClick={onStartRun}><Play size={15} /> {isStartingRun ? "Starting..." : "Run with Codex"}</button>
            {activeRun && <button className="secondary-button" type="button" onClick={() => onCancelRun(activeRun.id)}><Square size={14} /> Cancel</button>}
          </div>
          {!assignedAgent ? <p className="task-detail-hint">Assign a Codex agent before starting this task.</p> : task.status !== "todo" && !activeRun ? <p className="task-detail-hint">Only Todo tasks can be started. Successful runs are sent to Review for human approval.</p> : <p className="task-detail-hint">Codex works in the local project workspace. Successful runs move the task to Review.</p>}
          {latestRun ? <RunSummary run={latestRun} now={now} /> : <p className="task-detail-empty">No runs recorded for this task.</p>}
        </TaskSection>
      </div>
    </aside>
  );
}

function TaskSection({ title, icon, count, children }: { title: string; icon?: ReactNode; count?: number; children: ReactNode }) {
  return <section className="task-detail-section"><h3>{icon}{title}{count !== undefined && <span>{count}</span>}</h3>{children}</section>;
}

function RunSummary({ run, now }: { run: TaskRun; now: number }) {
  const runtimeEnd = run.completedAt ? timestamp(run.completedAt) : now;
  const runtime = Math.max(0, runtimeEnd - timestamp(run.startedAt));
  const outputRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    outputRef.current?.scrollTo({ top: outputRef.current.scrollHeight });
  }, [run.id, run.output.length]);

  return <div className="task-run-summary">
    <div className="task-run-meta"><span className={`run-status ${run.status}`}>{run.status}</span><span>{formatDuration(runtime)}</span>{run.exitCode !== null && <span>exit {run.exitCode}</span>}</div>
    {run.error && <p className="task-run-error">{run.error}</p>}
    <pre ref={outputRef} className="task-run-output" aria-live="polite">{run.output.length === 0 ? "Waiting for Codex output..." : run.output.map((line) => `[${line.stream}] ${line.text}`).join("\n\n")}</pre>
  </div>;
}

function timestamp(value: string) { return Date.parse(value.endsWith("Z") ? value : `${value}Z`); }

function formatDuration(milliseconds: number) {
  const seconds = Math.floor(milliseconds / 1_000);
  return `${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, "0")}s`;
}
