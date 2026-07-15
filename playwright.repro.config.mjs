import { defineConfig } from "@playwright/test";

const baseURL =
  process.env.MEI_TEST_BASE_URL?.replace(/\/$/, "") || "http://127.0.0.1:3000";

export default defineConfig({
  testDir: "./tests/e2e",
  testMatch: "spa-navigation-repro.spec.mjs",
  timeout: 300000,
  workers: 1,
  use: {
    baseURL,
    headless: process.env.PW_HEADED !== "1",
    channel: process.env.PW_CHANNEL || undefined,
    launchOptions: { slowMo: process.env.PW_SLOWMO ? Number(process.env.PW_SLOWMO) : 0 },
    trace: "retain-on-failure",
    video: "retain-on-failure",
  },
  reporter: [["list"]],
});
