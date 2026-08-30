import type { Agent } from "../../services/agents";
import type { CollaborationEntry } from "../../services/collaboration";
import type { FlowState } from "../../services/flow";
import type { IntegrationAttempt } from "../../services/integrations";
import type { ProjectBlocker } from "../../services/interruptions";
import type { ArchitectureDecision } from "../../services/knowledge";
import type { PlanningProposal } from "../../services/planning";
import type { ProjectWorkflowSnapshot } from "../../services/workflow";

type BoardIndicatorInput = {
  workflow?: ProjectWorkflowSnapshot;
  blockers: ProjectBlocker[];
  integrations: IntegrationAttempt[];
  proposals: PlanningProposal[];
  collaboration: CollaborationEntry[];
  decisions: ArchitectureDecision[];
  flow?: FlowState;
  agents: Agent[];
};

export type BoardIndicators = {
  activeBlockerCount: number;
  queuedIntegrationCount: number;
  proposedPlanCount: number;
  openCollaborationCount: number;
  acceptedDecisionCount: number;
  activeFlowCount: number;
  queuedFlowCount: number;
  flowCapacity: number;
};

/** Adapts snapshot-first cockpit data to the compact project-header counters. */
export function deriveBoardIndicators(input: BoardIndicatorInput): BoardIndicators {
  const projectedTasks = input.workflow?.stages.flatMap((stage) => stage.tasks);
  const activity = input.workflow?.agentActivity;
  return {
    activeBlockerCount: input.workflow
      ? input.workflow.attention.filter((item) => item.kind === "project_blocker").length
      : input.blockers.filter((blocker) => blocker.status === "active").length,
    queuedIntegrationCount: projectedTasks
      ? projectedTasks.filter((task) => task.status === "approved").length
      : input.integrations.filter((attempt) => attempt.status === "queued").length,
    proposedPlanCount: input.workflow
      ? input.workflow.attention.filter((item) => item.kind === "planning_approval").length
      : input.proposals.filter((proposal) => proposal.status === "proposed").length,
    openCollaborationCount: input.workflow
      ? input.workflow.attention.filter((item) => item.kind === "collaboration").length
      : input.collaboration.filter((entry) => !entry.parentId && entry.status === "open").length,
    acceptedDecisionCount: input.decisions.filter((decision) => decision.status === "accepted").length,
    activeFlowCount: input.flow?.activeWorkerRuns ?? activity?.filter((item) => item.status === "running").length ?? 0,
    queuedFlowCount: input.flow?.queued ?? activity?.filter((item) => item.status === "queued").length ?? 0,
    flowCapacity: input.flow?.limits.workerMaxConcurrentRuns ?? Math.max(1, input.agents.reduce((total, agent) => total + agent.maxConcurrentTasks, 0)),
  };
}
