import { invoke } from "@tauri-apps/api/core";
import type { Task } from "./tasks";

export type TaskInputRequest = {
  id: string;
  taskId: string;
  requestingRunId: string | null;
  requestingAgentId: string | null;
  question: string;
  status: "open" | "answered";
  answer: string | null;
  requestedAt: string;
  answeredAt: string | null;
};

export type ProjectBlocker = {
  id: string;
  projectId: string;
  title: string;
  description: string | null;
  affectsAllTasks: boolean;
  affectedTaskIds: string[];
  status: "active" | "resolved";
  createdAt: string;
  resolvedAt: string | null;
};

export function listTaskInputRequests(taskId: string): Promise<TaskInputRequest[]> {
  return invoke("list_task_input_requests", { taskId });
}

export function requestTaskInput(
  taskId: string,
  question: string,
  runId?: string,
): Promise<TaskInputRequest> {
  return invoke("request_task_input", { input: { taskId, question, runId } });
}

export function answerTaskInput(
  requestId: string,
  answer: string,
): Promise<{ request: TaskInputRequest; task: Task }> {
  return invoke("answer_task_input", { input: { requestId, answer } });
}

export function listProjectBlockers(projectId: string): Promise<ProjectBlocker[]> {
  return invoke("list_project_blockers", { projectId });
}

export function createProjectBlocker(input: {
  projectId: string;
  title: string;
  description?: string;
  affectsAllTasks: boolean;
  affectedTaskIds: string[];
}): Promise<ProjectBlocker> {
  return invoke("create_project_blocker", { input });
}

export function resolveProjectBlocker(blockerId: string): Promise<ProjectBlocker> {
  return invoke("resolve_project_blocker", { blockerId });
}
