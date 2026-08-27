import { invoke } from "@tauri-apps/api/core";

export type CollaborationKind = "comment" | "request" | "blocker" | "interface_change" | "escalation";

export type CollaborationEntry = {
  id: string;
  projectId: string;
  taskId: string | null;
  parentId: string | null;
  authorType: "human" | "agent" | "system";
  authorAgentId: string | null;
  authorRunId: string | null;
  kind: CollaborationKind;
  message: string;
  status: "open" | "resolved";
  referencedTaskIds: string[];
  createdAt: string;
  resolvedAt: string | null;
};

export type CollaborationEntryInput = {
  projectId: string;
  taskId?: string;
  parentId?: string;
  kind: CollaborationKind;
  message: string;
  referencedTaskIds: string[];
};

export function listCollaborationEntries(projectId: string): Promise<CollaborationEntry[]> {
  return invoke("list_collaboration_entries", { projectId });
}

export function createCollaborationEntry(input: CollaborationEntryInput): Promise<CollaborationEntry> {
  return invoke("create_collaboration_entry", { input });
}

export function resolveCollaborationEntry(entryId: string): Promise<CollaborationEntry> {
  return invoke("resolve_collaboration_entry", { entryId });
}
