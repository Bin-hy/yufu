import assert from 'node:assert/strict'

import type { Browser, BrowserContext, Page } from '@playwright/test'

import {
  awaitConsentIfShown,
  config,
  loginAtCenter,
  type SsoConfig,
} from './enterprise-sso-fixture.ts'

interface ProtocolEvidence {
  issuer: string
  subject: string
  clientId: string
  audience: string
  sid: string
  authTime: number
  stateVerified: string
  nonceVerified: string
  accessTokenFingerprint: string
}

async function waitForText(
  page: Page,
  testId: string,
  timeout = 20_000,
): Promise<string> {
  const locator = page.getByTestId(testId)
  const deadline = Date.now() + timeout
  let observed = ''
  while (Date.now() < deadline) {
    observed = ((await locator.textContent().catch(() => null)) ?? '').trim()
    if (observed !== '' && observed !== '—') return observed
    await page.waitForTimeout(100)
  }
  throw new Error(`expected ${testId} text; observed ${JSON.stringify(observed)}`)
}

async function readEvidence(page: Page): Promise<ProtocolEvidence> {
  const authTimeText = await waitForText(page, 'auth-time')
  const authTime = Number(authTimeText)
  assert.ok(Number.isSafeInteger(authTime) && authTime > 0, `invalid auth_time: ${authTimeText}`)

  return {
    issuer: await waitForText(page, 'issuer'),
    subject: await waitForText(page, 'local-subject'),
    clientId: await waitForText(page, 'client-id'),
    audience: await waitForText(page, 'audience'),
    sid: await waitForText(page, 'sid'),
    authTime,
    stateVerified: await waitForText(page, 'state-validated'),
    nonceVerified: await waitForText(page, 'nonce-validated'),
    accessTokenFingerprint: await waitForText(page, 'access-token-fingerprint'),
  }
}

async function waitForSidChange(
  page: Page,
  previous: string,
  timeout = 20_000,
): Promise<string> {
  const locator = page.getByTestId('sid')
  const deadline = Date.now() + timeout
  let observed = ''
  while (Date.now() < deadline) {
    observed = ((await locator.textContent().catch(() => null)) ?? '').trim()
    if (observed !== '' && observed !== '—' && observed !== previous) return observed
    await page.waitForTimeout(100)
  }
  throw new Error(
    `sid did not change from ${previous}; observed ${JSON.stringify(observed)}`,
  )
}

function assertEvidence(a: ProtocolEvidence, b: ProtocolEvidence, cfg: SsoConfig): void {
  assert.equal(a.issuer, cfg.opOrigin)
  assert.equal(b.issuer, cfg.opOrigin)
  assert.equal(a.subject, cfg.subject)
  assert.equal(b.subject, cfg.subject)
  assert.equal(a.clientId, cfg.webClientId)
  assert.equal(b.clientId, cfg.spaClientId)
  assert.equal(a.audience, cfg.webClientId)
  assert.equal(b.audience, cfg.spaClientId)
  assert.notEqual(a.sid, b.sid)
  assert.notEqual(a.accessTokenFingerprint, b.accessTokenFingerprint)
  assert.match(a.accessTokenFingerprint, /^sha256:[0-9a-f]{16}$/)
  assert.match(b.accessTokenFingerprint, /^sha256:[0-9a-f]{16}$/)
  assert.equal(a.stateVerified, 'true')
  assert.equal(b.stateVerified, 'true')
  assert.equal(a.nonceVerified, 'true')
  assert.equal(b.nonceVerified, 'true')
}

async function beginWebLogin(page: Page, cfg: SsoConfig): Promise<void> {
  await page.goto(cfg.aOrigin + '/')
  await page.getByRole('link', { name: /Sign in with Yufu/ }).click()
  await page.getByLabel('用户名', { exact: true }).waitFor({ state: 'visible' })
}

async function beginSpaLogin(page: Page, cfg: SsoConfig): Promise<void> {
  await page.goto(cfg.bOrigin + '/')
  await page.getByRole('button', { name: 'Sign in with Yufu' }).click()
  await page.getByLabel('用户名', { exact: true }).waitFor({ state: 'visible' })
}

async function updateCenterProfile(context: BrowserContext, cfg: SsoConfig): Promise<void> {
  const csrfResponse = await context.request.get(
    cfg.opOrigin + '/api/v1/identity/csrf',
    { headers: { origin: cfg.opOrigin } },
  )
  assert.equal(csrfResponse.status(), 200, await csrfResponse.text())
  const csrfToken = ((await csrfResponse.json()) as { csrf_token: string }).csrf_token
  const response = await context.request.patch(
    cfg.opOrigin + '/api/v1/account/profile',
    {
      headers: {
        origin: cfg.opOrigin,
        'x-csrf-token': csrfToken,
      },
      data: { nickname: 'Protocol Profile' },
    },
  )
  assert.equal(response.status(), 200, await response.text())
}

