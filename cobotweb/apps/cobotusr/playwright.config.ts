import { defineConfig, devices } from '@playwright/test'
export default defineConfig({
    testDir: './tests/e2e',
    fullyParallel: false,
    timeout: 30000,
    expect: { timeout: 7000 },
    reporter: [
        ['list'],
        ['html', { open: 'never', outputFolder: '/tmp/cobotusr-playwright-report' }],
        [
            'json',
            {
                outputFile:
                    '../../../docs/specs/cobotusr-frontend-redesign/evidence/after/e2e-results.json',
            },
        ],
    ],
    use: { baseURL: 'http://127.0.0.1:5188', trace: 'retain-on-failure' },
    webServer: {
        command: 'node tests/e2e/serve.mjs',
        timeout: 120000,
        url: 'http://127.0.0.1:5188',
        reuseExistingServer: false,
    },
    projects: [
        { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
        { name: 'firefox', use: { ...devices['Desktop Firefox'] } },
        { name: 'webkit', use: { ...devices['Desktop Safari'] } },
    ],
})
