import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type PlanningProposalStatus = "generating" | "proposed" | "approved" | "rejected" | "failed" | "cancelled";

export type PlanningTask = {
  key: string;
  title: string;
  description: string | null;
  acceptanceCriteria: string[];
  implementationNotes: string | null;
  relevantPaths: string[];
  requiredCapabilities: string[];
  dependencyKeys: string[];
  priority: "critical" | "high" | "normal" | "low";
};

export type PlanningPlan = {
  summary: string;
  milestone: { title: string; description: string | null } | null;
  epic: { title: string; description: string | null } | null;
  tasks: PlanningTask[];
};

export type PlanningProposal = {
  id: string;
  projectId: string;
  agentId: string | null;
  goal: string;
  status: PlanningProposalStatus;
  plan: PlanningPlan | null;
  rawOutput: string;
  error: string | null;
  milestoneId: string | null;
  epicId: string | null;
  taskIds: string[];
  createdAt: string;
  updatedAt: string;
  completedAt: string | null;
  decidedAt: string | null;
};

export function listPlanningProposals(projectId: string): Promise<PlanningProposal[]> {
  return invoke("list_planning_proposals", { projectId });
}

export function startPlanningProposal(projectId: string, agentId: string, goal: string): Promise<PlanningProposal> {
  return invoke("start_planning_proposal", { input: { projectId, agentId, goal } });
}

export function approvePlanningProposal(proposalId: string): Promise<PlanningProposal> {
  return invoke("approve_planning_proposal", { proposalId });
}

export function rejectPlanningProposal(proposalId: string): Promise<PlanningProposal> {
  return invoke("reject_planning_proposal", { proposalId });
}

export function cancelPlanningProposal(proposalId: string): Promise<void> {
  return invoke("cancel_local_worker_run", { runId: proposalId });
}

export function listenToPlanningEvents(handler: (proposalId: string) => void): Promise<UnlistenFn> {
  return listen<string>("planning://event", ({ payload }) => handler(payload));
}
