import { invoke } from "@tauri-apps/api/core";

export type ProjectCostControl = {
  projectId: string;
  monthlyBudgetMicros: number;
  warningThresholdPercent: number;
  blockNewRuns: boolean;
  updatedAt: string;
};

export type ModelPricing = {
  projectId: string;
  provider: string;
  model: string;
  inputMicrosPerMillion: number;
  cachedInputMicrosPerMillion: number;
  outputMicrosPerMillion: number;
  updatedAt: string;
};

export type OperationalMetrics = {
  runCount: number;
  completedRuns: number;
  failedRuns: number;
  cancelledRuns: number;
  retryCount: number;
  successRatePercent: number;
  averageDurationSeconds: number;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  estimatedCostMicros: number;
  unpricedRunCount: number;
};

export type FlowMetrics = {
  readyLeadTimeSeconds: number | null;
  inProgressSeconds: number | null;
  reviewQueueSeconds: number | null;
  integrationQueueSeconds: number | null;
  blockedSeconds: number | null;
  conflictRatePercent: number;
  validationFailureRatePercent: number;
  milestoneThroughput: number;
};

export type AgentMetric = {
  agentId: string;
  agentName: string;
  provider: string;
  model: string;
  runCount: number;
  successRatePercent: number;
  averageDurationSeconds: number;
  estimatedCostMicros: number;
};

export type WorkerMetric = {
  workerId: string;
  workerName: string;
  runCount: number;
  busySeconds: number;
  utilizationPercent: number;
};

export type CostMetric = {
  provider: string;
  model: string;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  estimatedCostMicros: number;
  priced: boolean;
};

export type ProjectMetrics = {
  rangeDays: number;
  operational: OperationalMetrics;
  flow: FlowMetrics;
  agents: AgentMetric[];
  workers: WorkerMetric[];
  costs: CostMetric[];
  costControl: ProjectCostControl;
  pricing: ModelPricing[];
  currentMonthCostMicros: number;
  budgetUtilizationPercent: number | null;
  budgetStatus: "unconfigured" | "within_budget" | "warning" | "exceeded";
};

export function getProjectMetrics(projectId: string, rangeDays: number): Promise<ProjectMetrics> {
  return invoke<ProjectMetrics>("get_project_metrics", { projectId, rangeDays });
}

export function updateProjectCostControl(input: {
  projectId: string;
  monthlyBudgetMicros: number;
  warningThresholdPercent: number;
  blockNewRuns: boolean;
}): Promise<ProjectCostControl> {
  return invoke<ProjectCostControl>("update_project_cost_control", { input });
}

export function upsertModelPricing(input: {
  projectId: string;
  provider: string;
  model: string;
  inputMicrosPerMillion: number;
  cachedInputMicrosPerMillion: number;
  outputMicrosPerMillion: number;
}): Promise<ModelPricing> {
  return invoke<ModelPricing>("upsert_model_pricing", { input });
}

export function deleteModelPricing(projectId: string, provider: string, model: string): Promise<boolean> {
  return invoke<boolean>("delete_model_pricing", { projectId, provider, model });
}

export function usdToMicros(value: number): number {
  return Math.round(Math.max(0, value) * 1_000_000);
}

export function microsToUsd(value: number): number {
  return value / 1_000_000;
}
