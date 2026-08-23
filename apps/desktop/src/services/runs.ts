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

type StartedTaskRun = {
  run: TaskRun;
  task: Task;
};

export function listTaskRuns(taskId: string): Promise<TaskRun[]> {
  return invoke<TaskRun[]>("list_task_runs", { taskId });
}

export function startTaskRun(taskId: string): Promise<StartedTaskRun> {
  return invoke<StartedTaskRun>("start_task_run", { taskId });
}

export function cancelTaskRun(runId: string): Promise<void> {
  return invoke<void>("cancel_local_worker_run", { runId });
}

export function exportTaskRunLog(runId: string, destinationPath: string): Promise<void> {
  return invoke<void>("export_task_run_log", { runId, destinationPath });
}
