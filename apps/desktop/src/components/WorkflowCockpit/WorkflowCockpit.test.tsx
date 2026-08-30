// @vitest-environment jsdom

import { DndContext } from "@dnd-kit/core";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Agent } from "../../services/agents";
import type { Task } from "../../services/tasks";
import { createFallbackWorkflowSnapshot, mergeWorkflowSnapshots, type AttentionItem } from "../../services/workflow";
import { AgentActivityRail, AttentionTray, FlowStageColumn, useWorkflowClock, WorkflowTaskCard } from "./WorkflowCockpit";

afterEach(() => { cleanup(); vi.useRealTimers(); });

const attention: AttentionItem[] = Array.from({ length: 6 }, (_, index) => ({
  id: `attention-${index + 1}`,
  kind: index === 0 ? "health_broken" : "review_approval",
  severity: index === 0 ? "critical" : "normal",
  title: `Attention ${index + 1}`,
  createdAt: `2026-08-30 10:0${index}:00`,
}));

function AttentionHarness({ onOpen }: { onOpen: (item: AttentionItem) => void }) {
  const [expanded, setExpanded] = useState(false);
  return <AttentionTray items={attention} expanded={expanded} onToggle={() => setExpanded((current) => !current)} onOpen={onOpen} />;
}

const idleAgent: Agent = {
  id: "agent-1", name: "Idle architect", provider: "codex", role: "architect", model: null, systemPrompt: null, skills: [], maxConcurrentTasks: 1,
  createdAt: "2026-08-30T10:00:00Z", updatedAt: "2026-08-30T10:00:00Z",
};

function AgentRailHarness() {
  const [showIdle, setShowIdle] = useState(false);
  return <AgentActivityRail activities={[]} idleAgents={[idleAgent]} idleCount={1} isOpen isDrawer showIdle={showIdle} now={Date.now()} onClose={vi.fn()} onToggleIdle={() => setShowIdle((current) => !current)} onOpen={vi.fn()} />;
}

