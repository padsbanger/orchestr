import { RefreshCw, ShieldAlert, X } from "lucide-react";
import { useState, type FormEvent } from "react";
import type { ProjectBlocker } from "../../services/interruptions";
import type { Task } from "../../services/tasks";
import "./ProjectBlockersPanel.css";

type BlockerInput = {
  title: string;
  description?: string;
  affectsAllTasks: boolean;
  affectedTaskIds: string[];
};

type Props = {
  blockers: ProjectBlocker[];
  tasks: Task[];
  isLoading: boolean;
  isSaving: boolean;
  resolvingId?: string;
  onClose: () => void;
  onRefresh: () => void;
  onCreate: (input: BlockerInput) => void;
  onResolve: (blockerId: string) => void;
};

export function ProjectBlockersPanel({ blockers, tasks, isLoading, isSaving, resolvingId, onClose, onRefresh, onCreate, onResolve }: Props) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [affectsAllTasks, setAffectsAllTasks] = useState(true);
  const [affectedTaskIds, setAffectedTaskIds] = useState<string[]>([]);
  const active = blockers.filter((blocker) => blocker.status === "active");
  const resolved = blockers.filter((blocker) => blocker.status === "resolved");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    onCreate({ title, description: description || undefined, affectsAllTasks, affectedTaskIds });
    setTitle("");
    setDescription("");
    setAffectedTaskIds([]);
  };

  const toggleTask = (taskId: string) => {
    setAffectedTaskIds((current) => current.includes(taskId) ? current.filter((id) => id !== taskId) : [...current, taskId]);
  };

  return <aside className="board-inspector-panel project-blockers-panel" aria-label="Project blockers">
    <header className="project-blockers-header">
      <div><p className="eyebrow">Flow interruption</p><h2>Project blockers</h2></div>
      <div className="project-blockers-header-actions"><button className="icon-button" type="button" disabled={isLoading} onClick={onRefresh} aria-label="Refresh blockers"><RefreshCw size={15} /></button><button className="icon-button" type="button" onClick={onClose} aria-label="Close project blockers"><X size={16} /></button></div>
    </header>
    <div className="project-blockers-content">
      <div className="project-blocker-summary"><ShieldAlert size={17} /><div><strong>{active.length} active</strong><p>Blocked tasks are removed from automatic scheduling until the shared issue is resolved.</p></div></div>
      <form className="project-blocker-form" onSubmit={submit}>
        <h3>Record blocker</h3>
        <label>Title<input value={title} required onChange={(event) => setTitle(event.target.value)} placeholder="Required SDK unavailable" /></label>
        <label>Context<textarea rows={3} value={description} onChange={(event) => setDescription(event.target.value)} placeholder="What is unavailable, who owns it, and what changes when resolved?" /></label>
        <label className="project-blocker-all"><input type="checkbox" checked={affectsAllTasks} onChange={(event) => setAffectsAllTasks(event.target.checked)} /> Pause all project tasks</label>
        {!affectsAllTasks && <fieldset className="project-blocker-task-picker"><legend>Affected tasks</legend>{tasks.filter((task) => !["done", "integrating"].includes(task.status)).map((task) => <label key={task.id}><input type="checkbox" checked={affectedTaskIds.includes(task.id)} onChange={() => toggleTask(task.id)} /><span>{task.title}</span><code>{task.status}</code></label>)}</fieldset>}
        <button className="primary-button" type="submit" disabled={isSaving || !title.trim() || (!affectsAllTasks && affectedTaskIds.length === 0)}>{isSaving ? "Recording..." : "Record blocker"}</button>
      </form>
      <BlockerList title="Active blockers" blockers={active} tasks={tasks} resolvingId={resolvingId} onResolve={onResolve} />
      {resolved.length > 0 && <BlockerList title="Resolved history" blockers={resolved} tasks={tasks} />}
    </div>
  </aside>;
}

function BlockerList({ title, blockers, tasks, resolvingId, onResolve }: { title: string; blockers: ProjectBlocker[]; tasks: Task[]; resolvingId?: string; onResolve?: (id: string) => void }) {
  return <section className="project-blocker-list"><h3>{title}<span>{blockers.length}</span></h3>{blockers.length === 0 ? <p className="project-blocker-empty">No blockers in this state.</p> : blockers.map((blocker) => <article key={blocker.id} className={blocker.status}>
    <header><strong>{blocker.title}</strong><span>{blocker.status}</span></header>
    {blocker.description && <p>{blocker.description}</p>}
    <small>{blocker.affectsAllTasks ? "All project tasks" : blocker.affectedTaskIds.map((id) => tasks.find((task) => task.id === id)?.title ?? id).join(", ")}</small>
    {onResolve && <button className="secondary-button" type="button" disabled={resolvingId === blocker.id} onClick={() => onResolve(blocker.id)}>{resolvingId === blocker.id ? "Resolving..." : "Resolve blocker"}</button>}
  </article>)}</section>;
}
