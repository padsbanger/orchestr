import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AgentReviewStatus = "running" | "completed" | "failed" | "cancelled";
export type AgentReviewDecision = "approve" | "request_changes";

export type AgentReview = {
  id: string;
  taskId: string;
  agentId: string;
  status: AgentReviewStatus;
  decision: AgentReviewDecision | null;
  notes: string | null;
  rawOutput: string;
  error: string | null;
  startedAt: string;
  completedAt: string | null;
};

export function listAgentReviews(taskId: string): Promise<AgentReview[]> {
  return invoke("list_agent_reviews", { taskId });
}

export function startAgentReview(taskId: string, agentId: string): Promise<AgentReview> {
  return invoke("start_agent_review", { input: { taskId, agentId } });
}

export function listenToAgentReviewEvents(handler: (reviewId: string) => void): Promise<UnlistenFn> {
  return listen<string>("agent-review://event", ({ payload }) => handler(payload));
}
