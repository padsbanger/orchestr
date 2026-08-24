import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
    coverage: {
      provider: "v8",
      reporter: ["json", "text"],
      reportsDirectory: "coverage",
      include: ["src/services/agentReviews.ts"],
      exclude: ["src/**/*.test.ts"],
    },
  },
});
