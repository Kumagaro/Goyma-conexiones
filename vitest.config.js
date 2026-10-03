import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    include: ["src/js/**/__tests__/**/*.test.js"],
    pool: "threads",
    poolOptions: {
      threads: {
        singleThread: true,
      },
    },
    fileParallelism: false,
    isolate: false,
    maxWorkers: 1,
    minWorkers: 1,
  },
});
