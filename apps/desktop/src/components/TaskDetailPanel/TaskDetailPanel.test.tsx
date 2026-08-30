// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import type { Agent } from "../../services/agents";
import type { AgentReview } from "../../services/agentReviews";
import type { IntegrationAttempt, RevertAttempt } from "../../services/integrations";
import type { TaskInputRequest } from "../../services/interruptions";
import type { ValidationAttempt } from "../../services/quality";
import type { TaskRun } from "../../services/runs";
import type { Task } from "../../services/tasks";
import type { WorkflowTaskView } from "../../services/workflow";
import { TaskDetailPanel } from "./TaskDetailPanel";

vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn() }));

Object.defineProperty(HTMLElement.prototype, "scrollTo", {
  configurable: true,
  value: vi.fn(),
});

afterEach(() => {
  cleanup();
});

const agent: Agent = {
  id: "agent-1",
  name: "Builder",
  provider: "codex",
  role: "implementer",
  model: "gpt-5",
  systemPrompt: null,
  skills: [],
  maxConcurrentTasks: 1,
  createdAt: "2026-08-30T10:00:00Z",
  updatedAt: "2026-08-30T10:00:00Z",
};

const reviewer: Agent = {
  ...agent,
  id: "agent-2",
  name: "Architect",
  role: "reviewer",
};

const baseTask: Task = {
  id: "task-1",
  projectId: "project-1",
  title: "Simplify the inspector",
  description: "Keep the workflow legible.",
  acceptanceCriteria: ["All actions remain available"],
  implementationNotes: null,
  relevantPaths: [],
  requiredCapabilities: [],
  dependencyIds: [],
  assignedAgentId: agent.id,
  branch: "task/task-1",
  worktreePath: "C:/worktrees/task-1",
  priority: "high",
  blockedReason: null,
  milestoneId: null,
  epicId: null,
  status: "ready",
  position: 0,
  createdAt: "2026-08-30T10:00:00Z",
  updatedAt: "2026-08-30T10:00:00Z",
};

function workflowView(task: Task, stage: WorkflowTaskView["stage"] = "queue"): WorkflowTaskView {
  return {
    id: task.id,
    projectId: task.projectId,
    title: task.title,
    priority: task.priority,
    status: task.status,
    stage,
    position: task.position,
    statusChangedAt: "2026-08-30T11:00:00Z",
    currentActor: { kind: "agent", id: agent.id, label: "Builder" },
    nextAction: { kind: "start", label: "Queue Builder to start isolated implementation." },
    readiness: { ready: task.status === "ready", reason: task.status === "ready" ? "All dependencies are integrated." : "Waiting for workflow requirements." },
    assignedAgentId: task.assignedAgentId,
    blockedReason: task.blockedReason,
    milestoneId: task.milestoneId,
    epicId: task.epicId,
    createdAt: task.createdAt,
    updatedAt: task.updatedAt,
  };
}

function renderPanel(task: Task, overrides: Partial<ComponentProps<typeof TaskDetailPanel>> = {}) {
  const props: ComponentProps<typeof TaskDetailPanel> = {
    task,
    workflowView: workflowView(task),
    assignedAgent: agent,
    recoveryAgents: [agent],
    reviewerAgents: [],
    agentReviews: [],
    inputRequests: [],
    architectureDecisions: [],
    isAgentReviewStarting: false,
    runs: [],
    isStartingRun: false,
    isCleaningWorktree: false,
    isOpeningWorktree: false,
    isReviewLoading: false,
    isReviewActionPending: false,
    onClose: vi.fn(),
    onEdit: vi.fn(),
    onStartRun: vi.fn(),
    onCancelRun: vi.fn(),
    onRecoverRun: vi.fn(),
    onResolveRunFailure: vi.fn(),
    onRequestInput: vi.fn(),
    onAnswerInput: vi.fn(),
    onCleanupWorktree: vi.fn(),
    onOpenWorktree: vi.fn(),
    onApproveReview: vi.fn(),
    onRequestChanges: vi.fn(),
    onStartAgentReview: vi.fn(),
    ...overrides,
  };
  return render(<TaskDetailPanel {...props} />);
}

