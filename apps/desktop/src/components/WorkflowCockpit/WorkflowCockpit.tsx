import { useDroppable } from "@dnd-kit/core";
import { SortableContext, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { AlertTriangle, ArrowLeft, Bot, CheckCircle2, ChevronDown, ChevronRight, CircleUserRound, Clock3, GripVertical, MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { Agent } from "../../services/agents";
import type { Task, TaskStatus } from "../../services/tasks";
import { parseWorkflowTimestamp, type AgentActivityItem, type AttentionItem, type WorkflowActorKind, type WorkflowStage, type WorkflowTaskView } from "../../services/workflow";

const taskTones: Record<TaskStatus, string> = {
  backlog: "neutral", ready: "blue", in_progress: "amber", needs_input: "yellow", review: "violet",
  approved: "indigo", integrating: "cyan", blocked: "orange", done: "green",
};

export type WorkflowTaskActions = {
  onInspect: (task: Task) => void;
  onEdit: (task: Task) => void;
  onDelete: (task: Task) => void;
  onPlanningState: (task: Task, status: "backlog" | "ready") => void;
};

export function useWorkflowClock() {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const interval = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(interval);
  }, []);
  return now;
}

export function AttentionTray({ items, expanded, isLoading = false, snapshotError, onToggle, onOpen }: { items: AttentionItem[]; expanded: boolean; isLoading?: boolean; snapshotError?: string; onToggle: () => void; onOpen: (item: AttentionItem) => void }) {
  const visibleItems = expanded ? items : items.slice(0, 5);
  return (
    <section className={`attention-tray ${items.length > 0 ? "has-items" : "is-clear"}`} aria-label="Attention">
      <div className="attention-heading"><AlertTriangle size={15} /><strong>Attention</strong><span>{items.length}</span></div>
      <div className="attention-items">
        {snapshotError && <div className="attention-sync-error" role="status"><AlertTriangle size={13} />{snapshotError}</div>}
        {isLoading && !snapshotError && <div className="attention-sync-state">Refreshing workflow status...</div>}
        {visibleItems.map((item) => <button key={item.id} className={`attention-item ${item.severity}`} type="button" onClick={() => onOpen(item)}>
          <span className="attention-kind">{formatAttentionKind(item.kind)}</span>
          <strong>{item.title}</strong>
          {item.detail && <span className="attention-detail">{item.detail}</span>}
          <ChevronRight size={14} aria-hidden="true" />
        </button>)}
        {items.length === 0 && !isLoading && !snapshotError && <div className="attention-clear"><CheckCircle2 size={14} /> No human action needed</div>}
      </div>
      {items.length > 5 && <button className="attention-expand" type="button" aria-expanded={expanded} onClick={onToggle}>{expanded ? "Show less" : `View all ${items.length}`}<ChevronDown size={14} /></button>}
    </section>
  );
}

type FlowStageColumnProps = {
  stage: WorkflowStage;
  label: string;
  totalCount: number;
  taskViews: WorkflowTaskView[];
  tasksById: ReadonlyMap<string, Task>;
  activeOnMobile: boolean;
  showAllDone: boolean;
  now: number;
  onToggleDone: () => void;
  recentlyTransitionedTaskIds: string[];
} & WorkflowTaskActions;

export function FlowStageColumn(props: FlowStageColumnProps) {
  const { stage, label, totalCount, taskViews, tasksById, activeOnMobile, showAllDone, now, onToggleDone, recentlyTransitionedTaskIds, ...actions } = props;
  const taskPairs = taskViews.flatMap((view) => {
    const task = tasksById.get(view.id);
    return task ? [{ task, view }] : [];
  });
  const sortedPairs = stage === "done"
    ? [...taskPairs].sort((left, right) => parseWorkflowTimestamp(right.view.statusChangedAt) - parseWorkflowTimestamp(left.view.statusChangedAt))
    : taskPairs;
  const visiblePairs = stage === "done" && !showAllDone ? sortedPairs.slice(0, 10) : sortedPairs;
  return (
    <section className={`flow-stage flow-stage-${stage} ${activeOnMobile ? "is-mobile-active" : ""}`}>
      <header className="flow-stage-header"><div><span className={`stage-index stage-index-${stage}`} /><h2>{label}</h2></div><span>{totalCount}</span></header>
      {stage === "verify" && <div className="verify-stepper" aria-label="Verification progress"><span>Review</span><ChevronRight size={11} /><span>Queued</span><ChevronRight size={11} /><span>Integrating</span></div>}
      <div className="flow-stage-list">
        {stage === "queue" ? <>
          <QueueTaskGroup label="Ready" status="ready" pairs={visiblePairs.filter(({ task }) => task.status === "ready")} now={now} recentlyTransitionedTaskIds={recentlyTransitionedTaskIds} {...actions} />
          <QueueTaskGroup label="Draft" status="backlog" pairs={visiblePairs.filter(({ task }) => task.status === "backlog")} now={now} recentlyTransitionedTaskIds={recentlyTransitionedTaskIds} {...actions} />
          {visiblePairs.some(({ task }) => task.status !== "ready" && task.status !== "backlog") && <TaskGroup label="Waiting" pairs={visiblePairs.filter(({ task }) => task.status !== "ready" && task.status !== "backlog")} now={now} recentlyTransitionedTaskIds={recentlyTransitionedTaskIds} {...actions} />}
        </> : <TaskGroup pairs={visiblePairs} now={now} recentlyTransitionedTaskIds={recentlyTransitionedTaskIds} {...actions} />}
        {visiblePairs.length === 0 && stage !== "queue" && <p className="empty-column">No work in this stage</p>}
      </div>
      {stage === "done" && totalCount > 10 && <button className="done-expand" type="button" onClick={onToggleDone}>{showAllDone ? "Show recent 10" : `View all ${totalCount}`}</button>}
    </section>
  );
}

