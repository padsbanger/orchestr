import { BookOpenCheck, RefreshCw, X } from "lucide-react";
import { useState, type FormEvent } from "react";
import type { ArchitectureDecision, ArchitectureDecisionInput } from "../../services/knowledge";
import type { Task } from "../../services/tasks";
import "./ProjectKnowledgePanel.css";

type Props = {
  decisions: ArchitectureDecision[];
  tasks: Task[];
  previewTaskId?: string;
  previewDecisions: ArchitectureDecision[];
  isLoading: boolean;
  isPreviewLoading: boolean;
  isSaving: boolean;
  decidingId?: string;
  onClose: () => void;
  onRefresh: () => void;
  onPreviewTask: (taskId?: string) => void;
  onCreate: (input: Omit<ArchitectureDecisionInput, "projectId">) => void;
  onDecide: (decisionId: string, status: "accepted" | "rejected") => void;
};

type Relevance = "project" | "paths" | "tasks";

export function ProjectKnowledgePanel(props: Props) {
  const [title, setTitle] = useState("");
  const [context, setContext] = useState("");
  const [decision, setDecision] = useState("");
  const [consequences, setConsequences] = useState("");
  const [supersedesDecisionId, setSupersedesDecisionId] = useState("");
  const [relevance, setRelevance] = useState<Relevance>("project");
  const [paths, setPaths] = useState("");
  const [taskIds, setTaskIds] = useState<string[]>([]);
  const accepted = props.decisions.filter((item) => item.status === "accepted");
  const proposed = props.decisions.filter((item) => item.status === "proposed");
  const history = props.decisions.filter((item) => ["superseded", "rejected"].includes(item.status));

  const submit = (event: FormEvent) => {
    event.preventDefault();
    props.onCreate({
      title,
      context,
      decision,
      consequences: consequences || undefined,
      supersedesDecisionId: supersedesDecisionId || undefined,
      relevantPaths: relevance === "paths" ? paths.split("\n").map((path) => path.trim()).filter(Boolean) : [],
      relevantTaskIds: relevance === "tasks" ? taskIds : [],
    });
    setTitle("");
    setContext("");
    setDecision("");
    setConsequences("");
    setSupersedesDecisionId("");
    setPaths("");
    setTaskIds([]);
  };

  const toggleTask = (taskId: string) => {
    setTaskIds((current) => current.includes(taskId) ? current.filter((id) => id !== taskId) : [...current, taskId]);
  };

  const relevanceMissing = (relevance === "paths" && !paths.trim()) || (relevance === "tasks" && taskIds.length === 0);

  return <aside className="board-inspector-panel project-knowledge-panel" aria-label="Architecture decisions and project knowledge">
    <header className="project-knowledge-header">
      <div><p className="eyebrow">Durable project memory</p><h2>Architecture decisions</h2></div>
      <div><button className="icon-button" type="button" disabled={props.isLoading} onClick={props.onRefresh} aria-label="Refresh architecture decisions"><RefreshCw size={15} /></button><button className="icon-button" type="button" onClick={props.onClose} aria-label="Close project knowledge"><X size={16} /></button></div>
    </header>
    <div className="project-knowledge-content">
      <section className="knowledge-source-summary"><BookOpenCheck size={18} /><div><strong>Managed + repository context</strong><p>Accepted ADRs are injected alongside task specifications, AGENTS.md, architecture docs, coding standards, repository instructions, and declared skills.</p></div></section>
      <section className="knowledge-preview">
        <div><h3>Task context preview</h3><span>{props.previewDecisions.length} accepted</span></div>
        <select value={props.previewTaskId ?? ""} onChange={(event) => props.onPreviewTask(event.target.value || undefined)}><option value="">Select a task...</option>{props.tasks.map((task) => <option key={task.id} value={task.id}>{task.title}</option>)}</select>
        {props.isPreviewLoading ? <p>Loading relevant knowledge...</p> : props.previewTaskId && (props.previewDecisions.length > 0 ? <div className="knowledge-preview-list">{props.previewDecisions.map((item) => <article key={item.id}><code>{adrLabel(item)}</code><strong>{item.title}</strong><p>{item.decision}</p></article>)}</div> : <p>No accepted managed ADRs apply. Repository instructions remain authoritative.</p>)}
      </section>
      <form className="architecture-decision-form" onSubmit={submit}>
        <h3>Propose decision</h3>
        <label>Title<input required value={title} onChange={(event) => setTitle(event.target.value)} placeholder="Use Tauri for the desktop shell" /></label>
        <label>Context<textarea required rows={3} value={context} onChange={(event) => setContext(event.target.value)} placeholder="What forces and constraints require a decision?" /></label>
        <label>Decision<textarea required rows={3} value={decision} onChange={(event) => setDecision(event.target.value)} placeholder="What is the authoritative technical direction?" /></label>
        <label>Consequences<textarea rows={2} value={consequences} onChange={(event) => setConsequences(event.target.value)} placeholder="Tradeoffs and follow-up constraints" /></label>
        <label>Replaces accepted ADR<select value={supersedesDecisionId} onChange={(event) => setSupersedesDecisionId(event.target.value)}><option value="">None</option>{accepted.map((item) => <option key={item.id} value={item.id}>{adrLabel(item)} {item.title}</option>)}</select></label>
        <fieldset className="knowledge-relevance"><legend>Relevant to</legend>{(["project", "paths", "tasks"] as Relevance[]).map((value) => <label key={value}><input type="radio" name="adr-relevance" checked={relevance === value} onChange={() => setRelevance(value)} /> {value === "project" ? "Entire project" : value === "paths" ? "Repository paths" : "Specific tasks"}</label>)}</fieldset>
        {relevance === "paths" && <label>Paths<textarea rows={3} value={paths} onChange={(event) => setPaths(event.target.value)} placeholder={"src/auth\ncrates/orchestr-db"} /><small>One repository-relative file or directory per line. Directory scopes include descendants.</small></label>}
        {relevance === "tasks" && <fieldset className="knowledge-task-picker"><legend>Tasks</legend>{props.tasks.map((task) => <label key={task.id}><input type="checkbox" checked={taskIds.includes(task.id)} onChange={() => toggleTask(task.id)} /><span>{task.title}</span><code>{task.status}</code></label>)}</fieldset>}
        <button className="primary-button" type="submit" disabled={props.isSaving || !title.trim() || !context.trim() || !decision.trim() || relevanceMissing}>{props.isSaving ? "Recording..." : "Record proposal"}</button>
      </form>
      <DecisionList title="Proposals" decisions={proposed} decidingId={props.decidingId} onDecide={props.onDecide} />
      <DecisionList title="Accepted decisions" decisions={accepted} />
      {history.length > 0 && <DecisionList title="Decision history" decisions={history} />}
    </div>
  </aside>;
}

