import { describe, expect, it } from "vitest";
import type { Agent } from "../../services/agents";
import type { ProjectWorkflowSnapshot, WorkflowTaskView } from "../../services/workflow";
import { deriveBoardIndicators } from "./BoardPageModel";

const projectedTask: WorkflowTaskView = {
  id: "task-1", projectId: "project-1", title: "Land it", priority: "high", status: "approved", stage: "verify", position: 0,
  statusChangedAt: "2026-08-30T10:00:00Z", nextAction: { kind: "integrate", label: "Integrate" }, createdAt: "2026-08-30T09:00:00Z", updatedAt: "2026-08-30T10:00:00Z",
};

const workflow: ProjectWorkflowSnapshot = {
  projectId: "project-1",
  generatedAt: "2026-08-30T10:00:00Z",
  health: { projectId: "project-1", status: "healthy", lastValidationAttemptId: null, lastSuccessfulValidationAt: null, lastIntegrationAt: null, failingGate: null, updatedAt: "2026-08-30T10:00:00Z" },
  stages: [{ id: "verify", label: "Verify & Land", totalCount: 1, tasks: [projectedTask] }],
  attention: [
    { id: "blocker", kind: "project_blocker", severity: "high", title: "Blocked", createdAt: "2026-08-30T10:00:00Z" },
    { id: "plan", kind: "planning_approval", severity: "normal", title: "Plan", createdAt: "2026-08-30T10:00:00Z" },
    { id: "collab", kind: "collaboration", severity: "normal", title: "Handoff", createdAt: "2026-08-30T10:00:00Z" },
  ],
  agentActivity: [
    { id: "running", agentId: "agent-1", agentName: "Builder", role: "implementer", activityType: "implementation", status: "running", startedAt: "2026-08-30T10:00:00Z" },
    { id: "queued", agentId: "agent-2", agentName: "Reviewer", role: "reviewer", activityType: "review", status: "queued", startedAt: "2026-08-30T10:00:00Z" },
  ],
  idleAgentCount: 0,
};

const agent: Agent = { id: "agent-1", name: "Builder", provider: "codex", role: "implementer", model: null, systemPrompt: null, skills: [], maxConcurrentTasks: 2, createdAt: "2026-08-30T09:00:00Z", updatedAt: "2026-08-30T09:00:00Z" };

describe("deriveBoardIndicators", () => {
  it("uses the snapshot as the source for workflow-sensitive header counts", () => {
    const indicators = deriveBoardIndicators({ workflow, blockers: [], integrations: [], proposals: [], collaboration: [], decisions: [], agents: [agent] });

    expect(indicators).toEqual({ activeBlockerCount: 1, queuedIntegrationCount: 1, proposedPlanCount: 1, openCollaborationCount: 1, acceptedDecisionCount: 0, activeFlowCount: 1, queuedFlowCount: 1, flowCapacity: 2 });
  });

  it("uses loaded controls when no snapshot exists and honors configured flow capacity", () => {
    const indicators = deriveBoardIndicators({
      blockers: [{ status: "active" }, { status: "resolved" }] as never,
      integrations: [{ status: "queued" }, { status: "merged" }] as never,
      proposals: [{ status: "proposed" }, { status: "approved" }] as never,
      collaboration: [{ parentId: null, status: "open" }, { parentId: "parent", status: "open" }] as never,
      decisions: [{ status: "accepted" }, { status: "proposed" }] as never,
      flow: { activeWorkerRuns: 3, queued: 4, limits: { workerMaxConcurrentRuns: 5 } } as never,
      agents: [agent],
    });

    expect(indicators).toEqual({ activeBlockerCount: 1, queuedIntegrationCount: 1, proposedPlanCount: 1, openCollaborationCount: 1, acceptedDecisionCount: 1, activeFlowCount: 3, queuedFlowCount: 4, flowCapacity: 5 });
  });
});