type TaskPair = { task: Task; view: WorkflowTaskView };

function QueueTaskGroup({ label, status, pairs, now, recentlyTransitionedTaskIds, ...actions }: { label: string; status: "backlog" | "ready"; pairs: TaskPair[]; now: number; recentlyTransitionedTaskIds: string[] } & WorkflowTaskActions) {
  const { setNodeRef, isOver } = useDroppable({ id: `column:${status}` });
  const orderedPairs = [...pairs].sort((left, right) => left.task.position - right.task.position);
  return <section ref={setNodeRef} className={`flow-task-group ${isOver ? "is-over" : ""}`}>
    <header><span>{label}</span><strong>{orderedPairs.length}</strong></header>
    <SortableContext items={orderedPairs.map(({ task }) => task.id)} strategy={verticalListSortingStrategy}>
      {orderedPairs.map(({ task, view }) => <WorkflowTaskCard key={task.id} task={task} view={view} now={now} isRecentlyTransitioned={recentlyTransitionedTaskIds.includes(task.id)} {...actions} />)}
    </SortableContext>
    {orderedPairs.length === 0 && <p className="empty-task-group">No {label.toLocaleLowerCase()} tasks</p>}
  </section>;
}

function TaskGroup({ label, pairs, now, recentlyTransitionedTaskIds, ...actions }: { label?: string; pairs: TaskPair[]; now: number; recentlyTransitionedTaskIds: string[] } & WorkflowTaskActions) {
  return <section className="flow-task-group flow-task-group-static">
    {label && <header><span>{label}</span><strong>{pairs.length}</strong></header>}
    <SortableContext items={pairs.map(({ task }) => task.id)} strategy={verticalListSortingStrategy}>
      {pairs.map(({ task, view }) => <WorkflowTaskCard key={task.id} task={task} view={view} now={now} isRecentlyTransitioned={recentlyTransitionedTaskIds.includes(task.id)} {...actions} />)}
    </SortableContext>
  </section>;
}

type AgentActivityRailProps = {
  activities: AgentActivityItem[];
  idleAgents: Agent[];
  idleCount: number;
  isOpen: boolean;
  isDrawer: boolean;
  showIdle: boolean;
  now: number;
  onClose: () => void;
  onToggleIdle: () => void;
  onOpen: (activity: AgentActivityItem) => void;
};

export function AgentActivityRail(props: AgentActivityRailProps) {
  const { activities, idleAgents, idleCount, isOpen, isDrawer, showIdle, now, onClose, onToggleIdle, onOpen } = props;
  const order = { running: 0, queued: 1, waiting: 2 } as const;
  const sortedActivities = [...activities].sort((left, right) => order[left.status] - order[right.status]);
  const drawerClosed = agentDrawerClosed(isDrawer, isOpen);
  return <aside className={agentRailClass(isOpen)} aria-label="Agent activity" aria-hidden={trueOrUndefined(drawerClosed)} hidden={drawerClosed} inert={trueOrUndefined(drawerClosed)}>
    <header><div><Bot size={15} /><h2>Agent activity</h2></div><span>{activities.length}</span><button type="button" onClick={onClose} aria-label="Close agent activity"><ChevronRight size={15} /></button></header>
    <AgentActivityList activities={sortedActivities} idleAgents={idleAgents} idleCount={idleCount} showIdle={showIdle} now={now} onToggleIdle={onToggleIdle} onOpen={onOpen} />
  </aside>;
}

function agentDrawerClosed(isDrawer: boolean, isOpen: boolean): boolean { return isDrawer && !isOpen; }

function agentRailClass(isOpen: boolean): string { return `agent-activity-rail ${isOpen ? "is-open" : ""}`; }