describe("WorkflowCockpit", () => {
  it("limits Attention to five items and expands the full actionable list", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    render(<AttentionHarness onOpen={onOpen} />);

    expect(screen.getByRole("button", { name: /attention 5/i })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /attention 6/i })).toBeNull();

    await user.click(screen.getByRole("button", { name: /view all 6/i }));
    await user.click(screen.getByRole("button", { name: /attention 6/i }));

    expect(screen.getByRole("button", { name: /show less/i })).toBeTruthy();
    expect(onOpen).toHaveBeenCalledWith(attention[5]);
  });

  it("keeps idle agent identities collapsed until requested", async () => {
    const user = userEvent.setup();
    render(<AgentRailHarness />);
    expect(screen.queryByText("Idle architect")).toBeNull();
    await user.click(screen.getByRole("button", { name: /1 idle/i }));
    expect(screen.getByText("Idle architect")).toBeTruthy();
  });

  it("renders a blocked task once in its authoritative Verify origin", () => {
    const blockedTask: Task = {
      id: "blocked-task", projectId: "project-1", title: "Blocked delivery", description: null, acceptanceCriteria: ["Works"], implementationNotes: null,
      relevantPaths: [], requiredCapabilities: [], dependencyIds: [], assignedAgentId: null, branch: null, worktreePath: null, priority: "normal",
      blockedReason: "Integration conflict", milestoneId: null, epicId: null, status: "blocked", position: 0,
      createdAt: "2026-08-30T10:00:00Z", updatedAt: "2026-08-30T11:00:00Z",
    };
    const fallback = createFallbackWorkflowSnapshot({ projectId: "project-1", tasks: [blockedTask], agents: [] });
    const blockedView = fallback.stages.find((stage) => stage.id === "queue")!.tasks[0];
    const authoritative = {
      ...fallback,
      stages: fallback.stages.map((stage) => stage.id === "queue"
        ? { ...stage, totalCount: 0, tasks: [] }
        : stage.id === "verify"
          ? { ...stage, totalCount: 1, tasks: [{ ...blockedView, stage: "verify" as const }] }
          : stage),
    };
    const merged = mergeWorkflowSnapshots(authoritative, fallback)!;
    const actions = { onInspect: vi.fn(), onEdit: vi.fn(), onDelete: vi.fn(), onPlanningState: vi.fn() };

    render(<DndContext><>{merged.stages.filter((stage) => stage.id === "queue" || stage.id === "verify").map((stage) => <FlowStageColumn key={stage.id} stage={stage.id} label={stage.label} totalCount={stage.totalCount} taskViews={stage.tasks} tasksById={new Map([[blockedTask.id, blockedTask]])} activeOnMobile showAllDone={false} now={Date.now()} onToggleDone={vi.fn()} recentlyTransitionedTaskIds={[]} {...actions} />)}</></DndContext>);

    const queue = screen.getByRole("heading", { name: "Queue" }).closest("section")!;
    const verify = screen.getByRole("heading", { name: "Verify & Land" }).closest("section")!;
    expect(within(queue).queryByText("Blocked delivery")).toBeNull();
    expect(within(verify).getByText("Blocked delivery")).toBeTruthy();
    expect(screen.getAllByText("Blocked delivery")).toHaveLength(1);
  });

  it("hides a closed drawer from focus and accessibility traversal", () => {
    render(<AgentActivityRail activities={[]} idleAgents={[idleAgent]} idleCount={1} isOpen={false} isDrawer showIdle={false} now={Date.now()} onClose={vi.fn()} onToggleIdle={vi.fn()} onOpen={vi.fn()} />);
    const rail = document.querySelector(".agent-activity-rail")!;
    expect(rail.hasAttribute("hidden")).toBe(true);
    expect(rail.hasAttribute("inert")).toBe(true);
    expect(screen.queryByRole("button", { name: /1 idle/i })).toBeNull();
  });

  it("refreshes elapsed labels on the shared minute clock", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-30T12:01:30Z"));
    const activity = [{ id: "activity-1", agentId: "agent-1", agentName: "Builder", role: "implementer", activityType: "implementation" as const, status: "running" as const, startedAt: "2026-08-30T12:00:00Z" }];
    function ClockedRail() {
      const now = useWorkflowClock();
      return <AgentActivityRail activities={activity} idleAgents={[]} idleCount={0} isOpen isDrawer={false} showIdle={false} now={now} onClose={vi.fn()} onToggleIdle={vi.fn()} onOpen={vi.fn()} />;
    }
    render(<ClockedRail />);
    expect(screen.getByText("1m")).toBeTruthy();
    act(() => vi.advanceTimersByTime(60_000));
    expect(screen.getByText("2m")).toBeTruthy();
    vi.useRealTimers();
  });

  it("exposes drag handles only for Backlog and Ready planning work", () => {
    const task = (id: string, title: string, status: Task["status"]): Task => ({
      id,
      projectId: "project-1",
      title,
      description: null,
      acceptanceCriteria: ["Works"],
      implementationNotes: null,
      relevantPaths: [],
      requiredCapabilities: [],
      dependencyIds: [],
      assignedAgentId: null,
      branch: null,
      worktreePath: null,
      priority: "normal",
      blockedReason: null,
      milestoneId: null,
      epicId: null,
      status,
      position: 0,
      createdAt: "2026-08-30T10:00:00Z",
      updatedAt: "2026-08-30T10:00:00Z",
    });
    const draft = task("draft", "Draft planning", "backlog");
    const review = task("review", "Review delivery", "review");
    const actions = { onInspect: vi.fn(), onEdit: vi.fn(), onDelete: vi.fn(), onPlanningState: vi.fn() };

    render(<DndContext><WorkflowTaskCard task={draft} now={Date.now()} isRecentlyTransitioned={false} {...actions} /><WorkflowTaskCard task={review} now={Date.now()} isRecentlyTransitioned={false} {...actions} /></DndContext>);

    expect(screen.getByRole("button", { name: "Reorder Draft planning" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Reorder Review delivery" })).toBeNull();
  });
});
