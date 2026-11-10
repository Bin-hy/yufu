import { expect, test } from '@playwright/test'

import { awaitConsentIfShown, config, loginAtCenter, signToken } from './enterprise-sso-fixture'

test('local logout keeps B while central logout ends the center session', async ({
  page,
  context,
}) => {
  const cfg = config()

  await page.goto(cfg.aOrigin + '/')
  await page.getByRole('link', { name: /Sign in with Cobotrix/ }).click()
  await loginAtCenter(page, cfg)
  await expect(page.getByTestId('display-name')).toHaveText(cfg.username)

  await page.getByRole('button', { name: 'Sign out of this app' }).click()
  await expect(page.getByRole('link', { name: /Sign in with Cobotrix/ })).toBeVisible()

  const b = await context.newPage()
  await b.goto(cfg.bOrigin + '/')
  await b.getByRole('button', { name: 'Sign in with Cobotrix' }).click()
  await awaitConsentIfShown(b, cfg)
  await expect(b.getByTestId('display-name')).toHaveText(cfg.username)

  await b.getByRole('button', { name: 'Sign out of the identity center' }).click()
  // The center never ends a browser-wide session on a bare GET.
  const confirm = b.getByRole('button', { name: '确认退出' })
  await confirm.waitFor({ state: 'visible', timeout: 20_000 })
  await confirm.click()
  // A string pattern is a glob, and "[::1]" would be read as a character class.
  await b.waitForURL((url) => url.href === cfg.bOrigin + '/')

  await page.goto(cfg.aOrigin + '/login')
  await expect(page.getByLabel('密码', { exact: true })).toBeVisible()
})

test('back-channel logout clears the web session', async ({ page }) => {
  const cfg = config()

  await page.goto(cfg.aOrigin + '/')
  await page.getByRole('link', { name: /Sign in with Cobotrix/ }).click()
  await loginAtCenter(page, cfg)
  expect((await page.request.get(cfg.aOrigin + '/api/me')).status()).toBe(200)

  const now = Math.floor(Date.now() / 1000)
  const token = signToken(
    {
      iss: cfg.opOrigin,
      aud: cfg.webClientId,
      sub: cfg.subject,
      iat: now,
      exp: now + 120,
      jti: 'logout-' + now,
      events: { 'http://schemas.openid.net/event/backchannel-logout': {} },
    },
    'logout+jwt',
  )
  const response = await page.request.post(cfg.aOrigin + '/oidc/backchannel-logout', {
    form: { logout_token: token },
  })

  expect(response.status()).toBe(200)
  expect((await page.request.get(cfg.aOrigin + '/api/me')).status()).toBe(401)
})
