import { Gauge, RefreshCw, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import type { Agent } from "../../services/agents";
import type { FlowLimitInput, FlowState } from "../../services/flow";
import type { Task } from "../../services/tasks";
import "./FlowControlPanel.css";

type FlowControlPanelProps = {
  flow?: FlowState;
  tasks: Task[];
  agents: Agent[];
  isLoading: boolean;
  isSaving: boolean;
  onClose: () => void;
  onRefresh: () => void;
  onSave: (limits: FlowLimitInput) => void;
  onCancel: (runId: string) => void;
};

export function FlowControlPanel({ flow, tasks, agents, isLoading, isSaving, onClose, onRefresh, onSave, onCancel }: FlowControlPanelProps) {
  const [limits, setLimits] = useState<FlowLimitInput>(() => valuesFrom(flow));
  useEffect(() => setLimits(valuesFrom(flow)), [flow]);
  const taskTitles = new Map(tasks.map((task) => [task.id, task.title]));
  const agentNames = new Map(agents.map((agent) => [agent.id, agent.name]));

  return <aside className="board-inspector-panel flow-control-panel" aria-label="Execution flow control">
    <header className="flow-control-header">
      <div><p className="eyebrow">Bounded local concurrency</p><h2>Flow control</h2></div>
      <div className="flow-control-header-actions"><button className="icon-button" type="button" disabled={isLoading} onClick={onRefresh} aria-label="Refresh flow control"><RefreshCw size={16} className={isLoading ? "spin" : undefined} /></button><button className="icon-button" type="button" onClick={onClose} aria-label="Close flow control"><X size={16} /></button></div>
    </header>
    <div className="flow-control-content">
      <section className="flow-control-overview">
        <FlowMeter label="Worker" value={flow?.activeWorkerRuns ?? 0} limit={flow?.limits.workerMaxConcurrentRuns ?? limits.workerMaxConcurrentRuns} />
        <FlowMeter label="In Progress" value={flow?.inProgress ?? 0} limit={flow?.limits.inProgressLimit ?? limits.inProgressLimit} />
        <FlowMeter label="Review" value={flow?.review ?? 0} limit={flow?.limits.reviewLimit ?? limits.reviewLimit} />
        <FlowMeter label="Approved" value={(flow?.approved ?? 0) + (flow?.integrating ?? 0)} limit={flow?.limits.approvedLimit ?? limits.approvedLimit} />
      </section>
      {flow?.blockedReason ? <p className="flow-pressure"><Gauge size={14} /> {flow.blockedReason} Queued work will resume automatically when capacity opens.</p> : <p className="flow-control-hint">Capacity is available. Priority selects among eligible queued tasks; dependencies still require Done.</p>}
      <form className="flow-limit-form" onSubmit={(event) => { event.preventDefault(); onSave(limits); }}>
        <h3>Limits</h3>
        <div className="flow-limit-grid">
          <LimitInput label="Local worker" value={limits.workerMaxConcurrentRuns} onChange={(value) => setLimits((current) => ({ ...current, workerMaxConcurrentRuns: value }))} />
          <LimitInput label="In Progress" value={limits.inProgressLimit} onChange={(value) => setLimits((current) => ({ ...current, inProgressLimit: value }))} />
          <LimitInput label="Review" value={limits.reviewLimit} onChange={(value) => setLimits((current) => ({ ...current, reviewLimit: value }))} />
          <LimitInput label="Approved + Integrating" value={limits.approvedLimit} onChange={(value) => setLimits((current) => ({ ...current, approvedLimit: value }))} />
        </div>
        <button className="secondary-button" type="submit" disabled={isSaving}>{isSaving ? "Saving..." : "Apply limits"}</button>
      </form>
      <section>
        <div className="flow-queue-title"><h3>Execution queue</h3><span>{flow?.queued ?? 0}</span></div>
        {!flow || flow.queue.length === 0 ? <p className="flow-control-empty">No implementation runs are waiting.</p> : <ol className="flow-run-list">{flow.queue.map((run, index) => <li key={run.id}><span>{String(index + 1).padStart(2, "0")}</span><div><strong>{taskTitles.get(run.taskId) ?? run.taskId}</strong><code>{agentNames.get(run.agentId) ?? run.agentId}</code></div><button className="icon-button" type="button" onClick={() => onCancel(run.id)} aria-label={`Cancel queued run for ${taskTitles.get(run.taskId) ?? run.taskId}`}><Square size={13} /></button></li>)}</ol>}
      </section>
    </div>
  </aside>;
}

function FlowMeter({ label, value, limit }: { label: string; value: number; limit: number }) {
  const constrained = value >= limit;
  return <div className={constrained ? "constrained" : undefined}><span>{label}</span><strong>{value}<small> / {limit}</small></strong></div>;
}

function LimitInput({ label, value, onChange }: { label: string; value: number; onChange: (value: number) => void }) {
  return <label>{label}<input type="number" min="1" max="32" value={value} onChange={(event) => onChange(Number(event.target.value))} required /></label>;
}

function valuesFrom(flow?: FlowState): FlowLimitInput {
  return {
    workerMaxConcurrentRuns: flow?.limits.workerMaxConcurrentRuns ?? 4,
    inProgressLimit: flow?.limits.inProgressLimit ?? 4,
    reviewLimit: flow?.limits.reviewLimit ?? 3,
    approvedLimit: flow?.limits.approvedLimit ?? 2,
  };
}
