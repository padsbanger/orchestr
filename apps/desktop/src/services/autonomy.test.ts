import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, listenMock } = vi.hoisted(() => ({ invokeMock: vi.fn(), listenMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import {
  advanceProjectAutonomy,
  getProjectAutonomy,
  listenToAutonomyEvents,
  pauseProjectAutonomy,
  startProjectAutonomy,
  stopProjectAutonomy,
  updateProjectAutonomy,
} from "./autonomy";

describe("autonomy service", () => {
  beforeEach(() => { invokeMock.mockReset(); listenMock.mockReset(); });

  it("uses explicit project-scoped commands", async () => {
    invokeMock.mockResolvedValue({ autonomy: { status: "stopped" } });
    const configuration = {
      planningProposalId: "plan-1", reviewerAgentId: "reviewer-1",
      autoSchedule: true, autoReview: true, autoIntegrate: true,
      maxTasksPerCycle: 2, maxAutoRetries: 1,
      pauseOnFailure: true, pauseOnNeedsInput: true,
    };

    await getProjectAutonomy("project-1");
    await updateProjectAutonomy("project-1", configuration);
    await startProjectAutonomy("project-1");
    await pauseProjectAutonomy("project-1");
    await stopProjectAutonomy("project-1");
    await advanceProjectAutonomy("project-1");

    expect(invokeMock.mock.calls).toEqual([
      ["get_project_autonomy", { projectId: "project-1" }],
      ["update_project_autonomy", { input: { projectId: "project-1", ...configuration } }],
      ["start_project_autonomy", { projectId: "project-1" }],
      ["pause_project_autonomy", { projectId: "project-1" }],
      ["stop_project_autonomy", { projectId: "project-1" }],
      ["advance_project_autonomy", { projectId: "project-1" }],
    ]);
  });

  it("subscribes to durable autonomy changes", async () => {
    const unlisten = vi.fn();
    const handler = vi.fn();
    listenMock.mockResolvedValue(unlisten);
    await expect(listenToAutonomyEvents(handler)).resolves.toBe(unlisten);
    const eventHandler = listenMock.mock.calls[0][1];
    eventHandler({ payload: "project-1" });
    expect(handler).toHaveBeenCalledWith("project-1");
  });
});
