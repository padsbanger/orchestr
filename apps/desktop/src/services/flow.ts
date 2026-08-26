import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TaskRun } from "./runs";

export type FlowLimits = {
  projectId: string;
  workerId: string;
  workerMaxConcurrentRuns: number;
  inProgressLimit: number;
  reviewLimit: number;
  approvedLimit: number;
};

export type FlowState = {
  limits: FlowLimits;
  activeWorkerRuns: number;
  inProgress: number;
  review: number;
  approved: number;
  integrating: number;
  queued: number;
  blockedReason: string | null;
  queue: TaskRun[];
  schedulerDecisions: SchedulerDecision[];
};

export type SchedulerDecision = {
  id: string;
  projectId: string;
  taskId: string | null;
  workerId: string | null;
  runId: string | null;
  outcome: "scheduled" | "skipped" | "blocked";
  reason: string;
  createdAt: string;
};

export type ScheduleProjectResult = {
  scheduled: SchedulerDecision[];
  skipped: SchedulerDecision[];
  blockedReason: string | null;
};

export type FlowLimitInput = Pick<FlowLimits, "workerMaxConcurrentRuns" | "inProgressLimit" | "reviewLimit" | "approvedLimit">;

export function getFlowState(projectId: string): Promise<FlowState> {
  return invoke<FlowState>("get_flow_state", { projectId });
}

export function updateFlowLimits(projectId: string, limits: FlowLimitInput): Promise<FlowState> {
  return invoke<FlowState>("update_flow_limits", { input: { projectId, ...limits } });
}

export function scheduleReadyTasks(projectId: string): Promise<ScheduleProjectResult> {
  return invoke<ScheduleProjectResult>("schedule_ready_tasks", { projectId });
}

export function listenToFlowChanges(handler: (runId: string) => void) {
  return listen<string>("scheduler://changed", ({ payload }) => handler(payload));
}