function trueOrUndefined(value: boolean): true | undefined { return value ? true : undefined; }

function AgentActivityList({ activities, idleAgents, idleCount, showIdle, now, onToggleIdle, onOpen }: Pick<AgentActivityRailProps, "activities" | "idleAgents" | "idleCount" | "showIdle" | "now" | "onToggleIdle" | "onOpen">) {
  return <div className="agent-activity-list">{activities.map((activity) => <AgentActivityRow key={activity.id} activity={activity} now={now} onOpen={onOpen} />)}<EmptyAgentActivity isEmpty={activities.length === 0} /><IdleAgentGroup agents={idleAgents} count={idleCount} expanded={showIdle} onToggle={onToggleIdle} /></div>;
}

function AgentActivityRow({ activity, now, onOpen }: { activity: AgentActivityItem; now: number; onOpen: AgentActivityRailProps["onOpen"] }) {
  return <button className={`agent-activity-item ${activity.status}`} type="button" onClick={() => onOpen(activity)}><span className={`activity-state ${activity.status}`} aria-hidden="true" /><span className="agent-activity-copy"><strong>{activity.agentName}</strong><span>{activity.role} / {formatActivityType(activity.activityType)}</span><OptionalText value={activity.taskTitle} element="b" /><OptionalText value={activity.waitingReason} element="em" /></span><span className="agent-activity-meta"><b>{activity.status}</b><time dateTime={activity.startedAt}>{formatAge(activity.startedAt, now)}</time><OptionalText value={activity.workerState} element="small" /></span></button>;
}

function OptionalText({ value, element }: { value?: string; element: "b" | "em" | "small" }) {
  if (!value) return null;
  if (element === "b") return <b>{value}</b>;
  if (element === "em") return <em>{value}</em>;
  return <small>{value}</small>;
}

function EmptyAgentActivity({ isEmpty }: { isEmpty: boolean }) { return isEmpty ? <div className="agent-activity-empty"><CircleUserRound size={20} /><p>No active or waiting agents</p></div> : null; }

function IdleAgentGroup({ agents, count, expanded, onToggle }: { agents: Agent[]; count: number; expanded: boolean; onToggle: () => void }) {
  if (count === 0) return null;
  return <div className="idle-agents"><button type="button" aria-expanded={expanded} onClick={onToggle}><ChevronDown size={14} /> {count} idle</button>{expanded ? <ul>{agents.map((agent) => <li key={agent.id}><span className="activity-state idle" /><span><strong>{agent.name}</strong><small>{agent.role}</small></span></li>)}</ul> : null}</div>;
}

type WorkflowTaskCardProps = { task: Task; view?: WorkflowTaskView; now: number; isRecentlyTransitioned: boolean } & WorkflowTaskActions;

export function WorkflowTaskCard({ task, view, now, isRecentlyTransitioned, onInspect, onEdit, onDelete, onPlanningState }: WorkflowTaskCardProps) {
  const sortable = isPlanningTask(task.status);
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: task.id, disabled: !sortable });
  return <article ref={setNodeRef} style={{ transform: CSS.Transform.toString(transform), transition }} className={taskCardClass(task.status, isRecentlyTransitioned, isDragging)}>
    <TaskMoveAffordance task={task} sortable={sortable} attributes={attributes} listeners={listeners} />
    <button className="task-card-copy" type="button" onClick={() => onInspect(task)}>
      <div className="task-card-title"><h3>{task.title}</h3><TaskPriority priority={task.priority} /></div>
      <TaskStatusLine task={task} view={view} now={now} />
      <p className={nextActionClass(task.status)}>{taskActionText(task, view)}</p>
      <TaskActor actor={view?.currentActor} />
    </button>
    <TaskCardMenu task={task} onPlanningState={onPlanningState} onEdit={onEdit} onDelete={onDelete} />
  </article>;
}

function isPlanningTask(status: TaskStatus): boolean { return status === "backlog" || status === "ready"; }

function taskCardClass(status: TaskStatus, transitioned: boolean, dragging: boolean): string {
  return ["task-card cockpit-task-card", status === "in_progress" ? "is-running" : "", transitioned ? "just-transitioned" : "", dragging ? "is-dragging" : ""].filter(Boolean).join(" ");
}

function TaskMoveAffordance({ task, sortable, attributes, listeners }: { task: Task; sortable: boolean; attributes: ReturnType<typeof useSortable>["attributes"]; listeners: ReturnType<typeof useSortable>["listeners"] }) {
  return sortable ? <button className="drag-handle" type="button" aria-label={`Reorder ${task.title}`} {...attributes} {...listeners}><GripVertical size={15} /></button> : <span className={`task-state-marker ${taskTones[task.status]}`} aria-hidden="true" />;
}

