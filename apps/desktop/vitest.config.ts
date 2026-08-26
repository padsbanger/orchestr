import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
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
      ],
      exclude: ["src/**/*.test.ts"],
    },
  },
});
