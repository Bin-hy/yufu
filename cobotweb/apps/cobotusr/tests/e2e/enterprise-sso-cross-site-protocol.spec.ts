import { test } from '@playwright/test'

import { runCrossSiteProtocolScenario } from './enterprise-sso-cross-site-protocol'

test('cross-site SSO keeps identity stable and application sessions independent', async (
  { browser },
  testInfo,
) => {
  test.setTimeout(180_000)
  await runCrossSiteProtocolScenario(browser, testInfo.project.name)
})
