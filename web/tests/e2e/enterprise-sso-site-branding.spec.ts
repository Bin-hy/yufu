import { test } from '@playwright/test'
import { runSiteBrandingScenario } from './enterprise-sso-site-branding'
test('site-branding settings publish, inherit OIDC appearance and preserve conflicting drafts', async ({ browser }, info) => {
  test.setTimeout(180_000)
  await runSiteBrandingScenario(browser, info.project.name)
})
