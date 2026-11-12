import { test } from '@playwright/test'

import { runProductionGovernanceScenario } from './enterprise-sso-production-governance'

test('production governance reviews, assigns and activates two applications', async (
  { browser },
  testInfo,
) => {
  test.setTimeout(300_000)
  await runProductionGovernanceScenario(browser, testInfo.project.name)
})
