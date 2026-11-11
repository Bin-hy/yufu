import { randomUUID } from 'node:crypto'

import {
  expect,
  test,
  type APIRequestContext,
  type APIResponse,
  type BrowserContext,
  type Page,
} from '@playwright/test'

import {
  allocateExampleOrigins,
  awaitConsentIfShown,
  config,
  startGeneratedExamples,
  type GeneratedExamples,
  type SsoConfig,
} from './enterprise-sso-fixture'

interface Account {
  username: string
  password: string
  nickname: string
  kind: 'employee' | 'collaborator'
  developer: boolean
}

interface OnboardedAccount extends Account {
  context: BrowserContext
  page: Page
  userId: number
}

interface Environment {
  environment_id: string
  kind: 'development' | 'production'
  client_id: string
  primary_client_kind: 'server_web' | 'browser_spa'
  active_version_id: string | null
  active_config: { access_audience: string } | null
  revision: number
}

interface Application {
  application_id: number
  app_key: string
  name: string
  revision: number
  owner_user_id: number
  members: { user_id: number; member_role: string }[]
  environments: Environment[]
}

interface Reader {
  reader_id: string
  enabled: boolean
  approved_audience: string
}

async function json<T = Record<string, unknown>>(response: APIResponse, expected = 200): Promise<T> {
  const body = await response.text()
  expect(response.status(), body).toBe(expected)
  return JSON.parse(body) as T
}

async function csrf(
  request: APIRequestContext,
  cfg: SsoConfig,
  path = '/api/v1/identity/csrf',
): Promise<string> {
  const response = await request.get(cfg.opOrigin + path, {
    headers: { origin: cfg.opOrigin },
  })
  return (await json<{ csrf_token: string }>(response)).csrf_token
}

async function write(
  request: APIRequestContext,
  cfg: SsoConfig,
  method: string,
  path: string,
  data: unknown,
  headers: Record<string, string> = {},
): Promise<APIResponse> {
  return request.fetch(cfg.opOrigin + '/api/v1' + path, {
    method,
    headers: {
      origin: cfg.opOrigin,
      'x-csrf-token': await csrf(request, cfg),
      ...headers,
    },
    data,
  })
}

async function centerLogin(
  page: Page,
  cfg: SsoConfig,
  username: string,
  password: string,
): Promise<void> {
  await page.goto(cfg.opOrigin + '/login')
  await page.getByLabel('用户名', { exact: true }).fill(username)
  await page.getByLabel('密码', { exact: true }).fill(password)
  await page.getByRole('button', { name: '登录', exact: true }).click()
  await page.waitForURL(
    (url) => url.origin === cfg.opOrigin && url.pathname === '/account',
    { timeout: 30_000 },
  )
  await expect(page.locator('.nav-group-title').first()).toBeVisible()
}

async function issueInvitation(
  request: APIRequestContext,
  cfg: SsoConfig,
  account: Account,
): Promise<string> {
  const response = await write(request, cfg, 'POST', '/admin/invitations', {
    purpose: 'onboarding',
    person_kind: account.kind,
    developer_grant: account.developer,
    target_user_id: null,
  })
  return (await json<{ secret: string }>(response)).secret
}

async function acceptInvitation(
  context: BrowserContext,
  cfg: SsoConfig,
  account: Account,
  secret: string,
): Promise<number> {
  const exchange = await write(context.request, cfg, 'POST', '/identity/actions/exchange', {
    kind: 'invitation',
    secret,
  })
  const action = await json(exchange)
  const accepted = await write(context.request, cfg, 'POST', '/identity/invitations/accept', {
    action_id: action.action_id,
    existing_account: false,
    username: account.username,
    password: account.password,
    nickname: account.nickname,
  })
  const receipt = await json<{ requires_login: boolean; user_id: number }>(accepted)
  expect(receipt.requires_login).toBe(true)
  return receipt.user_id
}

async function openRegistration(page: Page): Promise<void> {
  await page.getByRole('button', { name: '注册应用', exact: true }).click()
  await expect(page.locator('#app-key')).toBeVisible()
}

async function fillBasic(
  page: Page,
  key: string,
  name: string,
  description: string,
): Promise<void> {
  await page.locator('#app-key').fill(key)
  await page.locator('#app-name').fill(name)
  await page.locator('#app-description').fill(description)
}

