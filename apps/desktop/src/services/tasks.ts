import { invoke } from "@tauri-apps/api/core";

export const TASK_STATUSES = ["backlog", "todo", "in_progress", "review", "done"] as const;
export type TaskStatus = typeof TASK_STATUSES[number];

export type Task = {
  id: string;
  projectId: string;
  title: string;
  description: string | null;
  status: TaskStatus;
  position: number;
  createdAt: string;
  updatedAt: string;
};

type TaskInput = { title: string; description?: string };

export function listTasks(projectId: string): Promise<Task[]> {
  return invoke<Task[]>("list_tasks", { projectId });
}

export function createTask(projectId: string, input: TaskInput): Promise<Task> {
  return invoke<Task>("create_task", { input: { projectId, ...input } });
}

export function updateTask(id: string, input: TaskInput): Promise<Task> {
  return invoke<Task>("update_task", { input: { id, ...input } });
}

export function deleteTask(id: string): Promise<void> {
  return invoke("delete_task", { id });
}

export function moveTask(id: string, status: TaskStatus, position: number): Promise<Task> {
  return invoke<Task>("move_task", { input: { id, status, position } });
}
