import { invoke } from "@tauri-apps/api/core";

export type Workspace = {
  id: string;
  projectId: string;
  workerId: string;
  path: string;
  createdAt: string;
  updatedAt: string;
};

export type Project = {
  id: string;
  name: string;
  description: string | null;
  defaultBranch: string;
  createdAt: string;
  updatedAt: string;
  workspaces: Workspace[];
};

type CreateProjectInput = {
  name: string;
  description?: string;
  parentPath: string;
  directoryName: string;
};

type RegisterProjectInput = {
  name: string;
  description?: string;
  path: string;
};

export function listProjects(): Promise<Project[]> {
  return invoke<Project[]>("list_projects");
}

export function getProject(id: string): Promise<Project | null> {
  return invoke<Project | null>("get_project", { id });
}

export function createProject(input: CreateProjectInput): Promise<Project> {
  return invoke<Project>("create_project", { input });
}

export function registerProject(input: RegisterProjectInput): Promise<Project> {
  return invoke<Project>("register_project", { input });
}