export async function runCrossSiteProtocolScenario(
  browser: Browser,
  engine: string,
): Promise<void> {
  const cfg = config()
  const context = await browser.newContext({ ignoreHTTPSErrors: true })

  try {
    const a = await context.newPage()
    const b = await context.newPage()
    a.setDefaultTimeout(30_000)
    b.setDefaultTimeout(30_000)

    // Start both first-time authorization flows before either has a center login.
    await beginWebLogin(a, cfg)
    await beginSpaLogin(b, cfg)
    await loginAtCenter(a, cfg)
    await b.reload()
    await awaitConsentIfShown(b, cfg)

    const aEvidence = await readEvidence(a)
    const bEvidence = await readEvidence(b)
    assertEvidence(aEvidence, bEvidence, cfg)

    const centerCookies = await context.cookies(cfg.opOrigin)
    assert.ok(centerCookies.some((cookie) => cookie.name.includes('sso')))
    assert.ok(
      (await context.cookies(cfg.aOrigin)).some(
        (cookie) => cookie.name === 'yufu_session',
      ),
    )
    assert.ok(
      !(await context.cookies(cfg.bOrigin)).some(
        (cookie) => cookie.name === 'yufu_session',
      ),
    )
    assert.equal(await a.evaluate(() => globalThis.sessionStorage.length), 0)
    assert.ok((await b.evaluate(() => globalThis.sessionStorage.length)) > 0)

    await a.reload()
    assert.equal(await waitForText(a, 'display-name'), cfg.username)
    await b.reload()
    assert.equal(await waitForText(b, 'display-name'), cfg.username)

    // A new tab has no SPA session storage, but the center cookie still supplies SSO.
    await b.close()
    const reopened = await context.newPage()
    reopened.setDefaultTimeout(30_000)
    await reopened.goto(cfg.bOrigin + '/')
    await reopened.getByRole('button', { name: 'Sign in with Yufu' }).click()
    await awaitConsentIfShown(reopened, cfg)
    assert.equal(await reopened.getByLabel('密码', { exact: true }).count(), 0)
    const reopenedEvidence = await readEvidence(reopened)
    assertEvidence(aEvidence, reopenedEvidence, cfg)

    // Losing only this tab's token must not lose the enterprise session.
    await reopened.evaluate(() => globalThis.sessionStorage.clear())
    await reopened.reload()
    await reopened.getByRole('button', { name: 'Sign in with Yufu' }).click()
    await awaitConsentIfShown(reopened, cfg)
    assert.equal(await reopened.getByLabel('密码', { exact: true }).count(), 0)
    const recoveredEvidence = await readEvidence(reopened)
    assertEvidence(aEvidence, recoveredEvidence, cfg)

    // prompt=none is a protocol result, not a login form.
    const anonymous = await browser.newContext({ ignoreHTTPSErrors: true })
    try {
      const nonePage = await anonymous.newPage()
      nonePage.setDefaultTimeout(30_000)
      await nonePage.goto(cfg.aOrigin + '/login?prompt=none')
      await nonePage.getByRole('heading', { name: 'Sign-in failed' }).waitFor()
      const message = await nonePage.locator('body').innerText()
      assert.ok(message.includes('login_required'), message)
    } finally {
      await anonymous.close()
    }

    await a.goto(cfg.aOrigin + '/login?prompt=none')
    assert.equal(await waitForText(a, 'display-name'), cfg.username)
    assert.equal(await a.getByLabel('密码', { exact: true }).count(), 0)

    await a.goto(cfg.aOrigin + '/login?prompt=consent')
    await awaitConsentIfShown(a, cfg)
    const consentEvidence = await readEvidence(a)
    assert.equal(consentEvidence.subject, cfg.subject)

    const beforePromptLogin = await readEvidence(a)
    await a.goto(cfg.aOrigin + '/login?prompt=login')
    await loginAtCenter(a, cfg)
    await waitForSidChange(a, beforePromptLogin.sid)
    const afterPromptLogin = await readEvidence(a)
    assert.notEqual(afterPromptLogin.sid, beforePromptLogin.sid)
    assert.notEqual(
      afterPromptLogin.accessTokenFingerprint,
      beforePromptLogin.accessTokenFingerprint,
    )

    const beforeMaxAge = afterPromptLogin
    await a.goto(cfg.aOrigin + '/login?max_age=0')
    await loginAtCenter(a, cfg)
    await waitForSidChange(a, beforeMaxAge.sid)
    const afterMaxAge = await readEvidence(a)
    assert.notEqual(afterMaxAge.sid, beforeMaxAge.sid)
    assert.notEqual(
      afterMaxAge.accessTokenFingerprint,
      beforeMaxAge.accessTokenFingerprint,
    )

    await updateCenterProfile(context, cfg)
    await a.goto(cfg.aOrigin + '/login?prompt=consent')
    await awaitConsentIfShown(a, cfg)
    const afterProfileChange = await readEvidence(a)
    assert.equal(afterProfileChange.issuer, cfg.opOrigin)
    assert.equal(afterProfileChange.subject, cfg.subject)

    console.log(
      `E4 cross-site protocol passed: ${JSON.stringify({
        engine,
        sharedIdentity: { issuer: cfg.opOrigin, subject: cfg.subject },
        distinctClients: [cfg.webClientId, cfg.spaClientId],
        distinctSid: aEvidence.sid !== recoveredEvidence.sid,
        distinctAccessTokens:
          aEvidence.accessTokenFingerprint !== recoveredEvidence.accessTokenFingerprint,
        prompt: { none: 'login_required-when-anonymous', login: 'reauthenticated', consent: 'reshown' },
        maxAgeZero: 'reauthenticated',
        profileChangeKeptSubject: true,
      })}`,
    )
  } finally {
    await context.close()
  }
}
