import { invoke } from "@tauri-apps/api/core";
import type { Task } from "./tasks";

export type RunStatus = "queued" | "running" | "failed" | "completed" | "cancelled";

export type RunOutput = {
  stream: "stdout" | "stderr";
  text: string;
  createdAt: string;
};

export type RunEvent = {
  id: number;
  kind: string;
  message: string;
  command: string | null;
  filePath: string | null;
  exitCode: number | null;
  createdAt: string;
};

export type TaskRun = {
  id: string;
  taskId: string;
  agentId: string;
  workerId: string;
  status: RunStatus;
  startedAt: string;
  completedAt: string | null;
  exitCode: number | null;
  error: string | null;
  output: RunOutput[];
  events: RunEvent[];
};

export type StartedTaskRun = {
  run: TaskRun;
  task: Task;
};

export function listTaskRuns(taskId: string): Promise<TaskRun[]> {
  return invoke<TaskRun[]>("list_task_runs", { taskId });
}

export function startTaskRun(taskId: string): Promise<StartedTaskRun> {
  return invoke<StartedTaskRun>("start_task_run", { taskId });
}

export function recoverTaskRun(runId: string, mode: "resume" | "restart_clean", agentId?: string): Promise<StartedTaskRun> {
  return invoke<StartedTaskRun>("recover_task_run", { input: { runId, mode, agentId } });
}

export function resolveFailedRun(runId: string, action: "abandon" | "escalate", note?: string): Promise<Task> {
  return invoke<Task>("resolve_failed_run", { input: { runId, action, note } });
}

export function cancelTaskRun(runId: string): Promise<void> {
  return invoke<void>("cancel_local_worker_run", { runId });
}

export function cancelQueuedTaskRun(runId: string): Promise<void> {
  return invoke<void>("cancel_queued_task_run", { runId });
}

export function exportTaskRunLog(runId: string, destinationPath: string): Promise<void> {
  return invoke<void>("export_task_run_log", { runId, destinationPath });
}
