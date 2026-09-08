import { defineConfig } from "vitest/config";
export default defineConfig({
  server: { port: 1420, strictPort: true },
  clearScreen: false,
  test: { environment: "jsdom", include: ["src/**/*.test.ts"] },
});
