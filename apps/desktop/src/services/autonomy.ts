import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AutonomyStatus = "stopped" | "running" | "paused" | "completed";

export type ProjectAutonomy = {
  projectId: string;
  status: AutonomyStatus;
  planningProposalId: string | null;
  reviewerAgentId: string | null;
  autoSchedule: boolean;
  autoReview: boolean;
  autoIntegrate: boolean;
  maxTasksPerCycle: number;
  maxAutoRetries: number;
  pauseOnFailure: boolean;
  pauseOnNeedsInput: boolean;
  pauseReason: string | null;
  startedAt: string | null;
  stoppedAt: string | null;
  lastCycleAt: string | null;
  updatedAt: string;
};

export type AutonomyConfiguration = Pick<ProjectAutonomy,
  "planningProposalId" | "reviewerAgentId" | "autoSchedule" | "autoReview" |
  "autoIntegrate" | "maxTasksPerCycle" | "maxAutoRetries" | "pauseOnFailure" |
  "pauseOnNeedsInput">;

export type AutonomyCycle = {
  id: string;
  projectId: string;
  triggerKind: "user" | "timer" | "event";
  status: "running" | "completed" | "paused" | "failed" | "skipped";
  scheduledCount: number;
  reviewCount: number;
  retryCount: number;
  integrationCount: number;
  outcome: string | null;
  startedAt: string;
  completedAt: string | null;
};

export type AutonomyEvent = {
  id: number;
  projectId: string;
  cycleId: string | null;
  kind: string;
  message: string;
  taskId: string | null;
  runId: string | null;
  createdAt: string;
};

export type AutonomyCounts = {
  total: number;
  backlog: number;
  ready: number;
  inProgress: number;
  needsInput: number;
  review: number;
  blocked: number;
  done: number;
};

export type ProjectAutonomySnapshot = {
  autonomy: ProjectAutonomy;
  goal: string | null;
  taskIds: string[];
  counts: AutonomyCounts;
  cycles: AutonomyCycle[];
  events: AutonomyEvent[];
};

export function getProjectAutonomy(projectId: string): Promise<ProjectAutonomySnapshot> {
  return invoke("get_project_autonomy", { projectId });
}

export function updateProjectAutonomy(projectId: string, configuration: AutonomyConfiguration): Promise<ProjectAutonomySnapshot> {
  return invoke("update_project_autonomy", { input: { projectId, ...configuration } });
}

export function startProjectAutonomy(projectId: string): Promise<ProjectAutonomySnapshot> {
  return invoke("start_project_autonomy", { projectId });
}

export function pauseProjectAutonomy(projectId: string): Promise<ProjectAutonomySnapshot> {
  return invoke("pause_project_autonomy", { projectId });
}

export function stopProjectAutonomy(projectId: string): Promise<ProjectAutonomySnapshot> {
  return invoke("stop_project_autonomy", { projectId });
}

export function advanceProjectAutonomy(projectId: string): Promise<ProjectAutonomySnapshot> {
  return invoke("advance_project_autonomy", { projectId });
}

export function listenToAutonomyEvents(handler: (projectId: string) => void): Promise<UnlistenFn> {
  return listen<string>("autonomy://changed", ({ payload }) => handler(payload));
}
