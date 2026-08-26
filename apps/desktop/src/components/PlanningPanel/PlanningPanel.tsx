import { Bot, Check, RefreshCw, Sparkles, X } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";
import type { Agent } from "../../services/agents";
import type { PlanningProposal, PlanningTask } from "../../services/planning";
import "./PlanningPanel.css";

type Props = {
  proposals: PlanningProposal[];
  agents: Agent[];
  isLoading: boolean;
  isStarting: boolean;
  actionId?: string;
  onClose: () => void;
  onRefresh: () => void;
  onStart: (agentId: string, goal: string) => void;
  onApprove: (proposalId: string) => void;
  onReject: (proposalId: string) => void;
  onCancel: (proposalId: string) => void;
};

export function PlanningPanel(props: Props) {
  const [goal, setGoal] = useState("");
  const [agentId, setAgentId] = useState("");

  useEffect(() => {
    if (!agentId && props.agents[0]) setAgentId(props.agents[0].id);
  }, [agentId, props.agents]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    props.onStart(agentId, goal.trim());
    setGoal("");
  };

  return <aside className="board-inspector-panel planning-panel" aria-label="Planning agent proposals">
    <header className="planning-header">
      <div><p className="eyebrow">Human-approved decomposition</p><h2>Planning agent</h2></div>
      <div><button className="icon-button" type="button" disabled={props.isLoading} onClick={props.onRefresh} aria-label="Refresh plans"><RefreshCw size={15} /></button><button className="icon-button" type="button" onClick={props.onClose} aria-label="Close planning"><X size={16} /></button></div>
    </header>
    <div className="planning-content">
      <section className="planning-summary"><Sparkles size={18} /><div><strong>Draft first, create work after approval</strong><p>The planner inspects the repository and accepted ADRs in read-only mode. Its milestone, epic, tasks, priorities, criteria, and dependencies remain a proposal until you approve them.</p></div></section>
      <form className="planning-form" onSubmit={submit}>
        <label>Planning agent<select required value={agentId} onChange={(event) => setAgentId(event.target.value)}><option value="">Select an agent...</option>{props.agents.map((agent) => <option key={agent.id} value={agent.id}>{agent.name} · {agent.model || "default model"}</option>)}</select></label>
        <label>Project goal<textarea required rows={5} value={goal} onChange={(event) => setGoal(event.target.value)} placeholder="Add GitHub OAuth authentication, including callback handling, session persistence, and accessible sign-in UI." /></label>
        <button className="primary-button" type="submit" disabled={props.isStarting || !agentId || !goal.trim()}><Bot size={15} /> {props.isStarting ? "Starting planner..." : "Generate plan"}</button>
        {props.agents.length === 0 && <small>Create a Codex agent before generating a plan.</small>}
      </form>
      <section className="planning-proposals"><h3>Proposal history <span>{props.proposals.length}</span></h3>
        {props.isLoading && props.proposals.length === 0 ? <p className="planning-empty">Loading plans...</p> : props.proposals.length === 0 ? <p className="planning-empty">No plans yet. Describe a project outcome above.</p> : props.proposals.map((proposal) => <ProposalCard key={proposal.id} proposal={proposal} actionId={props.actionId} onApprove={props.onApprove} onReject={props.onReject} onCancel={props.onCancel} />)}
      </section>
    </div>
  </aside>;
}

function ProposalCard({ proposal, actionId, onApprove, onReject, onCancel }: { proposal: PlanningProposal; actionId?: string; onApprove: Props["onApprove"]; onReject: Props["onReject"]; onCancel: Props["onCancel"] }) {
  const pending = actionId === proposal.id;
  return <article className={`planning-proposal ${proposal.status}`}>
    <header><span>{proposal.status.replace("_", " ")}</span><time>{formatDate(proposal.createdAt)}</time></header>
    <h4>{proposal.goal}</h4>
    {proposal.status === "generating" && <div className="planning-running"><span /><p>Planner is inspecting project context and building the dependency graph.</p></div>}
    {proposal.error && <p className="planning-error">{proposal.error}</p>}
    {proposal.plan && <PlanPreview proposal={proposal} />}
    {proposal.rawOutput && <details className="planning-transcript"><summary>Planner transcript</summary><pre>{proposal.rawOutput}</pre></details>}
    {proposal.status === "proposed" && <footer><button className="primary-button" type="button" disabled={pending} onClick={() => onApprove(proposal.id)}><Check size={14} /> {pending ? "Applying..." : "Approve and create work"}</button><button className="secondary-button" type="button" disabled={pending} onClick={() => onReject(proposal.id)}>Reject</button></footer>}
    {proposal.status === "generating" && <footer><button className="secondary-button" type="button" disabled={pending} onClick={() => onCancel(proposal.id)}>{pending ? "Cancelling..." : "Cancel"}</button></footer>}
    {proposal.status === "approved" && <p className="planning-created"><Check size={14} /> Created {proposal.taskIds.length} task{proposal.taskIds.length === 1 ? "" : "s"}{proposal.milestoneId ? ", milestone" : ""}{proposal.epicId ? ", and epic" : ""}.</p>}
  </article>;
}

function PlanPreview({ proposal }: { proposal: PlanningProposal }) {
  const plan = proposal.plan!;
  return <div className="planning-plan">
    <p>{plan.summary}</p>
    {(plan.milestone || plan.epic) && <div className="planning-outcomes">{plan.milestone && <div><small>Milestone</small><strong>{plan.milestone.title}</strong>{plan.milestone.description && <p>{plan.milestone.description}</p>}</div>}{plan.epic && <div><small>Epic</small><strong>{plan.epic.title}</strong>{plan.epic.description && <p>{plan.epic.description}</p>}</div>}</div>}
    <div className="planning-task-list">{plan.tasks.map((task, index) => <TaskPreview key={task.key} task={task} index={index} />)}</div>
  </div>;
}

function TaskPreview({ task, index }: { task: PlanningTask; index: number }) {
  return <article><header><code>{String(index + 1).padStart(2, "0")}</code><span className={`priority ${task.priority}`}>{task.priority}</span></header><strong>{task.title}</strong>{task.description && <p>{task.description}</p>}<ul>{task.acceptanceCriteria.map((criterion) => <li key={criterion}>{criterion}</li>)}</ul>{task.dependencyKeys.length > 0 && <small>Depends on: {task.dependencyKeys.join(", ")}</small>}{task.requiredCapabilities.length > 0 && <small>Capabilities: {task.requiredCapabilities.join(", ")}</small>}</article>;
}

function formatDate(value: string) {
  const date = new Date(value.includes("T") ? value : `${value.replace(" ", "T")}Z`);
  return Number.isNaN(date.valueOf()) ? value : date.toLocaleString();
}
