import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  answerTaskInput,
  createProjectBlocker,
  listProjectBlockers,
  listTaskInputRequests,
  requestTaskInput,
  resolveProjectBlocker,
} from "./interruptions";

describe("input and blocker service", () => {
  beforeEach(() => invokeMock.mockReset());

  it("persists and answers task input requests", async () => {
    invokeMock.mockResolvedValue({ id: "input-1" });
    await requestTaskInput("task-1", "Which API?", "run-1");
    expect(invokeMock).toHaveBeenLastCalledWith("request_task_input", {
      input: { taskId: "task-1", question: "Which API?", runId: "run-1" },
    });

    await listTaskInputRequests("task-1");
    expect(invokeMock).toHaveBeenLastCalledWith("list_task_input_requests", { taskId: "task-1" });

    await answerTaskInput("input-1", "Use v2.");
    expect(invokeMock).toHaveBeenLastCalledWith("answer_task_input", {
      input: { requestId: "input-1", answer: "Use v2." },
    });
  });

  it("creates, lists, and resolves scoped project blockers", async () => {
    const input = {
      projectId: "project-1",
      title: "SDK unavailable",
      affectsAllTasks: false,
      affectedTaskIds: ["task-1"],
    };
    invokeMock.mockResolvedValue({ id: "blocker-1" });

    await createProjectBlocker(input);
    expect(invokeMock).toHaveBeenLastCalledWith("create_project_blocker", { input });
    await listProjectBlockers("project-1");
    expect(invokeMock).toHaveBeenLastCalledWith("list_project_blockers", {
      projectId: "project-1",
    });
    await resolveProjectBlocker("blocker-1");
    expect(invokeMock).toHaveBeenLastCalledWith("resolve_project_blocker", {
      blockerId: "blocker-1",
    });
  });
});
