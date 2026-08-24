import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import {
  listAgentReviews,
  listenToAgentReviewEvents,
  startAgentReview,
  type AgentReview,
} from "./agentReviews";

const review: AgentReview = {
  id: "review-1",
  taskId: "task-1",
  agentId: "agent-1",
  status: "completed",
  decision: "approve",
  notes: "Ready to integrate.",
  rawOutput: "ORCHESTR_REVIEW_DECISION: approve",
  error: null,
  startedAt: "2026-08-23T10:00:00Z",
  completedAt: "2026-08-23T10:01:00Z",
};

describe("agent review service", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
  });

  it("lists reviews for a task", async () => {
    invokeMock.mockResolvedValue([review]);

    await expect(listAgentReviews("task-1")).resolves.toEqual([review]);
    expect(invokeMock).toHaveBeenCalledWith("list_agent_reviews", { taskId: "task-1" });
  });

  it("starts a review with the selected task and agent", async () => {
    invokeMock.mockResolvedValue(review);

    await expect(startAgentReview("task-1", "agent-1")).resolves.toEqual(review);
    expect(invokeMock).toHaveBeenCalledWith("start_agent_review", {
      input: { taskId: "task-1", agentId: "agent-1" },
    });
  });

  it("forwards agent review event identifiers and returns the listener cleanup", async () => {
    const unlisten = vi.fn();
    const handler = vi.fn();
    listenMock.mockResolvedValue(unlisten);

    await expect(listenToAgentReviewEvents(handler)).resolves.toBe(unlisten);
    expect(listenMock).toHaveBeenCalledWith("agent-review://event", expect.any(Function));

    const listener = listenMock.mock.calls[0][1] as (event: { payload: string }) => void;
    listener({ payload: "review-1" });

    expect(handler).toHaveBeenCalledWith("review-1");
  });
});