async function nextStep(page: Page, title: string): Promise<void> {
  await page.getByRole('button', { name: '下一步', exact: true }).click()
  await expect(page.locator('#registration-step-title')).toHaveText(title)
}

async function fillWebDevelopment(
  page: Page,
  origin: string,
  audience: string,
): Promise<void> {
  await page.selectOption('#registration-dev-kind', 'server_web')
  await page.locator('#registration-dev-redirects').fill(origin + '/callback')
  await page.locator('#registration-dev-logout-redirects').fill(origin + '/')
  await page.locator('#registration-dev-audience').fill(audience)
  await page.selectOption('#registration-dev-scopes', 'openid profile email')
  await page.selectOption('#registration-dev-logout', 'backchannel_and_online')
  await page.locator('#registration-dev-backchannel').fill(origin + '/oidc/backchannel-logout')
}

async function fillSpaDevelopment(
  page: Page,
  origin: string,
  audience: string,
): Promise<void> {
  await page.selectOption('#registration-dev-kind', 'browser_spa')
  await page.locator('#registration-dev-redirects').fill(origin + '/callback')
  await page.locator('#registration-dev-logout-redirects').fill(origin + '/')
  await page.locator('#registration-dev-origins').fill(origin)
  await page.locator('#registration-dev-audience').fill(audience)
  await page.selectOption('#registration-dev-scopes', 'openid profile email')
  await page.selectOption('#registration-dev-logout', 'online')
}

async function fillProductionDraft(
  page: Page,
  kind: 'server_web' | 'browser_spa',
  audience: string,
): Promise<void> {
  await page.selectOption('#registration-prod-kind', kind)
  await page.locator('#registration-prod-audience').fill(audience)
}

async function confirmRegistration(page: Page): Promise<void> {
  await page.getByRole('button', { name: '确认并建立应用', exact: true }).click()
  await expect(page.getByRole('heading', { name: '应用已建立' })).toBeVisible()
}

async function readReceipt(page: Page): Promise<{ appId: number; secret: string | null }> {
  const receipt = page.locator('section').filter({ hasText: '应用已建立' }).first()
  const href = await receipt.getByRole('link', { name: '查看接入配置' }).getAttribute('href')
  expect(href).toMatch(/^\/developer\/apps\/\d+$/)
  const appId = Number(href!.split('/').pop())
  const codes = await receipt.locator('code').allTextContents()
  const secret = codes.length > 1 ? codes[1]!.trim() : null
  return { appId, secret }
}

async function getApp(
  request: APIRequestContext,
  cfg: SsoConfig,
  administrator: boolean,
  id: number,
): Promise<Application> {
  return json<Application>(
    await request.get(
      cfg.opOrigin + `/api/v1/${administrator ? 'admin' : 'developer'}/apps/${id}`,
    ),
  )
}

async function createReader(
  page: Page,
  cfg: SsoConfig,
  appId: number,
  environmentId: string,
): Promise<{ readerId: string; secret: string }> {
  await page.goto(cfg.opOrigin + `/developer/apps/${appId}`)
  await expect(page.locator('#detail-environment')).toBeVisible()
  await page.selectOption('#detail-environment', environmentId)
  const button = page.getByRole('button', { name: '为生效 Audience 创建后端凭据' })
  await expect(button).toBeEnabled()
  await button.click()
  const section = page
    .locator('section')
    .filter({ has: page.getByRole('heading', { name: /^后端内省凭据 / }) })
    .first()
  await expect(section).toBeVisible()
  const heading = (await section.getByRole('heading').textContent())!.trim()
  const readerId = heading.replace('后端内省凭据 ', '').trim()
  const secret = (await section.locator('code').textContent())!.trim()
  await section.getByRole('button', { name: '隐藏密钥' }).click()
  await expect(section).toHaveCount(0)
  return { readerId, secret }
}

async function addTestUser(
  request: APIRequestContext,
  cfg: SsoConfig,
  appId: number,
  userId: number,
): Promise<string> {
  const app = await getApp(request, cfg, true, appId)
  const environment = app.environments.find((candidate) => candidate.kind === 'development')!
  const response = await write(
    request,
    cfg,
    'PATCH',
    `/admin/apps/${appId}/environments/${environment.environment_id}/test-users/${userId}`,
    { present: true, expected_revision: app.revision },
    { 'idempotency-key': randomUUID() },
  )
  await json(response)
  return environment.environment_id
}