function TaskPriority({ priority }: { priority: Task["priority"] }) {
  if (priority !== "critical" && priority !== "high") return null;
  return <span className={`task-card-priority ${priority}`}>{priority}</span>;
}

function TaskStatusLine({ task, view, now }: { task: Task; view?: WorkflowTaskView; now: number }) {
  const changedAt = view ? view.statusChangedAt : task.updatedAt;
  return <div className="task-card-status-row"><span className={`task-status-chip ${taskTones[task.status]}`}>{task.status.replace("_", " ")}</span><time dateTime={changedAt}><Clock3 size={10} />{formatAge(changedAt, now)}</time></div>;
}

function taskActionText(task: Task, view: WorkflowTaskView | undefined): string {
  const projected = projectedActionText(view);
  if (projected) return projected;
  return task.blockedReason ? task.blockedReason : statusFallbackText(task.status);
}

function projectedActionText(view: WorkflowTaskView | undefined): string | undefined { return view ? (view.nextAction.reason || view.nextAction.label) : undefined; }

function nextActionClass(status: TaskStatus): string {
  return `task-next-action ${status === "blocked" || status === "needs_input" ? "is-waiting" : ""}`;
}

function TaskActor({ actor }: { actor?: WorkflowTaskView["currentActor"] }) {
  if (!actor) return <div className="task-assignment unassigned"><CircleUserRound size={12} /><span>Human</span></div>;
  return <div className="task-assignment assigned"><ActorIcon kind={actor.kind} /><span>{actor.label}</span></div>;
}

function ActorIcon({ kind }: { kind: WorkflowActorKind }) { return kind === "agent" ? <Bot size={12} /> : <CircleUserRound size={12} />; }

function TaskCardMenu({ task, onPlanningState, onEdit, onDelete }: Pick<WorkflowTaskCardProps, "task" | "onPlanningState" | "onEdit" | "onDelete">) {
  return <details className="task-card-overflow" onPointerDown={(event) => event.stopPropagation()}><summary aria-label={`Actions for ${task.title}`}><MoreHorizontal size={14} /></summary><div role="menu">
    {task.status === "backlog" && <button type="button" role="menuitem" onClick={() => onPlanningState(task, "ready")}><CheckCircle2 size={13} /> Mark ready</button>}
    {task.status === "ready" && <button type="button" role="menuitem" onClick={() => onPlanningState(task, "backlog")}><ArrowLeft size={13} /> Defer to draft</button>}
    <button type="button" role="menuitem" onClick={() => onEdit(task)}><Pencil size={13} /> Edit</button><button type="button" role="menuitem" className="danger" onClick={() => onDelete(task)}><Trash2 size={13} /> Delete</button>
  </div></details>;
}

export function WorkflowTaskDragPreview({ task, view }: { task?: Task; view?: WorkflowTaskView }) {
  if (!task) return null;
  return <article className="task-card cockpit-task-card task-drag-overlay"><span className="drag-handle" aria-hidden="true"><GripVertical size={15} /></span><div className="task-card-copy"><div className="task-card-title"><h3>{task.title}</h3><TaskPriority priority={task.priority} /></div><p className="task-next-action">{dragActionText(task, view)}</p></div></article>;
}

function dragActionText(task: Task, view: WorkflowTaskView | undefined): string { return view ? view.nextAction.label : statusFallbackText(task.status); }

function formatAttentionKind(kind: AttentionItem["kind"]) {
  const labels: Record<AttentionItem["kind"], string> = { needs_input: "Input", review_approval: "Review", run_recovery: "Run", integration_recovery: "Integration", project_blocker: "Blocker", health_broken: "Health", planning_approval: "Plan", collaboration: "Collaboration", autonomy_paused: "Autonomy", cost_block: "Cost" };
  return labels[kind];
}

function formatActivityType(type: AgentActivityItem["activityType"]) { return type === "assigned" ? "assigned work" : type; }

function formatAge(value: string, now: number) {
  const elapsed = Math.max(0, now - parseWorkflowTimestamp(value));
  if (!Number.isFinite(elapsed) || elapsed < 60_000) return "now";
  const minutes = Math.floor(elapsed / 60_000);
  return formatElapsedMinutes(minutes);
}

function formatElapsedMinutes(minutes: number) {
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h` : `${Math.floor(hours / 24)}d`;
}

function statusFallbackText(status: TaskStatus) {
  const text: Record<TaskStatus, string> = { backlog: "Complete the specification and mark Ready", ready: "Assign an agent or start work", in_progress: "Implementation is in progress", needs_input: "Answer the open question", review: "Review the changes", approved: "Waiting in the integration queue", integrating: "Integration validation is running", blocked: "Resolve the blocker", done: "Integrated into the project" };
  return text[status];
}
