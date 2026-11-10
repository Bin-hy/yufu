// WebKit fallback for hosts where the Playwright 1.65 driver and the available
// WebKit build disagree on protocol settings. Mirrors the three Playwright
// specs with the 1.61 driver already used by real-auth.mjs.
import assert from 'node:assert/strict'
import { log } from 'node:console'
import { setTimeout as delay } from 'node:timers/promises'

import { webkit } from 'playwright-webkit'

import setup, {
  awaitConsentIfShown,
  config,
  loginAtCenter,
  signToken,
} from './enterprise-sso-fixture.ts'

const cfg = config()

async function waitForText(locator, expected, timeout = 20_000) {
  const deadline = Date.now() + timeout
  let observed = ''
  while (Date.now() < deadline) {
    observed = ((await locator.textContent().catch(() => null)) ?? '').trim()
    if (observed === expected) return
    await delay(100)
  }
  throw new Error(`expected text ${JSON.stringify(expected)}, observed ${JSON.stringify(observed)}`)
}

async function openContext(browser) {
  const context = await browser.newContext({ ignoreHTTPSErrors: true })
  return context
}

async function assertAdmission(browser) {
  const context = await openContext(browser)
  try {
    const now = Math.floor(Date.now() / 1000)
    const claims = {
      iss: cfg.opOrigin,
      sub: cfg.subject,
      iat: now,
      exp: now + 300,
      session_id: 'fixture-credential-session',
      application: cfg.spaClientId,
    }
    const foreign = signToken({ ...claims, aud: 'urn:app:another-application' }, 'at+jwt')
    const foreignResponse = await context.request.post(cfg.bOrigin + '/api/operation', {
      headers: { authorization: 'Bearer ' + foreign },
    })
    assert.equal(foreignResponse.status(), 401)
    assert.deepEqual(await foreignResponse.json(), { error: 'invalid_token' })

    const idToken = signToken({ ...claims, aud: cfg.spaClientId }, 'JWT')
    const replayResponse = await context.request.post(cfg.bOrigin + '/api/operation', {
      headers: { authorization: 'Bearer ' + idToken },
    })
    assert.equal(replayResponse.status(), 401)
    assert.deepEqual(await replayResponse.json(), { error: 'invalid_token' })

    const originResponse = await context.request.post(cfg.bOrigin + '/api/operation', {
      headers: { origin: 'https://evil.example' },
    })
    assert.equal(originResponse.status(), 403)
    assert.deepEqual(await originResponse.json(), { error: 'origin_rejected' })
    log('WebKit: protected API admission checks passed')
  } finally {
    await context.close()
  }
}

async function signInThroughWeb(page) {
  await page.goto(cfg.aOrigin + '/')
  await page.getByRole('link', { name: /Sign in with Cobotrix/ }).click()
  await loginAtCenter(page, cfg)
  await waitForText(page.getByTestId('display-name'), cfg.username)
}

async function assertLoginAndSso(browser) {
  const context = await openContext(browser)
  try {
    const page = await context.newPage()
    page.setDefaultTimeout(30_000)
    await signInThroughWeb(page)

    const aCookies = await context.cookies(cfg.aOrigin)
    assert.ok(aCookies.some((cookie) => cookie.name === 'cobotrix_session'))
    assert.deepEqual(await context.cookies(cfg.bOrigin), [])

    const b = await context.newPage()
    b.setDefaultTimeout(30_000)
    await b.goto(cfg.bOrigin + '/')
    await b.getByRole('button', { name: 'Sign in with Cobotrix' }).click()
    await awaitConsentIfShown(b, cfg)
    await waitForText(b.getByTestId('display-name'), cfg.username)
    await waitForText(b.getByTestId('local-subject'), cfg.subject)
    assert.equal(await b.getByLabel('密码', { exact: true }).count(), 0)

    await b.getByRole('button', { name: 'Run protected operation' }).click()
    await waitForText(b.getByRole('status'), 'Operation accepted.')

    await page.reload()
    await waitForText(page.getByTestId('display-name'), cfg.username)
    await b.reload()
    await waitForText(b.getByTestId('display-name'), cfg.username)
    log('WebKit: web login, cross-client SSO and protected operation passed')
  } finally {
    await context.close()
  }
}

async function assertLocalAndCentralLogout(browser) {
  const context = await openContext(browser)
  try {
    const page = await context.newPage()
    page.setDefaultTimeout(30_000)
    await signInThroughWeb(page)

    await page.getByRole('button', { name: 'Sign out of this app' }).click()
    await page.getByRole('link', { name: /Sign in with Cobotrix/ }).waitFor()

    const b = await context.newPage()
    b.setDefaultTimeout(30_000)
    await b.goto(cfg.bOrigin + '/')
    await b.getByRole('button', { name: 'Sign in with Cobotrix' }).click()
    await awaitConsentIfShown(b, cfg)
    await waitForText(b.getByTestId('display-name'), cfg.username)

    await b.getByRole('button', { name: 'Sign out of the identity center' }).click()
    const confirm = b.getByRole('button', { name: '确认退出' })
    await confirm.waitFor({ state: 'visible' })
    await confirm.click()
    await b.waitForURL((url) => url.href === cfg.bOrigin + '/')

    await page.goto(cfg.aOrigin + '/login')
    await page.getByLabel('密码', { exact: true }).waitFor()
    log('WebKit: local logout and central logout confirmation passed')
  } finally {
    await context.close()
  }
}

async function assertBackchannelLogout(browser) {
  const context = await openContext(browser)
  try {
    const page = await context.newPage()
    page.setDefaultTimeout(30_000)
    await signInThroughWeb(page)
    assert.equal((await context.request.get(cfg.aOrigin + '/api/me')).status(), 200)

    const now = Math.floor(Date.now() / 1000)
    const token = signToken(
      {
        iss: cfg.opOrigin,
        aud: cfg.webClientId,
        sub: cfg.subject,
        iat: now,
        exp: now + 120,
        jti: 'webkit-logout-' + now,
        events: { 'http://schemas.openid.net/event/backchannel-logout': {} },
      },
      'logout+jwt',
    )
    const response = await context.request.post(cfg.aOrigin + '/oidc/backchannel-logout', {
      form: { logout_token: token },
    })
    assert.equal(response.status(), 200)
    assert.equal((await context.request.get(cfg.aOrigin + '/api/me')).status(), 401)
    log('WebKit: back-channel logout passed')
  } finally {
    await context.close()
  }
}

const teardown = await setup()
let browser
try {
  browser = await webkit.launch({ headless: true })
  await assertAdmission(browser)
  await assertLoginAndSso(browser)
  await assertLocalAndCentralLogout(browser)
  await assertBackchannelLogout(browser)
  log('WebKit (1.61 driver): enterprise SSO scenarios passed')
} finally {
  await browser?.close()
  await teardown()
}
