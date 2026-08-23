import { invoke } from "@tauri-apps/api/core";
import type { CommitSummary } from "./projects";
import type { Task } from "./tasks";

export type TaskReview = { branch: string; baseBranch: string; commits: CommitSummary[]; diff: string; changedFiles: Array<{ path: string; status: string }> };

export function getTaskReview(taskId: string): Promise<TaskReview> { return invoke<TaskReview>("get_task_review", { taskId }); }
export function approveTaskReview(taskId: string): Promise<Task> { return invoke<Task>("approve_task_review", { taskId }); }
export function requestTaskChanges(taskId: string): Promise<Task> { return invoke<Task>("request_task_changes", { taskId }); }
