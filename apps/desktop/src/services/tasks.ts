import { invoke } from "@tauri-apps/api/core";

export const TASK_STATUSES = ["backlog", "ready", "in_progress", "review", "approved", "integrating", "blocked", "done"] as const;
export type TaskStatus = typeof TASK_STATUSES[number];
export const TASK_PRIORITIES = ["critical", "high", "normal", "low"] as const;
export type TaskPriority = typeof TASK_PRIORITIES[number];

export type Task = {
  id: string;
  projectId: string;
  title: string;
  description: string | null;
  acceptanceCriteria: string[];
  implementationNotes: string | null;
  relevantPaths: string[];
  dependencyIds: string[];
  assignedAgentId: string | null;
  branch: string | null;
  worktreePath: string | null;
  priority: TaskPriority;
  blockedReason: string | null;
  milestoneId: string | null;
  epicId: string | null;
  status: TaskStatus;
  position: number;
  createdAt: string;
  updatedAt: string;
};

export type TaskInput = {
  title: string;
  description?: string;
  acceptanceCriteria: string[];
  implementationNotes?: string;
  relevantPaths: string[];
  dependencyIds: string[];
  assignedAgentId?: string;
  priority: TaskPriority;
  milestoneId?: string;
  epicId?: string;
};

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

export function cleanupTaskWorktree(id: string): Promise<Task> {
  return invoke<Task>("cleanup_task_worktree", { taskId: id });
}

export function openTaskWorktree(id: string): Promise<void> {
  return invoke("open_task_worktree", { taskId: id });
}