function DecisionList({ title, decisions, decidingId, onDecide }: { title: string; decisions: ArchitectureDecision[]; decidingId?: string; onDecide?: Props["onDecide"] }) {
  return <section className="architecture-decision-list"><h3>{title}<span>{decisions.length}</span></h3>{decisions.length === 0 ? <p className="architecture-decision-empty">No decisions in this state.</p> : decisions.map((item) => <article className={item.status} key={item.id}>
    <header><code>{adrLabel(item)}</code><span>{item.status}</span></header><h4>{item.title}</h4><div><strong>Context</strong><p>{item.context}</p></div><div><strong>Decision</strong><p>{item.decision}</p></div>{item.consequences && <div><strong>Consequences</strong><p>{item.consequences}</p></div>}<small>{decisionScope(item)}</small>
    {onDecide && <footer><button className="primary-button" type="button" disabled={decidingId === item.id} onClick={() => onDecide(item.id, "accepted")}>{decidingId === item.id ? "Saving..." : "Accept"}</button><button className="secondary-button" type="button" disabled={decidingId === item.id} onClick={() => onDecide(item.id, "rejected")}>Reject</button></footer>}
  </article>)}</section>;
}

function adrLabel(decision: ArchitectureDecision) {
  return `ADR-${String(decision.decisionNumber).padStart(3, "0")}`;
}

function decisionScope(decision: ArchitectureDecision) {
  if (decision.relevantPaths.length > 0) return `Paths: ${decision.relevantPaths.join(", ")}`;
  if (decision.relevantTaskIds.length > 0) return `${decision.relevantTaskIds.length} scoped task${decision.relevantTaskIds.length === 1 ? "" : "s"}`;
  return "Entire project";
}
