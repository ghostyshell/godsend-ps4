import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "tests",
  timeout: 30000,
  use: {
    baseURL: "http://localhost:1436",
  },
  webServer: {
    command: "npm run dev",
    port: 1436,
    reuseExistingServer: false,
    timeout: 60000,
  },
});
