import { invoke } from "@tauri-apps/api/core";
import type { Task } from "./tasks";

export type IntegrationStatus = "queued" | "integrating" | "conflict" | "merged" | "failed";

export type IntegrationAttempt = {
  id: string;
  taskId: string;
  sourceBranch: string;
  targetBranch: string;
  status: IntegrationStatus;
  queuePosition: number;
  mergeCommit: string | null;
  error: string | null;
  createdAt: string;
  startedAt: string | null;
  completedAt: string | null;
};

export type IntegrationExecution = {
  task: Task;
  attempt: IntegrationAttempt;
  outcome: "merged" | "conflict" | "failed";
  message: string;
  cleanupError: string | null;
};

export type RevertStatus = "running" | "reverted" | "validation_failed" | "failed";

export type RevertAttempt = {
  id: string;
  projectId: string;
  originalTaskId: string;
  integrationAttemptId: string;
  originalCommit: string;
  status: RevertStatus;
  revertCommit: string | null;
  repairTaskId: string | null;
  error: string | null;
  startedAt: string;
  completedAt: string | null;
};

export function listIntegrationAttempts(projectId: string): Promise<IntegrationAttempt[]> {
  return invoke<IntegrationAttempt[]>("list_integration_attempts", { projectId });
}

export function integrateNextTask(projectId: string): Promise<IntegrationExecution> {
  return invoke<IntegrationExecution>("integrate_next_task", { projectId, allowedTaskIds: null });
}

export function retryIntegrationAttempt(attemptId: string): Promise<Task> {
  return invoke<Task>("retry_integration_attempt", { attemptId });
}

export function retryIntegrationCleanup(attemptId: string): Promise<IntegrationAttempt> {
  return invoke<IntegrationAttempt>("retry_integration_cleanup", { attemptId });
}

export function listRevertAttempts(projectId: string): Promise<RevertAttempt[]> {
  return invoke<RevertAttempt[]>("list_revert_attempts", { projectId });
}

export function revertIntegration(attemptId: string, createRepairTask: boolean): Promise<RevertAttempt> {
  return invoke<RevertAttempt>("revert_integration", { input: { attemptId, createRepairTask } });
}
