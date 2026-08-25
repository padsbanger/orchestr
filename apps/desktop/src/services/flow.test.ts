import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import { getFlowState, listenToFlowChanges, updateFlowLimits, type FlowState } from "./flow";

const flow: FlowState = {
  limits: { projectId: "project-1", workerId: "local", workerMaxConcurrentRuns: 4, inProgressLimit: 4, reviewLimit: 3, approvedLimit: 2 },
  activeWorkerRuns: 2,
  inProgress: 2,
  review: 1,
  approved: 0,
  integrating: 0,
  queued: 1,
  blockedReason: null,
  queue: [],
};

describe("flow control service", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
  });

  it("loads the project flow snapshot", async () => {
    invokeMock.mockResolvedValue(flow);
    await expect(getFlowState("project-1")).resolves.toEqual(flow);
    expect(invokeMock).toHaveBeenCalledWith("get_flow_state", { projectId: "project-1" });
  });

  it("updates all worker and project limits through one domain command", async () => {
    invokeMock.mockResolvedValue(flow);
    const limits = { workerMaxConcurrentRuns: 3, inProgressLimit: 3, reviewLimit: 2, approvedLimit: 1 };
    await expect(updateFlowLimits("project-1", limits)).resolves.toEqual(flow);
    expect(invokeMock).toHaveBeenCalledWith("update_flow_limits", { input: { projectId: "project-1", ...limits } });
  });

  it("forwards scheduler changes", async () => {
    const unlisten = vi.fn();
    const handler = vi.fn();
    listenMock.mockResolvedValue(unlisten);
    await expect(listenToFlowChanges(handler)).resolves.toBe(unlisten);
    const listener = listenMock.mock.calls[0][1] as (event: { payload: string }) => void;
    listener({ payload: "run-1" });
    expect(handler).toHaveBeenCalledWith("run-1");
  });
});
