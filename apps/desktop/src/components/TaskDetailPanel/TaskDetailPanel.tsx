import { Bot, CheckSquare, Code2, FileCode2, GitBranch, Pencil, X } from "lucide-react";
import type { ReactNode } from "react";
import type { Agent } from "../../services/agents";
import type { Task } from "../../services/tasks";
import "./TaskDetailPanel.css";

type TaskDetailPanelProps = {
  task: Task;
  assignedAgent?: Agent;
  onClose: () => void;
  onEdit: (task: Task) => void;
};

export function TaskDetailPanel({ task, assignedAgent, onClose, onEdit }: TaskDetailPanelProps) {
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
      </div>
    </aside>
  );
}

function TaskSection({ title, icon, count, children }: { title: string; icon?: ReactNode; count?: number; children: ReactNode }) {
  return <section className="task-detail-section"><h3>{icon}{title}{count !== undefined && <span>{count}</span>}</h3>{children}</section>;
}
