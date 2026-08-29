import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  deleteModelPricing,
  getProjectMetrics,
  microsToUsd,
  updateProjectCostControl,
  upsertModelPricing,
  usdToMicros,
} from "./metrics";

describe("metrics service", () => {
  beforeEach(() => invokeMock.mockReset());

  it("loads a bounded project report", async () => {
    invokeMock.mockResolvedValue({ rangeDays: 30 });
    await getProjectMetrics("project-1", 30);
    expect(invokeMock).toHaveBeenCalledWith("get_project_metrics", { projectId: "project-1", rangeDays: 30 });
  });

  it("persists budget and pricing inputs at the command boundary", async () => {
    invokeMock.mockResolvedValue({});
    const control = { projectId: "project-1", monthlyBudgetMicros: 25_000_000, warningThresholdPercent: 80, blockNewRuns: true };
    await updateProjectCostControl(control);
    expect(invokeMock).toHaveBeenCalledWith("update_project_cost_control", { input: control });

    const pricing = { projectId: "project-1", provider: "codex", model: "gpt-test", inputMicrosPerMillion: 1_000_000, cachedInputMicrosPerMillion: 500_000, outputMicrosPerMillion: 2_000_000 };
    await upsertModelPricing(pricing);
    expect(invokeMock).toHaveBeenCalledWith("upsert_model_pricing", { input: pricing });
    await deleteModelPricing("project-1", "codex", "gpt-test");
    expect(invokeMock).toHaveBeenCalledWith("delete_model_pricing", { projectId: "project-1", provider: "codex", model: "gpt-test" });
  });

  it("converts dollar inputs to integer microdollars without negative values", () => {
    expect(usdToMicros(12.345678)).toBe(12_345_678);
    expect(usdToMicros(-4)).toBe(0);
    expect(microsToUsd(12_345_678)).toBe(12.345678);
  });
});
