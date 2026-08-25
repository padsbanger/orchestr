import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  listRevertAttempts,
  retryIntegrationCleanup,
  revertIntegration,
} from "./integrations";
import { recoverTaskRun, resolveFailedRun } from "./runs";

describe("failure recovery service", () => {
  beforeEach(() => invokeMock.mockReset());

  it("queues a worktree recovery with an optional replacement agent", async () => {
    invokeMock.mockResolvedValue({ run: { id: "run-2" }, task: { id: "task-1" } });

    await recoverTaskRun("run-1", "resume", "agent-2");

    expect(invokeMock).toHaveBeenCalledWith("recover_task_run", {
      input: { runId: "run-1", mode: "resume", agentId: "agent-2" },
    });
  });

  it("persists escalation and abandonment actions", async () => {
    invokeMock.mockResolvedValue({ id: "task-1" });

    await resolveFailedRun("run-1", "escalate", "Needs human investigation.");

    expect(invokeMock).toHaveBeenCalledWith("resolve_failed_run", {
      input: {
        runId: "run-1",
        action: "escalate",
        note: "Needs human investigation.",
      },
    });
  });

  it("retries cleanup independently from integration", async () => {
    invokeMock.mockResolvedValue({ id: "integration-1", status: "merged" });

    await retryIntegrationCleanup("integration-1");

    expect(invokeMock).toHaveBeenCalledWith("retry_integration_cleanup", {
      attemptId: "integration-1",
    });
  });

  it("loads revert history and requests an optional repair task", async () => {
    invokeMock.mockResolvedValue([]);
    await listRevertAttempts("project-1");
    expect(invokeMock).toHaveBeenLastCalledWith("list_revert_attempts", {
      projectId: "project-1",
    });

    invokeMock.mockResolvedValue({ id: "revert-1", status: "reverted" });
    await revertIntegration("integration-1", true);
    expect(invokeMock).toHaveBeenLastCalledWith("revert_integration", {
      input: { attemptId: "integration-1", createRepairTask: true },
    });
  });
});
