import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  createArchitectureDecision,
  decideArchitectureDecision,
  listArchitectureDecisions,
  listRelevantArchitectureDecisions,
} from "./knowledge";

describe("project knowledge service", () => {
  beforeEach(() => invokeMock.mockReset());

  it("lists the registry and task context preview", async () => {
    invokeMock.mockResolvedValue([]);
    await listArchitectureDecisions("project-1");
    expect(invokeMock).toHaveBeenLastCalledWith("list_architecture_decisions", { projectId: "project-1" });
    await listRelevantArchitectureDecisions("task-1");
    expect(invokeMock).toHaveBeenLastCalledWith("list_relevant_architecture_decisions", { taskId: "task-1" });
  });

  it("creates and explicitly decides proposals", async () => {
    invokeMock.mockResolvedValue({ id: "adr-1" });
    const input = {
      projectId: "project-1",
      title: "Use SQLite",
      context: "Local-first metadata",
      decision: "Persist metadata in SQLite.",
      relevantPaths: [],
      relevantTaskIds: [],
    };
    await createArchitectureDecision(input);
    expect(invokeMock).toHaveBeenLastCalledWith("create_architecture_decision", { input });
    await decideArchitectureDecision("adr-1", "accepted");
    expect(invokeMock).toHaveBeenLastCalledWith("decide_architecture_decision", {
      decisionId: "adr-1",
      status: "accepted",
    });
  });
});
