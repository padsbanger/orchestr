import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Agent } from "./agents";
import type { CollaborationEntry } from "./collaboration";
import type { FlowState } from "./flow";
import type { IntegrationAttempt } from "./integrations";
import type { ProjectBlocker } from "./interruptions";
import type { PlanningProposal } from "./planning";
import type { ProjectHealth } from "./quality";
import { getSetting, setSetting } from "./settings";
import type { Task, TaskPriority, TaskStatus } from "./tasks";

export const WORKFLOW_STAGES = ["queue", "build", "verify", "done"] as const;
export type WorkflowStage = typeof WORKFLOW_STAGES[number];
export type WorkflowBoardView = "flow" | "lifecycle";

export type WorkflowActorKind = "human" | "agent" | "worker" | "system";

export type WorkflowActor = {
  kind: WorkflowActorKind;
  id?: string;
  label: string;
};

export type WorkflowAction = {
  kind: string;
  label: string;
  reason?: string;
};

export type ReadinessSummary = {
  ready: boolean;
  reason?: string;
};

export type WorkflowTaskView = {
  id: string;
  projectId: string;
  title: string;
  priority: TaskPriority;
  status: TaskStatus;
  stage: WorkflowStage;
  position: number;
  statusChangedAt: string;
  currentActor?: WorkflowActor;
  nextAction: WorkflowAction;
  readiness?: ReadinessSummary;
  assignedAgentId?: string | null;
  blockedReason?: string | null;
  milestoneId?: string | null;
  epicId?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type WorkflowStageView = {
  id: WorkflowStage;
  label: string;
  totalCount: number;
  tasks: WorkflowTaskView[];
};

export type AttentionKind =
  | "needs_input"
  | "review_approval"
  | "run_recovery"
  | "integration_recovery"
  | "project_blocker"
  | "health_broken"
  | "planning_approval"
  | "collaboration"
  | "autonomy_paused"
  | "cost_block";

export type AttentionSeverity = "critical" | "high" | "normal";

export type AttentionItem = {
  id: string;
  kind: AttentionKind;
  severity: AttentionSeverity;
  title: string;
  detail?: string;
  taskId?: string;
  entityId?: string;
  createdAt: string;
};

export type AgentActivityType = "implementation" | "review" | "planning" | "assigned";
export type AgentActivityStatus = "running" | "queued" | "waiting";

export type AgentActivityItem = {
  id: string;
  agentId: string;
  agentName: string;
  role: string;
  taskId?: string;
  taskTitle?: string;
  workerId?: string;
  workerState?: string;
  activityType: AgentActivityType;
  status: AgentActivityStatus;
  startedAt: string;
  waitingReason?: string;
};

export type ProjectWorkflowSnapshot = {
  projectId: string;
  generatedAt: string;
  health: ProjectHealth;
  stages: WorkflowStageView[];
  attention: AttentionItem[];
  agentActivity: AgentActivityItem[];
  idleAgentCount: number;
};

export type WorkflowChangedEvent = {
  projectId: string;
  reason: string;
  taskId?: string;
};

export type WorkflowAttentionDestination =
  | { kind: "task"; taskId: string }
  | { kind: "panel"; panel: "integration" | "collaboration" | "planning" | "blockers" | "quality" }
  | { kind: "route"; route: "autonomy" | "metrics" };

export type WorkflowActivityDestination =
  | { kind: "task"; taskId: string }
  | { kind: "panel"; panel: "planning" }
  | { kind: "route"; route: "agents" };

type NestedWorkflowTaskView = Omit<WorkflowTaskView, keyof Task> & { task: Task };

type WorkflowSnapshotWire = Omit<ProjectWorkflowSnapshot, "stages"> & {
  stages: Array<Omit<WorkflowStageView, "tasks"> & { tasks: Array<WorkflowTaskView | NestedWorkflowTaskView> }>;
};

/**
 * The initial plan used a nested `task` field while the final Rust read model is
 * compact. Normalizing both shapes keeps a rolling frontend/backend update safe.
 */
function normalizeTaskView(value: WorkflowTaskView | NestedWorkflowTaskView): WorkflowTaskView {
  if (!("task" in value)) return value;
  const { task, ...projection } = value;
  return {
    ...task,
    ...projection,
    assignedAgentId: task.assignedAgentId,
    blockedReason: task.blockedReason,
    milestoneId: task.milestoneId,
    epicId: task.epicId,
  };
}

export async function getProjectWorkflowSnapshot(projectId: string): Promise<ProjectWorkflowSnapshot> {
  const snapshot = await invoke<WorkflowSnapshotWire>("get_project_workflow_snapshot", { projectId });
  return {
    ...snapshot,
    stages: snapshot.stages.map((stage) => ({
      ...stage,
      totalCount: stage.totalCount ?? stage.tasks.length,
      tasks: stage.tasks.map(normalizeTaskView),
    })),
  };
}

export function listenToWorkflowChanges(handler: (event: WorkflowChangedEvent) => void): Promise<UnlistenFn> {
  return listen<WorkflowChangedEvent>("workflow://changed", ({ payload }) => handler(payload));
}

export function attentionDestination(item: AttentionItem): WorkflowAttentionDestination | undefined {
  switch (item.kind) {
    case "integration_recovery": return { kind: "panel", panel: "integration" };
    case "collaboration": return { kind: "panel", panel: "collaboration" };
    case "planning_approval": return { kind: "panel", panel: "planning" };
    case "project_blocker": return { kind: "panel", panel: "blockers" };
    case "health_broken": return { kind: "panel", panel: "quality" };
    case "autonomy_paused": return { kind: "route", route: "autonomy" };
    case "cost_block": return { kind: "route", route: "metrics" };
    default: return item.taskId ? { kind: "task", taskId: item.taskId } : undefined;
  }
}

export function agentActivityDestination(item: AgentActivityItem): WorkflowActivityDestination {
  if (item.taskId) return { kind: "task", taskId: item.taskId };
  if (item.activityType === "planning") return { kind: "panel", panel: "planning" };
  return { kind: "route", route: "agents" };
}

export function canReorderPlanningTask(source: TaskStatus, destination: TaskStatus): boolean {
  return source === destination && (source === "backlog" || source === "ready");
}

export function canChangePlanningStatus(source: TaskStatus, destination: TaskStatus): boolean {
  return source !== destination
    && (source === "backlog" || source === "ready")
    && (destination === "backlog" || destination === "ready");
}

export function workflowBoardViewSettingKey(projectId: string): string {
  return `board.view.${projectId}`;
}

export function parseWorkflowBoardView(value: string | null | undefined): WorkflowBoardView {
  return value === "lifecycle" || value === "full_lifecycle" ? "lifecycle" : "flow";
}

export async function loadWorkflowBoardView(projectId: string): Promise<WorkflowBoardView> {
  return parseWorkflowBoardView(await getSetting(workflowBoardViewSettingKey(projectId)));
}

export function saveWorkflowBoardView(projectId: string, view: WorkflowBoardView): Promise<void> {
  return setSetting(workflowBoardViewSettingKey(projectId), view);
}

/** Merge local task completeness into an authoritative backend projection. */
export function mergeWorkflowSnapshots(snapshot?: ProjectWorkflowSnapshot, fallback?: ProjectWorkflowSnapshot): ProjectWorkflowSnapshot | undefined {
  if (!snapshot) return fallback;
  if (!fallback) return snapshot;
  const fallbackById = new Map(fallback.stages.flatMap((stage) => stage.tasks.map((task) => [task.id, task] as const)));
  const isFresh = (task: WorkflowTaskView) => {
    const localTask = fallbackById.get(task.id);
    return Boolean(localTask)
      && task.status === localTask?.status
      && parseWorkflowTimestamp(task.updatedAt) >= parseWorkflowTimestamp(localTask.updatedAt);
  };
  const snapshotIds = new Set(snapshot.stages.flatMap((stage) => stage.tasks.filter(isFresh).map((task) => task.id)));
  const seen = new Set<string>();
  const stages = fallback.stages.map((fallbackStage) => {
    const snapshotStage = snapshot.stages.find((stage) => stage.id === fallbackStage.id);
    const authoritativeTasks = (snapshotStage?.tasks ?? []).filter((task) => isFresh(task) && !seen.has(task.id));
    authoritativeTasks.forEach((task) => seen.add(task.id));
    const missingTasks = fallbackStage.tasks.filter((task) => !snapshotIds.has(task.id) && !seen.has(task.id));
    missingTasks.forEach((task) => seen.add(task.id));
    const tasks = [...authoritativeTasks, ...missingTasks];
    return { ...fallbackStage, ...snapshotStage, totalCount: tasks.length, tasks };
  });
  return { ...fallback, ...snapshot, stages };
}

type WorkflowFallbackInput = {
  projectId: string;
  tasks: Task[];
  agents: Agent[];
  health?: ProjectHealth;
  flow?: FlowState;
  blockers?: ProjectBlocker[];
  integrations?: IntegrationAttempt[];
  proposals?: PlanningProposal[];
  collaboration?: CollaborationEntry[];
};

const STAGE_LABELS: Record<WorkflowStage, string> = {
  queue: "Queue",
  build: "Build",
  verify: "Verify & Land",
  done: "Done",
};

export function stageForTaskStatus(status: TaskStatus): WorkflowStage {
  if (status === "in_progress" || status === "needs_input") return "build";
  if (status === "review" || status === "approved" || status === "integrating") return "verify";
  if (status === "done") return "done";
  return "queue";
}

function fallbackAction(task: Task): WorkflowAction {
  switch (task.status) {
    case "backlog": return { kind: "mark_ready", label: "Complete the specification and mark Ready" };
    case "ready": return { kind: "start", label: task.assignedAgentId ? "Waiting for the assigned agent" : "Assign an agent or start work" };
    case "in_progress": return { kind: "monitor", label: "Implementation is in progress" };
    case "needs_input": return { kind: "answer_input", label: "Answer the open question", reason: task.blockedReason ?? undefined };
    case "review": return { kind: "review", label: "Review the changes" };
    case "approved": return { kind: "integrate", label: "Waiting in the integration queue" };
    case "integrating": return { kind: "monitor_integration", label: "Integration validation is running" };
    case "blocked": return { kind: "resolve_blocker", label: "Resolve the blocker", reason: task.blockedReason ?? undefined };
    case "done": return { kind: "complete", label: "Integrated into the project" };
  }
}

function fallbackActor(task: Task, agentsById: ReadonlyMap<string, Agent>): WorkflowActor {
  const agent = task.assignedAgentId ? agentsById.get(task.assignedAgentId) : undefined;
  if (agent) return { kind: "agent", id: agent.id, label: agent.name };
  if (task.status === "approved" || task.status === "integrating") return { kind: "system", label: "Integration service" };
  return { kind: "human", label: "Human" };
}

function fallbackTaskView(task: Task, agentsById: ReadonlyMap<string, Agent>): WorkflowTaskView {
  return {
    id: task.id,
    projectId: task.projectId,
    title: task.title,
    priority: task.priority,
    status: task.status,
    stage: stageForTaskStatus(task.status),
    position: task.position,
    statusChangedAt: task.updatedAt,
    currentActor: fallbackActor(task, agentsById),
    nextAction: fallbackAction(task),
    readiness: task.status === "backlog" || task.status === "ready" || task.status === "blocked"
      ? { ready: task.status === "ready", reason: task.blockedReason ?? undefined }
      : undefined,
    assignedAgentId: task.assignedAgentId,
    blockedReason: task.blockedReason,
    milestoneId: task.milestoneId,
    epicId: task.epicId,
    createdAt: task.createdAt,
    updatedAt: task.updatedAt,
  };
}

function compareAttention(left: AttentionItem, right: AttentionItem) {
  const severity = { critical: 0, high: 1, normal: 2 } as const;
  return severity[left.severity] - severity[right.severity]
    || parseWorkflowTimestamp(left.createdAt) - parseWorkflowTimestamp(right.createdAt);
}

export function parseWorkflowTimestamp(value: string) {
  const normalized = /(?:z|[+-]\d{2}:?\d{2})$/i.test(value)
    ? value
    : `${value.includes("T") ? value : value.replace(" ", "T")}Z`;
  return new Date(normalized).getTime();
}

/** A deliberately conservative UI fallback used when the new command is absent. */
export function createFallbackWorkflowSnapshot(input: WorkflowFallbackInput): ProjectWorkflowSnapshot {
  const agentsById = new Map(input.agents.map((agent) => [agent.id, agent]));
  const projectedTasks = input.tasks.map((task) => fallbackTaskView(task, agentsById));
  const taskAttention = input.tasks.flatMap<AttentionItem>((task): AttentionItem[] => {
    if (task.status === "needs_input") return [{ id: `task:${task.id}:input`, kind: "needs_input" as const, severity: "high" as const, title: task.title, detail: task.blockedReason ?? "Human input is required.", taskId: task.id, createdAt: task.updatedAt }];
    if (task.status === "review") return [{ id: `task:${task.id}:review`, kind: "review_approval" as const, severity: task.priority === "critical" ? "critical" as const : "normal" as const, title: task.title, detail: "Changes are ready for human review.", taskId: task.id, createdAt: task.updatedAt }];
    return [];
  });
  const blockerAttention: AttentionItem[] = (input.blockers ?? []).filter((blocker) => blocker.status === "active").map((blocker) => ({
    id: `blocker:${blocker.id}`, kind: "project_blocker", severity: blocker.affectsAllTasks ? "critical" : "high", title: blocker.title,
    detail: blocker.description ?? undefined, entityId: blocker.id, createdAt: blocker.createdAt,
  }));
  const integrationAttention: AttentionItem[] = (input.integrations ?? []).filter((attempt) => attempt.status === "conflict" || attempt.status === "failed").map((attempt) => ({
    id: `integration:${attempt.id}`, kind: "integration_recovery", severity: "high", title: attempt.status === "conflict" ? "Integration conflict" : "Integration failed",
    detail: attempt.error ?? undefined, taskId: attempt.taskId, entityId: attempt.id, createdAt: attempt.completedAt ?? attempt.startedAt ?? attempt.createdAt,
  }));
  const proposalAttention: AttentionItem[] = (input.proposals ?? []).filter((proposal) => proposal.status === "proposed").map((proposal) => ({
    id: `proposal:${proposal.id}`, kind: "planning_approval", severity: "normal", title: "Planning proposal needs approval", detail: proposal.goal, entityId: proposal.id, createdAt: proposal.updatedAt,
  }));
  const collaborationAttention: AttentionItem[] = (input.collaboration ?? []).filter((entry) => !entry.parentId && entry.status === "open" && entry.kind !== "comment" && (entry.authorType === "agent" || entry.authorType === "system")).map((entry) => ({
    id: `collaboration:${entry.id}`, kind: "collaboration", severity: entry.kind === "blocker" || entry.kind === "escalation" ? "high" : "normal", title: entry.kind.replace("_", " "),
    detail: entry.message, taskId: entry.taskId ?? undefined, entityId: entry.id, createdAt: entry.createdAt,
  }));
  const healthAttention: AttentionItem[] = input.health?.status === "broken" ? [{
    id: `health:${input.projectId}`, kind: "health_broken", severity: "critical", title: "Integration branch is broken", detail: input.health.failingGate ?? undefined, createdAt: input.health.updatedAt,
  }] : [];

  const activeAgentIds = new Set<string>();
  const agentActivity = input.tasks.flatMap((task): AgentActivityItem[] => {
    if (!task.assignedAgentId || !["ready", "in_progress", "review"].includes(task.status)) return [];
    const agent = agentsById.get(task.assignedAgentId);
    if (!agent) return [];
    activeAgentIds.add(agent.id);
    const queuedRun = input.flow?.queue.find((run) => run.taskId === task.id);
    const status: AgentActivityStatus = task.status === "in_progress" ? "running" : queuedRun ? "queued" : "waiting";
    return [{
      id: `agent:${agent.id}:task:${task.id}`, agentId: agent.id, agentName: agent.name, role: agent.role, taskId: task.id, taskTitle: task.title,
      workerId: queuedRun?.workerId, activityType: task.status === "review" ? "review" : task.status === "in_progress" ? "implementation" : "assigned", status,
      startedAt: queuedRun?.startedAt ?? task.updatedAt, waitingReason: status === "waiting" ? fallbackAction(task).label : undefined,
    }];
  });

  return {
    projectId: input.projectId,
    generatedAt: new Date().toISOString(),
    health: input.health ?? {
      projectId: input.projectId, status: "unknown", lastValidationAttemptId: null, lastSuccessfulValidationAt: null,
      lastIntegrationAt: null, failingGate: null, updatedAt: new Date().toISOString(),
    },
    stages: WORKFLOW_STAGES.map((id) => {
      const stageTasks = projectedTasks.filter((task) => task.stage === id);
      return { id, label: STAGE_LABELS[id], totalCount: stageTasks.length, tasks: stageTasks };
    }),
    attention: [...healthAttention, ...taskAttention, ...blockerAttention, ...integrationAttention, ...proposalAttention, ...collaborationAttention].sort(compareAttention),
    agentActivity,
    idleAgentCount: input.agents.filter((agent) => !activeAgentIds.has(agent.id)).length,
  };
}
