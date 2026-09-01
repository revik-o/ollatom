import { defineConfig, devices } from '@playwright/test';

const DEVELOPMENT_SERVER_URL = 'http://127.0.0.1:4200';
const isContinuousIntegration = Boolean(process.env['CI']);
const angularCliEntryPoint = './node_modules/@angular/cli/bin/ng.js';

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: isContinuousIntegration,
  retries: isContinuousIntegration ? 2 : 0,
  workers: isContinuousIntegration ? 1 : undefined,
  reporter: isContinuousIntegration ? 'github' : 'list',
  outputDir: '../test-results',
  use: {
    baseURL: DEVELOPMENT_SERVER_URL,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: `"${process.execPath}" ${angularCliEntryPoint} serve --host 127.0.0.1 --port 4200`,
    cwd: process.cwd(),
    url: DEVELOPMENT_SERVER_URL,
    reuseExistingServer: !isContinuousIntegration,
    timeout: 120_000,
  },
});
