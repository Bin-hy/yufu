import { defineConfig } from '@playwright/test'
import { release } from 'node:os'

// macOS 14 resolves to a WebKit build whose protocol predates settings sent by
// this Playwright driver. enterprise-sso-webkit.mjs covers that engine with the
// compatible 1.61 driver instead.
export const externalWebkit =
  process.platform === 'darwin' && Number(release().split('.')[0]) < 24

export default defineConfig({
  testDir: './tests/e2e',
  testMatch: /enterprise-sso-.*\.spec\.ts/,
  fullyParallel: false,
  workers: 1,
  timeout: 90_000,
  expect: { timeout: 20_000 },
  reporter: [['list']],
  globalSetup: './tests/e2e/enterprise-sso-fixture.ts',
  use: {
    ignoreHTTPSErrors: true,
    trace: 'retain-on-failure',
  },
  projects: [
    // Device presets push WebKit-only settings this Playwright build does not
    // know, so each engine runs with the plain context defaults.
    { name: 'chromium', use: { browserName: 'chromium' } },
    { name: 'firefox', use: { browserName: 'firefox' } },
    ...(externalWebkit ? [] : [{ name: 'webkit', use: { browserName: 'webkit' as const } }]),
  ],
})
