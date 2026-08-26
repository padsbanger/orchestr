import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, listenMock } = vi.hoisted(() => ({ invokeMock: vi.fn(), listenMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import { approvePlanningProposal, cancelPlanningProposal, listPlanningProposals, listenToPlanningEvents, rejectPlanningProposal, startPlanningProposal } from "./planning";

describe("planning service", () => {
  beforeEach(() => { invokeMock.mockReset(); listenMock.mockReset(); });

  it("loads and starts project proposals", async () => {
    invokeMock.mockResolvedValueOnce([]).mockResolvedValueOnce({ id: "plan-1" });
    await expect(listPlanningProposals("project-1")).resolves.toEqual([]);
    await expect(startPlanningProposal("project-1", "agent-1", "Add OAuth")).resolves.toEqual({ id: "plan-1" });
    expect(invokeMock).toHaveBeenNthCalledWith(1, "list_planning_proposals", { projectId: "project-1" });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "start_planning_proposal", { input: { projectId: "project-1", agentId: "agent-1", goal: "Add OAuth" } });
  });

  it("forwards explicit human decisions and cancellation", async () => {
    invokeMock.mockResolvedValue(undefined);
    await approvePlanningProposal("plan-1");
    await rejectPlanningProposal("plan-2");
    await cancelPlanningProposal("plan-3");
    expect(invokeMock).toHaveBeenNthCalledWith(1, "approve_planning_proposal", { proposalId: "plan-1" });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "reject_planning_proposal", { proposalId: "plan-2" });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "cancel_local_worker_run", { runId: "plan-3" });
  });

  it("forwards planning events and listener cleanup", async () => {
    const unlisten = vi.fn();
    const handler = vi.fn();
    listenMock.mockResolvedValue(unlisten);
    await expect(listenToPlanningEvents(handler)).resolves.toBe(unlisten);
    const callback = listenMock.mock.calls[0][1] as (event: { payload: string }) => void;
    callback({ payload: "plan-1" });
    expect(handler).toHaveBeenCalledWith("plan-1");
  });
});
