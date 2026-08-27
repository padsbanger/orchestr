import { Check, MessagesSquare, RefreshCw, X } from "lucide-react";
import { useState, type FormEvent } from "react";
import type { Agent } from "../../services/agents";
import type { CollaborationEntry, CollaborationKind } from "../../services/collaboration";
import type { Task } from "../../services/tasks";
import "./CollaborationPanel.css";

type Props = {
  entries: CollaborationEntry[];
  tasks: Task[];
  agents: Agent[];
  isLoading: boolean;
  isSaving: boolean;
  actionId?: string;
  onClose: () => void;
  onRefresh: () => void;
  onCreate: (input: { taskId?: string; parentId?: string; kind: CollaborationKind; message: string; referencedTaskIds: string[] }) => void;
  onResolve: (entryId: string) => void;
};

const kinds: { value: CollaborationKind; label: string }[] = [
  { value: "comment", label: "Comment" }, { value: "request", label: "Request" },
  { value: "blocker", label: "Blocker" }, { value: "interface_change", label: "Interface change" },
  { value: "escalation", label: "Escalation" },
];

export function CollaborationPanel(props: Props) {
  const [kind, setKind] = useState<CollaborationKind>("comment");
  const [message, setMessage] = useState("");
  const [taskId, setTaskId] = useState("");
  const [references, setReferences] = useState<string[]>([]);
  const [showResolved, setShowResolved] = useState(false);
  const roots = props.entries.filter((entry) => !entry.parentId && (showResolved || entry.status === "open"));

  const submit = (event: FormEvent) => {
    event.preventDefault();
    props.onCreate({ taskId: taskId || undefined, kind, message: message.trim(), referencedTaskIds: references });
    setMessage(""); setReferences([]);
  };

  return <aside className="board-inspector-panel collaboration-panel" aria-label="Agent collaboration">
    <header className="collaboration-header"><div><p className="eyebrow">Auditable coordination</p><h2>Agent collaboration</h2></div><div><button className="icon-button" type="button" disabled={props.isLoading} onClick={props.onRefresh} aria-label="Refresh collaboration"><RefreshCw size={15} /></button><button className="icon-button" type="button" onClick={props.onClose} aria-label="Close collaboration"><X size={16} /></button></div></header>
    <div className="collaboration-content">
      <section className="collaboration-summary"><MessagesSquare size={18} /><div><strong>One visible coordination channel</strong><p>Agent comments, requests, blockers, interface changes, and escalations are persisted here and supplied to relevant future runs.</p></div></section>
      <form className="collaboration-form" onSubmit={submit}>
        <div className="collaboration-form-row"><label>Type<select value={kind} onChange={(event) => setKind(event.target.value as CollaborationKind)}>{kinds.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}</select></label><label>Primary task<select value={taskId} onChange={(event) => setTaskId(event.target.value)}><option value="">Project-wide</option>{props.tasks.map((task) => <option key={task.id} value={task.id}>{task.title}</option>)}</select></label></div>
        <label>Message<textarea required rows={4} value={message} onChange={(event) => setMessage(event.target.value)} placeholder="Record the decision, request, contract change, or escalation with enough context for another agent." /></label>
        <fieldset><legend>Reference tasks</legend><div className="collaboration-reference-picker">{props.tasks.map((task) => <label key={task.id}><input type="checkbox" checked={references.includes(task.id)} onChange={() => setReferences((current) => current.includes(task.id) ? current.filter((id) => id !== task.id) : [...current, task.id])} /><span>{task.title}</span></label>)}</div></fieldset>
        <button className="primary-button" type="submit" disabled={props.isSaving || !message.trim()}>{props.isSaving ? "Recording..." : "Record activity"}</button>
      </form>
      <div className="collaboration-list-header"><h3>Threads <span>{roots.length}</span></h3><label><input type="checkbox" checked={showResolved} onChange={(event) => setShowResolved(event.target.checked)} /> Show resolved</label></div>
      <section className="collaboration-list">{props.isLoading && props.entries.length === 0 ? <p>Loading activity...</p> : roots.length === 0 ? <p>No collaboration threads in this view.</p> : roots.map((entry) => <CollaborationThread key={entry.id} entry={entry} replies={props.entries.filter((reply) => reply.parentId === entry.id)} tasks={props.tasks} agents={props.agents} actionId={props.actionId} onCreate={props.onCreate} onResolve={props.onResolve} />)}</section>
    </div>
  </aside>;
}

function CollaborationThread({ entry, replies, tasks, agents, actionId, onCreate, onResolve }: { entry: CollaborationEntry; replies: CollaborationEntry[]; tasks: Task[]; agents: Agent[]; actionId?: string; onCreate: Props["onCreate"]; onResolve: Props["onResolve"] }) {
  const [reply, setReply] = useState("");
  const task = tasks.find((item) => item.id === entry.taskId);
  const referenced = tasks.filter((item) => entry.referencedTaskIds.includes(item.id));
  const author = entry.authorType === "human" ? "Human operator" : agents.find((agent) => agent.id === entry.authorAgentId)?.name ?? entry.authorType;
  return <article className={`collaboration-thread ${entry.kind} ${entry.status}`}><header><span>{entry.kind.replace("_", " ")}</span><code>{entry.status}</code></header><p>{entry.message}</p><small>{author} · {dateTime(entry.createdAt)}{task ? ` · ${task.title}` : " · Project-wide"}</small>{referenced.length > 0 && <div className="collaboration-references">References: {referenced.map((item) => item.title).join(", ")}</div>}
    {replies.length > 0 && <div className="collaboration-replies">{replies.map((item) => <div key={item.id}><p>{item.message}</p><small>{item.authorType === "human" ? "Human operator" : agents.find((agent) => agent.id === item.authorAgentId)?.name ?? item.authorType} · {dateTime(item.createdAt)}</small></div>)}</div>}
    {entry.status === "open" && <footer><textarea rows={2} value={reply} onChange={(event) => setReply(event.target.value)} placeholder="Reply in this thread..." /><div><button className="secondary-button" type="button" disabled={!reply.trim() || actionId === entry.id} onClick={() => { onCreate({ taskId: entry.taskId ?? undefined, parentId: entry.id, kind: "comment", message: reply.trim(), referencedTaskIds: [] }); setReply(""); }}>Reply</button><button className="secondary-button" type="button" disabled={actionId === entry.id} onClick={() => onResolve(entry.id)}><Check size={14} /> Resolve thread</button></div></footer>}
  </article>;
}

function dateTime(value: string) {
  const date = new Date(value.includes("T") ? value : `${value.replace(" ", "T")}Z`);
  return Number.isNaN(date.valueOf()) ? value : date.toLocaleString();
}
