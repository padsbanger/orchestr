import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.{ts,tsx}"],
    coverage: {
      provider: "v8",
      reporter: ["json", "text"],
      reportsDirectory: "coverage",
      include: [
        "src/services/agentReviews.ts",
        "src/services/confirmations.ts",
        "src/services/flow.ts",
        "src/services/integrations.ts",
        "src/services/interruptions.ts",
        "src/services/runs.ts",
        "src/services/workflow.ts",
        "src/components/TaskDetailPanel/TaskDetailPanel.tsx",
        "src/components/WorkflowCockpit/WorkflowCockpit.tsx",
        "src/pages/BoardPage/BoardPage.tsx",
        "src/pages/BoardPage/BoardPageModel.ts",
        "src/pages/BoardPage/BoardPageView.tsx",
      ],
      exclude: ["src/**/*.test.{ts,tsx}"],
    },
  },
});
