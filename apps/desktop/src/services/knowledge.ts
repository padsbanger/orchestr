import { invoke } from "@tauri-apps/api/core";

export type ArchitectureDecisionStatus = "proposed" | "accepted" | "superseded" | "rejected";

export type ArchitectureDecision = {
  id: string;
  projectId: string;
  decisionNumber: number;
  title: string;
  context: string;
  decision: string;
  consequences: string | null;
  status: ArchitectureDecisionStatus;
  supersedesDecisionId: string | null;
  relevantPaths: string[];
  relevantTaskIds: string[];
  createdAt: string;
  updatedAt: string;
  decidedAt: string | null;
};

export type ArchitectureDecisionInput = {
  projectId: string;
  title: string;
  context: string;
  decision: string;
  consequences?: string;
  supersedesDecisionId?: string;
  relevantPaths: string[];
  relevantTaskIds: string[];
};

export function listArchitectureDecisions(projectId: string): Promise<ArchitectureDecision[]> {
  return invoke("list_architecture_decisions", { projectId });
}

export function listRelevantArchitectureDecisions(taskId: string): Promise<ArchitectureDecision[]> {
  return invoke("list_relevant_architecture_decisions", { taskId });
}

export function createArchitectureDecision(input: ArchitectureDecisionInput): Promise<ArchitectureDecision> {
  return invoke("create_architecture_decision", { input });
}

export function decideArchitectureDecision(
  decisionId: string,
  status: "accepted" | "rejected",
): Promise<ArchitectureDecision> {
  return invoke("decide_architecture_decision", { decisionId, status });
}