describe("TaskDetailPanel phase tabs", () => {
  it("opens queue tasks on Work and keeps the run action reachable from Activity", async () => {
    const user = userEvent.setup();
    renderPanel(baseTask);

    expect(screen.getByRole("tab", { name: "Work" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByText("Queue Builder to start isolated implementation.")).toBeTruthy();

    await user.click(screen.getByRole("tab", { name: "Activity" }));

    expect(screen.getByRole("tab", { name: "Activity" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("button", { name: "Queue with Codex" })).toBeTruthy();
  });

  it("uses arrow-key tab navigation with a single selected tab", async () => {
    const user = userEvent.setup();
    renderPanel(baseTask);
    const workTab = screen.getByRole("tab", { name: "Work" });
    workTab.focus();

    await user.keyboard("{ArrowRight}");

    expect(screen.getByRole("tab", { name: "Activity" }).getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(screen.getByRole("tab", { name: "Activity" }));

    await user.keyboard("{End}");
    expect(screen.getByRole("tab", { name: "Review & Land" }).getAttribute("aria-selected")).toBe("true");
  });

  it("opens review tasks on Review & Land with approval actions intact", () => {
    const reviewTask = { ...baseTask, status: "review" as const };
    renderPanel(
      reviewTask,
      {
        workflowView: workflowView(reviewTask, "verify"),
        review: {
          branch: "task/task-1",
          baseBranch: "integration",
          commits: [],
          diff: "diff --git a/file.ts b/file.ts",
          changedFiles: [],
        },
      },
    );

    expect(screen.getByRole("tab", { name: "Review & Land" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("button", { name: "Approve for integration" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Request changes" })).toBeTruthy();
  });

  it("uses the authoritative blocked origin, actor, action, and readiness projection", () => {
    const blocked = { ...baseTask, status: "blocked" as const, blockedReason: "Integration conflict" };
    renderPanel(blocked, {
      workflowView: {
        ...workflowView(blocked, "verify"),
        currentActor: { kind: "system", label: "Integration service" },
        nextAction: { kind: "retry", label: "Recover integration", reason: "Resolve conflicts" },
        readiness: { ready: false, reason: "Conflict resolution is required." },
      },
    });

    expect(screen.getByRole("tab", { name: "Review & Land" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByText("Integration service")).toBeTruthy();
    expect(screen.getByText("Recover integration: Resolve conflicts")).toBeTruthy();
    expect(screen.getByText("Verify & Land")).toBeTruthy();
  });

  it("reports the selected tab so the board can load details lazily", async () => {
    const onTabChange = vi.fn();
    const user = userEvent.setup();
    renderPanel(baseTask, { onTabChange });
    expect(onTabChange).toHaveBeenLastCalledWith("work");

    await user.click(screen.getByRole("tab", { name: "Activity" }));

    expect(onTabChange).toHaveBeenLastCalledWith("activity");
  });

  it("recovers failed runs without hiding their execution evidence", async () => {
    const user = userEvent.setup();
    const onRecoverRun = vi.fn();
    const onResolveRunFailure = vi.fn();
    const failedTask = { ...baseTask, status: "in_progress" as const };
    const failedRun: TaskRun = {
      id: "run-1",
      taskId: failedTask.id,
      agentId: agent.id,
      workerId: "local",
      status: "failed",
      startedAt: "2026-08-30T10:00:00Z",
      completedAt: "2026-08-30T10:02:00Z",
      exitCode: 1,
      error: "Tests failed",
      output: [],
      events: [
        { id: 1, kind: "command.started", message: "in_progress: npm test", command: "npm test", filePath: null, exitCode: null, createdAt: "2026-08-30T10:00:10Z" },
        { id: 2, kind: "command.completed", message: "completed: npm test", command: "npm test", filePath: null, exitCode: 0, createdAt: "2026-08-30T10:01:00Z" },
        { id: 3, kind: "file.modified", message: "Updated the workflow panel", command: null, filePath: "src/Workflow.tsx", exitCode: null, createdAt: "2026-08-30T10:01:30Z" },
      ],
    };

    renderPanel(failedTask, {
      workflowView: workflowView(failedTask, "build"),
      runs: [failedRun],
      recoveryAgents: [agent, reviewer],
      onRecoverRun,
      onResolveRunFailure,
    });

    expect(screen.getByText("Run needs recovery")).toBeTruthy();
    expect(screen.getByText("Running JavaScript task")).toBeTruthy();
    expect(screen.getByText("Finished JavaScript task")).toBeTruthy();
    expect(screen.getByText("Changed files")).toBeTruthy();
    expect(screen.getByText("Tests failed")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Resume worktree" }));
    expect(onRecoverRun).toHaveBeenCalledWith("run-1", "resume");
    await user.click(screen.getByRole("button", { name: "Retry with agent" }));
    expect(onRecoverRun).toHaveBeenCalledWith("run-1", "resume", reviewer.id);
    await user.click(screen.getByRole("button", { name: "Escalate as blocked" }));
    expect(onResolveRunFailure).toHaveBeenCalledWith("run-1", "escalate");
  });

  it("answers open input requests and preserves answered decisions", async () => {
    const user = userEvent.setup();
    const onAnswerInput = vi.fn();
    const inputTask = { ...baseTask, status: "needs_input" as const };
    const requests: TaskInputRequest[] = [
      {
        id: "input-open",
        taskId: inputTask.id,
        requestingRunId: "run-1",
        requestingAgentId: agent.id,
        question: "Which compatibility target should we use?",
        status: "open",
        answer: null,
        requestedAt: "2026-08-30T10:00:00Z",
        answeredAt: null,
      },
      {
        id: "input-answered",
        taskId: inputTask.id,
        requestingRunId: null,
        requestingAgentId: agent.id,
        question: "Should the fallback remain?",
        status: "answered",
        answer: "Yes, during rollout.",
        requestedAt: "2026-08-29T10:00:00Z",
        answeredAt: "2026-08-29T10:10:00Z",
      },
    ];

    renderPanel(inputTask, {
      workflowView: workflowView(inputTask, "build"),
      inputRequests: requests,
      onAnswerInput,
    });

    expect(screen.getByText("Which compatibility target should we use?")).toBeTruthy();
    expect(screen.getByText(/Answered requests/)).toBeTruthy();
    await user.type(screen.getByPlaceholderText("Record the decision or missing information..."), "Support the latest two releases.");
    await user.click(screen.getByRole("button", { name: "Answer and resume" }));
    expect(onAnswerInput).toHaveBeenCalledWith("input-open", "Support the latest two releases.");
  });

  it("shows durable architect outcomes and keeps cancellation available", async () => {
    const user = userEvent.setup();
    const onCancelRun = vi.fn();
    const reviewTask = { ...baseTask, status: "review" as const };
    const review = (id: string, status: AgentReview["status"], decision: AgentReview["decision"] = null): AgentReview => ({
      id,
      taskId: reviewTask.id,
      agentId: reviewer.id,
      status,
      decision,
      notes: status === "completed" ? `Notes for ${id}` : null,
      rawOutput: status === "running" ? "Inspecting diff\nChecking tests" : "",
      error: status === "failed" ? "Provider unavailable" : null,
      startedAt: "2026-08-30T10:00:00Z",
      completedAt: status === "running" ? null : "2026-08-30T10:01:00Z",
    });
    const reviews = [
      review("running-review", "running"),
      review("failed-review", "failed"),
      review("cancelled-review", "cancelled"),
      review("approved-review", "completed", "approve"),
      review("changes-review", "completed", "request_changes"),
      review("completed-review", "completed"),
    ];

    renderPanel(reviewTask, {
      workflowView: workflowView(reviewTask, "verify"),
      reviewerAgents: [reviewer],
      agentReviews: reviews,
      onCancelRun,
    });

    expect(screen.getByText("Architect review in progress")).toBeTruthy();
    expect(screen.getByText("Architect review failed")).toBeTruthy();
    expect(screen.getByText("Architect review cancelled")).toBeTruthy();
    expect(screen.getByText("Architect approved the implementation")).toBeTruthy();
    expect(screen.getByText("Architect requested changes")).toBeTruthy();
    expect(screen.getByText("Architect review completed")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Cancel architect" }));
    expect(onCancelRun).toHaveBeenCalledWith("running-review");
  });

  it("renders validation, integration cleanup, and revert history on demand", () => {
    const doneTask = { ...baseTask, status: "done" as const };
    const integration: IntegrationAttempt = {
      id: "integration-1",
      taskId: doneTask.id,
      sourceBranch: "task/task-1",
      targetBranch: "integration",
      status: "merged",
      queuePosition: 0,
      mergeCommit: "abc123",
      error: "Worktree cleanup requires retry",
      createdAt: "2026-08-30T10:00:00Z",
      startedAt: "2026-08-30T10:01:00Z",
      completedAt: "2026-08-30T10:02:00Z",
    };
    const validation: ValidationAttempt = {
      id: "validation-1",
      projectId: doneTask.projectId,
      taskId: doneTask.id,
      integrationAttemptId: integration.id,
      stage: "integration",
      status: "passed",
      error: null,
      startedAt: "2026-08-30T10:01:00Z",
      completedAt: "2026-08-30T10:02:00Z",
      events: [],
    };
    const revert: RevertAttempt = {
      id: "revert-1",
      projectId: doneTask.projectId,
      originalTaskId: doneTask.id,
      integrationAttemptId: integration.id,
      originalCommit: "abc123",
      status: "reverted",
      revertCommit: "def456",
      repairTaskId: null,
      error: null,
      startedAt: "2026-08-30T11:00:00Z",
      completedAt: "2026-08-30T11:01:00Z",
    };

    renderPanel(doneTask, {
      workflowView: workflowView(doneTask, "done"),
      integrationAttempts: [integration],
      validationAttempts: [validation],
      revertAttempts: [revert],
    });

    expect(screen.getByText("integration validation")).toBeTruthy();
    expect(screen.getByText("Cleanup needs recovery: Worktree cleanup requires retry")).toBeTruthy();
    expect(screen.getByText("def456")).toBeTruthy();
  });

  it("explains incomplete readiness and unassigned work without inventing an actor", () => {
    const draftTask: Task = {
      ...baseTask,
      acceptanceCriteria: [],
      relevantPaths: [],
      requiredCapabilities: [],
      dependencyIds: ["dependency-1"],
      assignedAgentId: null,
      branch: null,
      worktreePath: null,
      status: "blocked",
      blockedReason: "Dependency is not integrated",
    };
    renderPanel(draftTask, {
      workflowView: {
        ...workflowView(draftTask),
        currentActor: undefined,
        readiness: { ready: false, reason: "Dependency is not Done." },
      },
      assignedAgent: undefined,
    });

    expect(screen.getByText("Acceptance criteria required")).toBeTruthy();
    expect(screen.getByText("Dependency is not Done.")).toBeTruthy();
    expect(screen.getByText("No agent assigned.")).toBeTruthy();
    expect(screen.getAllByText("Dependency is not integrated")).toHaveLength(2);
    expect(screen.getByText("Unassigned")).toBeTruthy();
  });
});
