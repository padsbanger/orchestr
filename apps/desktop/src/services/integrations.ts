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

export function listIntegrationAttempts(projectId: string): Promise<IntegrationAttempt[]> {
  return invoke<IntegrationAttempt[]>("list_integration_attempts", { projectId });
}

export function integrateNextTask(projectId: string): Promise<IntegrationExecution> {
  return invoke<IntegrationExecution>("integrate_next_task", { projectId });
}

export function retryIntegrationAttempt(attemptId: string): Promise<Task> {
  return invoke<Task>("retry_integration_attempt", { attemptId });
}
