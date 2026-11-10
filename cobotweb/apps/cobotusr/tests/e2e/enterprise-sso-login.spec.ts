import { expect, test } from '@playwright/test'

import { awaitConsentIfShown, config, loginAtCenter } from './enterprise-sso-fixture'

test('A login then B SSO in a new tab keeps identity and isolates storage', async ({
  page,
  context,
}) => {
  const cfg = config()

  await page.goto(cfg.aOrigin + '/')
  await page.getByRole('link', { name: /Sign in with Cobotrix/ }).click()
  await loginAtCenter(page, cfg)
  await expect(page.getByTestId('display-name')).toHaveText(cfg.username)

  const aCookies = await context.cookies(cfg.aOrigin)
  expect(aCookies.some((cookie) => cookie.name === 'cobotrix_session')).toBe(true)
  expect(await context.cookies(cfg.bOrigin)).toEqual([])

  const b = await context.newPage()
  await b.goto(cfg.bOrigin + '/')
  await b.getByRole('button', { name: 'Sign in with Cobotrix' }).click()
  await awaitConsentIfShown(b, cfg)

  await expect(b.getByTestId('display-name')).toHaveText(cfg.username)
  await expect(b.getByTestId('local-subject')).toHaveText(cfg.subject)
  await expect(b.getByLabel('密码', { exact: true })).toHaveCount(0)

  await b.getByRole('button', { name: 'Run protected operation' }).click()
  await expect(b.getByRole('status')).toHaveText('Operation accepted.')

  await page.reload()
  await expect(page.getByTestId('display-name')).toHaveText(cfg.username)
  await b.reload()
  await expect(b.getByTestId('display-name')).toHaveText(cfg.username)
})
