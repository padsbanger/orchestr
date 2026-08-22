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

export type CommitSummary = {
  hash: string;
  shortHash: string;
  subject: string;
  author: string;
  authoredAt: string;
};

export type ChangedFile = {
  path: string;
  status: string;
};

export type RepositoryDetails = {
  summary: {
    rootPath: string;
    defaultBranch: string;
    currentBranch: string | null;
    isClean: boolean;
    changedFileCount: number;
    latestCommit: CommitSummary | null;
  };
  recentCommits: CommitSummary[];
  changedFiles: ChangedFile[];
};

export type FilePreview =
  | { kind: "text"; content: string; truncated: boolean }
  | { kind: "image"; data: string; mimeType: string };

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

export function getRepositoryDetails(projectId: string): Promise<RepositoryDetails> {
  return invoke<RepositoryDetails>("get_repository_details", { projectId });
}

export function getRepositoryDiff(projectId: string, filePath: string): Promise<string | null> {
  return invoke<string | null>("get_repository_diff", { input: { projectId, filePath } });
}

export function getRepositoryFilePreview(projectId: string, filePath: string): Promise<FilePreview | null> {
  return invoke<FilePreview | null>("get_repository_file_preview", { input: { projectId, filePath } });
}
