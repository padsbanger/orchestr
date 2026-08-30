import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, listenMock } = vi.hoisted(() => ({ invokeMock: vi.fn(), listenMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import { agentActivityDestination, attentionDestination, canChangePlanningStatus, canReorderPlanningTask, createFallbackWorkflowSnapshot, getProjectWorkflowSnapshot, listenToWorkflowChanges, loadWorkflowBoardView, mergeWorkflowSnapshots, parseWorkflowBoardView, parseWorkflowTimestamp, saveWorkflowBoardView, workflowBoardViewSettingKey } from "./workflow";
import type { Agent } from "./agents";
import type { Task, TaskStatus } from "./tasks";

function task(id: string, status: TaskStatus, assignedAgentId: string | null = null): Task {
  return {
    id, projectId: "project-1", title: `Task ${id}`, description: null, acceptanceCriteria: ["Works"], implementationNotes: null,
    relevantPaths: [], requiredCapabilities: [], dependencyIds: [], assignedAgentId, branch: null, worktreePath: null, priority: "normal",
    blockedReason: status === "blocked" ? "Waiting for a dependency" : null, milestoneId: null, epicId: null, status, position: 0,
    createdAt: "2026-08-30T10:00:00.000Z", updatedAt: "2026-08-30T11:00:00.000Z",
  };
}

const agent: Agent = {
  id: "agent-1", name: "Builder", provider: "codex", role: "implementer", model: null, systemPrompt: null, skills: [], maxConcurrentTasks: 1,
  createdAt: "2026-08-30T10:00:00.000Z", updatedAt: "2026-08-30T10:00:00.000Z",
};

describe("workflow cockpit service", () => {
  beforeEach(() => { invokeMock.mockReset(); listenMock.mockReset(); });

  it("loads and normalizes the transitional nested task shape", async () => {
    const nestedTask = task("task-1", "ready", "agent-1");
    invokeMock.mockResolvedValue({
      projectId: "project-1", generatedAt: nestedTask.updatedAt,
      health: { projectId: "project-1", status: "healthy", lastValidationAttemptId: null, lastSuccessfulValidationAt: null, lastIntegrationAt: null, failingGate: null, updatedAt: nestedTask.updatedAt },
      stages: [{ id: "queue", label: "Queue", tasks: [{ task: nestedTask, stage: "queue", statusChangedAt: nestedTask.updatedAt, currentActor: { kind: "agent", id: "agent-1", label: "Builder" }, nextAction: { kind: "start", label: "Start work" } }] }],
      attention: [], agentActivity: [], idleAgentCount: 0,
    });

    const snapshot = await getProjectWorkflowSnapshot("project-1");

    expect(invokeMock).toHaveBeenCalledWith("get_project_workflow_snapshot", { projectId: "project-1" });
    expect(snapshot.stages[0].totalCount).toBe(1);
    expect(snapshot.stages[0].tasks[0]).toMatchObject({ id: "task-1", status: "ready", stage: "queue", assignedAgentId: "agent-1" });
  });

  it("forwards project-scoped workflow change events", async () => {
    const unlisten = vi.fn();
    const handler = vi.fn();
    listenMock.mockResolvedValue(unlisten);
    await expect(listenToWorkflowChanges(handler)).resolves.toBe(unlisten);
    const listener = listenMock.mock.calls[0][1] as (event: { payload: unknown }) => void;
    const payload = { projectId: "project-1", reason: "task_moved", taskId: "task-1" };
    listener({ payload });
    expect(handler).toHaveBeenCalledWith(payload);
  });

  it("projects every canonical status once and surfaces only actionable task attention", () => {
    const statuses: TaskStatus[] = ["backlog", "ready", "in_progress", "needs_input", "review", "approved", "integrating", "blocked", "done"];
    const snapshot = createFallbackWorkflowSnapshot({
      projectId: "project-1",
      tasks: statuses.map((status, index) => task(`task-${index}`, status, status === "ready" || status === "in_progress" ? "agent-1" : null)),
      agents: [agent],
    });

    const projected = snapshot.stages.flatMap((stage) => stage.tasks);
    expect(projected).toHaveLength(statuses.length);
    expect(new Set(projected.map((item) => item.id)).size).toBe(statuses.length);
    expect(snapshot.stages.find((stage) => stage.id === "verify")?.tasks.map((item) => item.status)).toEqual(["review", "approved", "integrating"]);
    expect(snapshot.attention.map((item) => item.kind)).toEqual(["needs_input", "review_approval"]);
    expect(snapshot.agentActivity.map((item) => item.status)).toEqual(["waiting", "running"]);
  });

  it("treats timezone-free SQLite timestamps as UTC", () => {
    expect(parseWorkflowTimestamp("2026-08-30 11:00:00")).toBe(parseWorkflowTimestamp("2026-08-30T11:00:00Z"));
  });

  it("keeps an authoritative blocked origin in Verify without a Queue duplicate", () => {
    const fallback = createFallbackWorkflowSnapshot({ projectId: "project-1", tasks: [task("blocked-task", "blocked")], agents: [] });
    const blocked = fallback.stages.find((stage) => stage.id === "queue")!.tasks[0];
    const snapshot = {
      ...fallback,
      stages: fallback.stages.map((stage) => stage.id === "queue"
        ? { ...stage, totalCount: 0, tasks: [] }
        : stage.id === "verify"
          ? { ...stage, totalCount: 1, tasks: [{ ...blocked, stage: "verify" as const }] }
          : stage),
    };

    const merged = mergeWorkflowSnapshots(snapshot, fallback)!;

    expect(merged.stages.find((stage) => stage.id === "queue")!.tasks).toHaveLength(0);
    expect(merged.stages.find((stage) => stage.id === "verify")!.tasks.map((item) => item.id)).toEqual(["blocked-task"]);
    expect(merged.stages.flatMap((stage) => stage.tasks).filter((item) => item.id === "blocked-task")).toHaveLength(1);
  });

  it("routes recovery and collaboration attention to the controls that resolve it", () => {
    const base = { id: "alert", severity: "high" as const, title: "Needs recovery", taskId: "task-1", createdAt: "2026-08-30T10:00:00Z" };
    expect(attentionDestination({ ...base, kind: "integration_recovery" })).toEqual({ kind: "panel", panel: "integration" });
    expect(attentionDestination({ ...base, kind: "collaboration" })).toEqual({ kind: "panel", panel: "collaboration" });
    expect(attentionDestination({ ...base, kind: "review_approval" })).toEqual({ kind: "task", taskId: "task-1" });
  });

  it("routes task activity to its inspector and taskless planning to Planning", () => {
    const base = { id: "activity", agentId: "agent-1", agentName: "Builder", role: "implementer", status: "running" as const, startedAt: "2026-08-30T10:00:00Z" };
    expect(agentActivityDestination({ ...base, activityType: "implementation", taskId: "task-1" })).toEqual({ kind: "task", taskId: "task-1" });
    expect(agentActivityDestination({ ...base, activityType: "planning" })).toEqual({ kind: "panel", panel: "planning" });
    expect(agentActivityDestination({ ...base, activityType: "assigned" })).toEqual({ kind: "route", route: "agents" });
  });

  it("only permits planning reorders within one status and explicit Draft/Ready changes", () => {
    expect(canReorderPlanningTask("backlog", "backlog")).toBe(true);
    expect(canReorderPlanningTask("ready", "ready")).toBe(true);
    expect(canReorderPlanningTask("ready", "backlog")).toBe(false);
    expect(canReorderPlanningTask("review", "review")).toBe(false);
    expect(canChangePlanningStatus("backlog", "ready")).toBe(true);
    expect(canChangePlanningStatus("ready", "backlog")).toBe(true);
    expect(canChangePlanningStatus("review", "ready")).toBe(false);
    expect(canChangePlanningStatus("ready", "review")).toBe(false);
  });

  it("loads and persists the per-project Flow preference contract", async () => {
    expect(workflowBoardViewSettingKey("project-1")).toBe("board.view.project-1");
    expect(parseWorkflowBoardView(null)).toBe("flow");
    expect(parseWorkflowBoardView("full_lifecycle")).toBe("lifecycle");
    invokeMock.mockResolvedValueOnce("lifecycle").mockResolvedValueOnce(undefined);
    await expect(loadWorkflowBoardView("project-1")).resolves.toBe("lifecycle");
    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_setting", { key: "board.view.project-1" });
    await saveWorkflowBoardView("project-1", "flow");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "set_setting", { key: "board.view.project-1", value: "flow" });
  });

  it("falls back to the newer local status when snapshot and task timestamps tie", () => {
    const localTask = { ...task("moving", "in_progress"), updatedAt: "2026-08-30 11:00:00" };
    const fallback = createFallbackWorkflowSnapshot({ projectId: "project-1", tasks: [localTask], agents: [] });
    const staleReady = { ...fallback.stages.find((stage) => stage.id === "build")!.tasks[0], status: "ready" as const, stage: "queue" as const };
    const snapshot = {
      ...fallback,
      stages: fallback.stages.map((stage) => stage.id === "build"
        ? { ...stage, totalCount: 0, tasks: [] }
        : stage.id === "queue"
          ? { ...stage, totalCount: 1, tasks: [staleReady] }
          : stage),
    };

    const merged = mergeWorkflowSnapshots(snapshot, fallback)!;

    expect(merged.stages.find((stage) => stage.id === "queue")!).toMatchObject({ totalCount: 0, tasks: [] });
    expect(merged.stages.find((stage) => stage.id === "build")!.tasks[0]).toMatchObject({ id: "moving", status: "in_progress" });
  });

  it("does not manufacture attention for ordinary human collaboration", () => {
    const entry = {
      id: "entry-1", projectId: "project-1", taskId: "task-1", parentId: null, authorAgentId: null, authorRunId: null,
      kind: "request" as const, message: "Please check this", status: "open" as const, referencedTaskIds: [], createdAt: "2026-08-30T10:00:00Z", resolvedAt: null,
    };
    const snapshot = createFallbackWorkflowSnapshot({
      projectId: "project-1", tasks: [], agents: [],
      collaboration: [{ ...entry, authorType: "human" }, { ...entry, id: "entry-2", authorType: "agent" }],
    });

    expect(snapshot.attention.filter((item) => item.kind === "collaboration").map((item) => item.id)).toEqual(["collaboration:entry-2"]);
  });
});
