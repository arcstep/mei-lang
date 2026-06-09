import { defineConfig } from "@playwright/test";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.dirname(fileURLToPath(import.meta.url));
const e2ePort = process.env.MEI_E2E_PORT || "3010";
const baseURL =
  process.env.MEI_TEST_BASE_URL?.replace(/\/$/, "") ||
  `http://127.0.0.1:${e2ePort}`;

const skipWebServer =
  process.env.MEI_TEST_SKIP_SERVER === "1" || !!process.env.MEI_TEST_BASE_URL;

export default defineConfig({
  testDir: "./e2e",
  testMatch: "**/*.spec.mjs",
  timeout: 90000,
  expect: { timeout: 20000 },
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  use: {
    baseURL,
    headless: true,
    trace: "on-first-retry",
  },
  reporter: [["list"]],
  ...(skipWebServer
    ? {}
    : {
        webServer: {
          command: `cargo run -p mei-lang-server --bin mei-host-web -- serve --port ${e2ePort}`,
          url: baseURL,
          cwd: rootDir,
          reuseExistingServer: !process.env.CI,
          timeout: 180_000,
        },
      }),
});
