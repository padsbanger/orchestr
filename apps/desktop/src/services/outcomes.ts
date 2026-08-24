import { invoke } from "@tauri-apps/api/core";

export type OutcomeStatus = "planned" | "active" | "completed" | "blocked";

export type Milestone = {
  id: string;
  projectId: string;
  title: string;
  description: string | null;
  status: OutcomeStatus;
  targetDate: string | null;
  createdAt: string;
  updatedAt: string;
};

export type Epic = {
  id: string;
  projectId: string;
  milestoneId: string | null;
  title: string;
  description: string | null;
  status: OutcomeStatus;
  createdAt: string;
  updatedAt: string;
};

export type TaskProgressCounts = {
  total: number;
  backlog: number;
  ready: number;
  inProgress: number;
  review: number;
  blocked: number;
  done: number;
};

export type ProjectProgress = {
  counts: TaskProgressCounts;
  milestones: { milestone: Milestone; counts: TaskProgressCounts; epics: Epic[] }[];
};

export function listMilestones(projectId: string): Promise<Milestone[]> { return invoke("list_milestones", { projectId }); }
export function listEpics(projectId: string): Promise<Epic[]> { return invoke("list_epics", { projectId }); }
export function getProjectProgress(projectId: string): Promise<ProjectProgress> { return invoke("get_project_progress", { projectId }); }
export function createMilestone(input: { projectId: string; title: string; description?: string; status: OutcomeStatus; targetDate?: string }): Promise<Milestone> { return invoke("create_milestone", { input }); }
export function updateMilestoneStatus(id: string, status: OutcomeStatus): Promise<Milestone> { return invoke("update_milestone_status", { input: { id, status } }); }
export function createEpic(input: { projectId: string; milestoneId?: string; title: string; description?: string; status: OutcomeStatus }): Promise<Epic> { return invoke("create_epic", { input }); }
export function updateEpicStatus(id: string, status: OutcomeStatus): Promise<Epic> { return invoke("update_epic_status", { input: { id, status } }); }