test('a developer registers Web A and SPA B, collaborates, transfers ownership and signs in a test user', async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(300_000)
  const cfg = config()
  const suffix = `${testInfo.project.name}-${randomUUID().slice(0, 8)}`
  const password = `Developer-${suffix}-Passw0rd!`
  const developerA: Account = {
    username: `c69-dev-a-${suffix}`,
    password,
    nickname: 'Developer A',
    kind: 'employee',
    developer: true,
  }
  const developerB: Account = {
    username: `c69-dev-b-${suffix}`,
    password,
    nickname: 'Developer B',
    kind: 'employee',
    developer: true,
  }
  const employee: Account = {
    username: `c69-user-${suffix}`,
    password,
    nickname: 'Test User',
    kind: 'employee',
    developer: false,
  }
  const onboarded: OnboardedAccount[] = []
  let examples: GeneratedExamples | undefined
  let userContext: BrowserContext | undefined

  try {
    await centerLogin(page, cfg, cfg.username, cfg.password)
    for (const account of [developerA, developerB, employee]) {
      const secret = await issueInvitation(page.request, cfg, account)
      const context = await browser.newContext({ ignoreHTTPSErrors: true })
      const accountPage = await context.newPage()
      accountPage.setDefaultTimeout(30_000)
      const userId = await acceptInvitation(context, cfg, account, secret)
      onboarded.push({ ...account, context, page: accountPage, userId })
    }

    const a = onboarded.find((account) => account.username === developerA.username)!
    const b = onboarded.find((account) => account.username === developerB.username)!
    const user = onboarded.find((account) => account.username === employee.username)!

    await json(
      await write(page.request, cfg, 'PATCH', '/admin/collaboration-candidates', {
        requester_user_id: a.userId,
        candidate_user_id: b.userId,
        allowed: true,
      }),
    )

    await centerLogin(a.page, cfg, a.username, a.password)
    const origins = await allocateExampleOrigins()
    const webAudience = `urn:c69:web:${suffix}`
    const spaAudience = `urn:c69:spa:${suffix}`

    await a.page.goto(cfg.opOrigin + '/developer/apps')
    await expect(a.page.getByRole('heading', { name: '开发者控制台' })).toBeVisible()
    await openRegistration(a.page)
    await fillBasic(a.page, `c69-web-${suffix}`, 'C69 Web A', 'Server-side web example')
    await nextStep(a.page, '开发环境')
    await fillWebDevelopment(a.page, origins.webOrigin, webAudience)
    await nextStep(a.page, '生产环境')
    await fillProductionDraft(a.page, 'server_web', webAudience + ':production')
    await nextStep(a.page, '确认建立')
    await confirmRegistration(a.page)
    const webReceipt = await readReceipt(a.page)
    expect(webReceipt.secret, 'Web A must receive a one-time client secret').toBeTruthy()
    await expect(a.page.getByText('开发环境密钥（仅此次展示）')).toBeVisible()
    await a.page.getByRole('button', { name: '我已保存，隐藏回执' }).click()
    await expect(a.page.getByRole('heading', { name: '应用已建立' })).toHaveCount(0)

    await a.page.goto(cfg.opOrigin + `/developer/apps/${webReceipt.appId}`)
    await expect(a.page.getByText(/Client ID：/)).toBeVisible()
    const webApp = await getApp(a.context.request, cfg, false, webReceipt.appId)
    const webDev = webApp.environments.find((environment) => environment.kind === 'development')!
    expect(webDev.primary_client_kind).toBe('server_web')
    expect(webDev.active_version_id).toBeTruthy()
    expect(JSON.stringify(webApp)).not.toContain(webReceipt.secret!)
    const webReader = await createReader(a.page, cfg, webReceipt.appId, webDev.environment_id)
    const webReaders = await json<{ items: Reader[] }>(
      await a.context.request.get(
        cfg.opOrigin +
          `/api/v1/developer/apps/${webReceipt.appId}/environments/${webDev.environment_id}/integration/readers`,
      ),
    )
    expect(
      webReaders.items.some(
        (reader) => reader.reader_id === webReader.readerId && reader.enabled,
      ),
    ).toBe(true)

    await a.page.goto(cfg.opOrigin + '/developer/apps')
    await openRegistration(a.page)
    await fillBasic(a.page, `c69-spa-${suffix}`, 'C69 SPA B', 'Browser SPA example')
    await nextStep(a.page, '开发环境')
    await fillSpaDevelopment(a.page, origins.spaOrigin, spaAudience)
    await nextStep(a.page, '生产环境')
    await fillProductionDraft(a.page, 'browser_spa', spaAudience + ':production')
    await nextStep(a.page, '确认建立')
    await confirmRegistration(a.page)
    const spaReceipt = await readReceipt(a.page)
    expect(spaReceipt.secret, 'SPA B must not receive a client secret').toBeNull()
    await expect(a.page.getByText('密钥（仅此次展示）')).toHaveCount(0)
    await a.page.getByRole('button', { name: '我已保存，隐藏回执' }).click()

    await a.page.goto(cfg.opOrigin + `/developer/apps/${spaReceipt.appId}`)
    const spaApp = await getApp(a.context.request, cfg, false, spaReceipt.appId)
    const spaDev = spaApp.environments.find((environment) => environment.kind === 'development')!
    expect(spaDev.primary_client_kind).toBe('browser_spa')
    expect(spaDev.active_version_id).toBeTruthy()
    expect(JSON.stringify(spaApp)).not.toContain('client_secret')
    const spaReader = await createReader(a.page, cfg, spaReceipt.appId, spaDev.environment_id)
    expect(webDev.client_id).not.toBe(spaDev.client_id)
    expect(origins.webOrigin).not.toBe(origins.spaOrigin)

    await a.page.goto(cfg.opOrigin + `/developer/apps/${webReceipt.appId}`)
    const candidate = a.page.locator('#detail-candidate')
    await expect(candidate).toBeVisible()
    await candidate.selectOption(String(b.userId))
    await a.page.getByRole('button', { name: '添加维护者' }).click()
    await expect(a.page.getByText(`账户 ${b.userId} · 维护者`)).toBeVisible()

    await centerLogin(b.page, cfg, b.username, b.password)
    await b.page.goto(cfg.opOrigin + `/developer/apps/${webReceipt.appId}`)
    await expect(b.page.getByRole('heading', { name: 'C69 Web A' })).toBeVisible()
    await b.page.locator('#detail-description').fill('Maintained by Developer B')
    await b.page.getByRole('button', { name: '保存资料' }).click()
    await expect(b.page.locator('#detail-description')).toHaveValue(
      'Maintained by Developer B',
    )
    await b.page.reload()
    await expect(b.page.locator('#detail-description')).toHaveValue(
      'Maintained by Developer B',
    )

    await a.page.reload()
    await expect(a.page.locator('#detail-candidate')).toBeVisible()
    await a.page.locator('#detail-candidate').selectOption(String(b.userId))
    await a.page.getByRole('button', { name: '交接负责人' }).click()
    await expect(a.page.getByText(`负责人账户 ${b.userId}`)).toBeVisible()
    await expect(a.page.getByText(`账户 ${a.userId} · 维护者`)).toBeVisible()
    await expect(a.page.getByRole('button', { name: '交接负责人' })).toHaveCount(0)
    await b.page.reload()
    await expect(b.page.getByText(`负责人账户 ${b.userId}`)).toBeVisible()
    await expect(b.page.getByRole('button', { name: '交接负责人' })).toBeVisible()

    const webEnvironmentId = await addTestUser(
      page.request,
      cfg,
      webReceipt.appId,
      user.userId,
    )
    const spaEnvironmentId = await addTestUser(
      page.request,
      cfg,
      spaReceipt.appId,
      user.userId,
    )
    expect(webEnvironmentId).toBe(webDev.environment_id)
    expect(spaEnvironmentId).toBe(spaDev.environment_id)

    examples = await startGeneratedExamples(
      cfg,
      origins,
      {
        clientId: webDev.client_id,
        clientSecret: webReceipt.secret!,
        readerClientId: webReader.readerId,
        readerClientSecret: webReader.secret,
        sessionSecret: `c69-session-${suffix}-at-least-32-characters`,
      },
      {
        clientId: spaDev.client_id,
        apiAudience: spaAudience,
        readerClientId: spaReader.readerId,
        readerClientSecret: spaReader.secret,
      },
    )

    userContext = await browser.newContext({ ignoreHTTPSErrors: true })
    const webPage = await userContext.newPage()
    webPage.setDefaultTimeout(30_000)
    await webPage.goto(examples.webOrigin + '/')
    await webPage.getByRole('link', { name: /Sign in with Cobotrix/ }).click()
    await webPage.getByLabel('用户名', { exact: true }).fill(user.username)
    await webPage.getByLabel('密码', { exact: true }).fill(user.password)
    await webPage.getByRole('button', { name: '登录', exact: true }).click()
    await awaitConsentIfShown(webPage, cfg)
    await expect(webPage.getByTestId('display-name')).toHaveText(user.username)
    await expect(webPage.getByTestId('local-subject')).toHaveText(String(user.userId))
    expect(await webPage.evaluate(() => sessionStorage.length)).toBe(0)
    const csrfValue = await webPage
      .locator('form')
      .filter({ hasText: 'Run protected operation' })
      .locator('input[name="_csrf"]')
      .inputValue()
    const webOperation = await userContext.request.post(examples.webOrigin + '/api/operation', {
      headers: { origin: examples.webOrigin },
      form: { _csrf: csrfValue },
    })
    expect(webOperation.status(), await webOperation.text()).toBe(200)
    expect(await webOperation.json()).toEqual({ ok: true, operation: 'example-write' })
    const webCookies = await userContext.cookies(examples.webOrigin)
    expect(webCookies.some((cookie) => cookie.name === 'cobotrix_session')).toBe(true)
    expect(await userContext.cookies(examples.spaOrigin)).toEqual([])

    const spaPage = await userContext.newPage()
    spaPage.setDefaultTimeout(30_000)
    await spaPage.goto(examples.spaOrigin + '/')
    await spaPage.getByRole('button', { name: 'Sign in with Cobotrix' }).click()
    await awaitConsentIfShown(spaPage, cfg)
    await expect(spaPage.getByTestId('display-name')).toHaveText(user.username)
    await expect(spaPage.getByTestId('local-subject')).toHaveText(String(user.userId))
    expect(await spaPage.getByLabel('密码', { exact: true }).count()).toBe(0)
    const spaStorageKeys = await spaPage.evaluate(() => Object.keys(sessionStorage))
    expect(spaStorageKeys.length).toBeGreaterThan(0)
    expect(await userContext.cookies(examples.spaOrigin)).not.toContainEqual(
      expect.objectContaining({ name: 'cobotrix_session' }),
    )
    await spaPage.getByRole('button', { name: 'Run protected operation' }).click()
    await expect(spaPage.getByRole('status')).toHaveText('Operation accepted.')

    const evidence = {
      engine: testInfo.project.name,
      applications: {
        web: {
          appKey: `c69-web-${suffix}`,
          clientId: webDev.client_id,
          kind: webDev.primary_client_kind,
          origin: origins.webOrigin,
          secretDeliveredOnce: true,
          secretAbsentAfterReload: true,
          reader: webReader.readerId,
        },
        spa: {
          appKey: `c69-spa-${suffix}`,
          clientId: spaDev.client_id,
          kind: spaDev.primary_client_kind,
          origin: origins.spaOrigin,
          secretDelivered: false,
          reader: spaReader.readerId,
        },
      },
      distinctClients: webDev.client_id !== spaDev.client_id,
      distinctSites: origins.webOrigin !== origins.spaOrigin,
      collaboration: {
        maintainerUserId: b.userId,
        ownerUserIdAfterTransfer: b.userId,
        previousOwnerRole: 'maintainer',
      },
      testUser: {
        userId: user.userId,
        webLogin: true,
        spaLogin: true,
        webProtectedOperation: 200,
        spaProtectedOperation: 'accepted',
      },
      isolation: {
        webSessionCookie: true,
        spaSessionStorage: true,
        webSessionStorageEmpty: true,
        sharedSessionCookie: false,
      },
    }
    testInfo.annotations.push({
      type: 'C69 desensitized evidence',
      description: JSON.stringify(evidence),
    })
    console.log('[C69] ' + JSON.stringify(evidence))
  } finally {
    await userContext?.close()
    await examples?.stop()
    await Promise.all(onboarded.map((account) => account.context.close()))
  }
})
