import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  outputDir: "Temp/verification/browser-results",
  reporter: [
    ["list"],
    ["json", { outputFile: "Temp/verification/browser-results.json" }],
  ],
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:1420",
    channel: "msedge",
    trace: "retain-on-failure",
    launchOptions: { args: ["--disk-cache-dir=" + process.env.TEMP] },
  },
  webServer: {
    command: "node scripts/deck.mjs web",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: false,
    timeout: 30000,
  },
});
