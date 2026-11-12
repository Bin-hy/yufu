import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'

import type {
  APIRequestContext,
  APIResponse,
  Browser,
  BrowserContext,
  Locator,
  Page,
  Response as BrowserResponse,
} from '@playwright/test'

import {
  allocateExampleOrigins,
  config,
  startGeneratedExamples,
  type GeneratedExamples,
  type SsoConfig,
} from './enterprise-sso-fixture.ts'

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

interface EnvironmentConfig {
  access_audience: string
  consent_policy: 'user_consent' | 'enterprise_preconsent'
  logout_capability: 'backchannel_and_online' | 'online' | 'local_only'
}

interface Environment {
  environment_id: string
  kind: 'development' | 'production'
  client_id: string
  revision: number
  active_config: EnvironmentConfig | null
  latest: { version_id: string; config: EnvironmentConfig }
  pending_approval: string | null
}

interface Application {
  application_id: number
  third_party: boolean
  revision: number
  environments: Environment[]
}

interface RegistrationResponse {
  application_id: number
  environments: { environment_id: string; kind: string; client_id: string }[]
  secrets?: { environment_id: string; client_secret: string }[]
}

interface ChangeResponse {
  summary: { approval_id?: string; revision?: number; version_id?: string }
}

interface Approval {
  approval_id: string
  status: string
  version_id: string
  logout_limitation_accepted: boolean
}

async function json<T = Record<string, unknown>>(
  response: APIResponse | BrowserResponse,
  expected = 200,
): Promise<T> {
  const body = await response.text()
  assert.equal(response.status(), expected, body)
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
  await page.locator('.nav-group-title').first().waitFor({ state: 'visible' })
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
  const action = await json<{ action_id: string }>(exchange)
  const accepted = await write(context.request, cfg, 'POST', '/identity/invitations/accept', {
    action_id: action.action_id,
    existing_account: false,
    username: account.username,
    password: account.password,
    nickname: account.nickname,
  })
  return (await json<{ user_id: number }>(accepted)).user_id
}

async function grantEnterpriseAdministrator(
  request: APIRequestContext,
  cfg: SsoConfig,
  rootPassword: string,
  userId: number,
): Promise<number> {
  const login = await request.post(cfg.opOrigin + '/api/v1/login', {
    headers: {
      origin: cfg.opOrigin,
      'x-csrf-token': await csrf(request, cfg, '/api/v1/csrf'),
    },
    data: { username: cfg.username, password: rootPassword },
  })
  const session = await json<{ access_token: string; csrf_token: string }>(login)
  const listed = await request.get(
    cfg.opOrigin + '/api/v1/authority?page_number=1&page_size=500',
    { headers: { authorization: 'Bearer ' + session.access_token } },
  )
  const authorities = (await json<{ data_list: { id: number; name: string; authority_uri: string }[] }>(listed))
    .data_list.filter(
      (authority) =>
        String(authority.name).startsWith('identity.uri:') &&
        String(authority.authority_uri).startsWith('/admin/'),
    )
  assert.ok(authorities.length > 0, 'identity administrator authorities must exist')
  for (const authority of authorities) {
    await json(
      await request.post(cfg.opOrigin + '/api/v1/user_authority_override', {
        headers: {
          authorization: 'Bearer ' + session.access_token,
          origin: cfg.opOrigin,
          'x-csrf-token': session.csrf_token,
        },
        data: { user_id: userId, authority_id: authority.id, allow: true },
      }),
    )
  }
  return authorities.length
}

async function createGroup(
  request: APIRequestContext,
  cfg: SsoConfig,
  name: string,
): Promise<{ groupId: string; revision: number }> {
  const response = await write(
    request,
    cfg,
    'POST',
    '/admin/groups',
    { name, description: 'C70 production governance fixture' },
    { 'idempotency-key': randomUUID() },
  )
  const receipt = await json<{ summary: { group_id: string; revision: number } }>(response)
  return { groupId: receipt.summary.group_id, revision: receipt.summary.revision }
}

async function addGroupMember(
  request: APIRequestContext,
  cfg: SsoConfig,
  groupId: string,
  userId: number,
  revision: number,
): Promise<number> {
  const response = await write(
    request,
    cfg,
    'PATCH',
    `/admin/groups/${groupId}/members/${userId}`,
    { present: true, expected_revision: revision },
    { 'idempotency-key': randomUUID() },
  )
  return (await json<{ summary: { revision: number } }>(response)).summary.revision
}

async function waitForTextContaining(
  locator: Locator,
  expected: string,
  timeout = 20_000,
): Promise<void> {
  const deadline = Date.now() + timeout
  let observed = ''
  while (Date.now() < deadline) {
    observed = ((await locator.textContent().catch(() => null)) ?? '').trim()
    if (observed.includes(expected)) return
    await locator.page().waitForTimeout(100)
  }
  throw new Error(`expected text containing ${JSON.stringify(expected)}, observed ${JSON.stringify(observed)}`)
}

async function waitForEnabled(locator: { isEnabled(): Promise<boolean> }, timeout = 20_000): Promise<void> {
  const deadline = Date.now() + timeout
  while (Date.now() < deadline) {
    if (await locator.isEnabled()) return
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error('expected control to become enabled')
}

async function getApp(
  request: APIRequestContext,
  cfg: SsoConfig,
  administrator: boolean,
  id: number,
): Promise<Application> {
  return json<Application>(
    await request.get(cfg.opOrigin + `/api/v1/${administrator ? 'admin' : 'developer'}/apps/${id}`),
  )
}

async function openRegistration(page: Page): Promise<void> {
  await page.getByRole('button', { name: '注册应用', exact: true }).click()
  await page.locator('#app-key').waitFor({ state: 'visible' })
}

async function nextStep(page: Page, title: string): Promise<void> {
  await page.getByRole('button', { name: '下一步', exact: true }).click()
  await waitForTextContaining(page.locator('#registration-step-title'), title)
}

async function fillProduction(
  page: Page,
  kind: 'server_web' | 'browser_spa',
  origin: string,
  audience: string,
): Promise<void> {
  await page.selectOption('#registration-prod-kind', kind)
  await page.locator('#registration-prod-redirects').fill(origin + '/callback')
  await page.locator('#registration-prod-logout-redirects').fill(origin + '/')
  if (kind === 'browser_spa') await page.locator('#registration-prod-origins').fill(origin)
  await page.locator('#registration-prod-audience').fill(audience)
  await page.selectOption('#registration-prod-scopes', 'openid profile email')
  await page.selectOption('#registration-prod-logout', 'online')
}

async function registerApplication(
  page: Page,
  request: APIRequestContext,
  cfg: SsoConfig,
  options: {
    key: string
    name: string
    description: string
    thirdParty: boolean
    kind: 'server_web' | 'browser_spa'
    origin: string
    audience: string
  },
): Promise<{ appId: number; production: Environment; productionSecret: string | null }> {
  await page.goto(cfg.opOrigin + '/developer/apps')
  await page.getByRole('heading', { name: '开发者控制台' }).waitFor({ state: 'visible' })
  await openRegistration(page)
  await page.locator('#app-key').fill(options.key)
  await page.locator('#app-name').fill(options.name)
  await page.locator('#app-description').fill(options.description)
  if (options.thirdParty) await page.getByLabel('第三方应用').check()
  await nextStep(page, '开发环境')
  await page.selectOption('#registration-dev-kind', options.kind)
  await page.locator('#registration-dev-redirects').fill(options.origin + '/callback')
  await page.locator('#registration-dev-logout-redirects').fill(options.origin + '/')
  if (options.kind === 'browser_spa') {
    await page.locator('#registration-dev-origins').fill(options.origin)
  } else {
    await page.selectOption('#registration-dev-logout', 'backchannel_and_online')
    await page.locator('#registration-dev-backchannel').fill(options.origin + '/oidc/backchannel-logout')
  }
  await page.locator('#registration-dev-audience').fill(options.audience + ':dev')
  await page.selectOption('#registration-dev-scopes', 'openid profile email')
  await nextStep(page, '生产环境')
  await fillProduction(page, options.kind, options.origin, options.audience)
  await nextStep(page, '确认建立')
  const responsePromise = page.waitForResponse(
    (response) =>
      response.url() === cfg.opOrigin + '/api/v1/developer/apps' &&
      response.request().method() === 'POST',
    { timeout: 30_000 },
  )
  await page.getByRole('button', { name: '确认并建立应用', exact: true }).click()
  const response = await responsePromise
  const receipt = await json<RegistrationResponse>(response)
  await page.getByRole('heading', { name: '应用已建立' }).waitFor({ state: 'visible' })
  if (receipt.secrets?.length) {
    await page.getByRole('button', { name: '我已保存，隐藏回执' }).click()
    await page.getByRole('heading', { name: '应用已建立' }).waitFor({ state: 'detached' })
  }
  const app = await getApp(request, cfg, false, receipt.application_id)
  const production = app.environments.find((environment) => environment.kind === 'production')
  assert.ok(production, `${options.key} has no production environment`)
  const productionSecret =
    receipt.secrets?.find((secret) => secret.environment_id === production.environment_id)
      ?.client_secret ?? null
  if (options.kind === 'server_web') assert.ok(productionSecret, 'production web secret must be issued once')
  else assert.equal(productionSecret, null, 'production SPA must not receive a secret')
  return { appId: receipt.application_id, production, productionSecret }
}

async function selectSearchOption(
  page: Page,
  inputSelector: string,
  query: string,
  value: string,
): Promise<void> {
  const input = page.locator(inputSelector)
  await input.click()
  await input.fill(query)
  await page.locator(`.entity-picker-option[data-value="${value}"]`).first().click()
}

async function assignGroup(
  page: Page,
  cfg: SsoConfig,
  appId: number,
  groupId: string,
): Promise<void> {
  await page.goto(cfg.opOrigin + `/admin/apps/${appId}`)
  await page.locator('#detail-environment').waitFor({ state: 'visible' })
  await page.selectOption('#assign-kind', 'group')
  await selectSearchOption(page, '#assign-id', groupId, groupId)
  const present = page.getByLabel('授予访问（取消为移除）')
  if (!(await present.isChecked())) await present.check()
  const responsePromise = page.waitForResponse(
    (response) =>
      response.url() === cfg.opOrigin + `/api/v1/admin/apps/${appId}/assignments` &&
      response.request().method() === 'PATCH',
    { timeout: 30_000 },
  )
  await page.getByRole('button', { name: '更新明确分配' }).click()
  await json(await responsePromise)
  await waitForTextContaining(page.getByText(`用户组 ${groupId}`), `用户组 ${groupId}`)
}

async function selectEnvironment(page: Page, cfg: SsoConfig, appId: number, environmentId: string): Promise<void> {
  await page.goto(cfg.opOrigin + `/developer/apps/${appId}`)
  await page.locator('#detail-environment').waitFor({ state: 'visible' })
  await page.selectOption('#detail-environment', environmentId)
}

async function submitProduction(
  page: Page,
  cfg: SsoConfig,
  appId: number,
  environmentId: string,
): Promise<string> {
  await selectEnvironment(page, cfg, appId, environmentId)
  const button = page.getByRole('button', { name: '提交已保存的最新版本供审核' })
  await waitForEnabled(button)
  const responsePromise = page.waitForResponse(
    (response) =>
      response.url() ===
        cfg.opOrigin + `/api/v1/developer/apps/${appId}/environments/${environmentId}/submit` &&
      response.request().method() === 'POST',
    { timeout: 30_000 },
  )
  await button.click()
  const receipt = await json<ChangeResponse>(await responsePromise)
  const approvalId = receipt.summary.approval_id
  assert.ok(approvalId, 'production submit must return an approval id')
  await waitForTextContaining(page.getByText('生产申请待审核'), '生产申请待审核')
  return approvalId
}

async function reviewProduction(
  page: Page,
  cfg: SsoConfig,
  appId: number,
  environmentId: string,
  approvalId: string,
  accept: boolean,
): Promise<{ status: number; body: string }> {
  await page.goto(cfg.opOrigin + `/admin/apps/${appId}`)
  await page.locator('#detail-environment').waitFor({ state: 'visible' })
  await page.selectOption('#detail-environment', environmentId)
  const checkbox = page.getByLabel('接受并记录退出能力限制')
  if (accept && !(await checkbox.isChecked())) await checkbox.check()
  if (!accept && (await checkbox.isChecked())) await checkbox.uncheck()
  const responsePromise = page.waitForResponse(
    (response) =>
      response.url() ===
        cfg.opOrigin + `/api/v1/admin/apps/${appId}/approvals/${approvalId}/review` &&
      response.request().method() === 'POST',
    { timeout: 30_000 },
  )
  await page.getByRole('button', { name: '批准固定申请快照' }).click()
  const response = await responsePromise
  return { status: response.status(), body: await response.text() }
}

async function selfReviewMustFail(
  request: APIRequestContext,
  cfg: SsoConfig,
  appId: number,
  environmentId: string,
  approvalId: string,
): Promise<void> {
  const app = await getApp(request, cfg, false, appId)
  const environment = app.environments.find((candidate) => candidate.environment_id === environmentId)
  assert.ok(environment)
  const response = await write(
    request,
    cfg,
    'POST',
    `/admin/apps/${appId}/approvals/${approvalId}/review`,
    {
      expected_revision: environment.revision,
      approve: true,
      reason: null,
      accept_logout_limitation: true,
    },
    { 'idempotency-key': randomUUID() },
  )
  assert.equal(response.status(), 403, 'self review must be rejected')
}

async function proposeLocalOnly(
  page: Page,
  cfg: SsoConfig,
  appId: number,
  environmentId: string,
): Promise<void> {
  await selectEnvironment(page, cfg, appId, environmentId)
  await page.selectOption('#detail-config-logout', 'local_only')
  const responsePromise = page.waitForResponse(
    (response) =>
      response.url() ===
        cfg.opOrigin + `/api/v1/developer/apps/${appId}/environments/${environmentId}/config` &&
      response.request().method() === 'PUT',
    { timeout: 30_000 },
  )
  await page.getByRole('button', { name: '保存生产配置提案' }).click()
  await json(await responsePromise)
  await waitForTextContaining(page.getByText('当前："online"'), '当前："online"')
  await waitForTextContaining(page.getByText('提案："local_only"'), '提案："local_only"')
}

async function createReader(
  page: Page,
  cfg: SsoConfig,
  appId: number,
  environmentId: string,
): Promise<{ readerId: string; secret: string }> {
  await selectEnvironment(page, cfg, appId, environmentId)
  const button = page.getByRole('button', { name: '为生效 Audience 创建后端凭据' })
  await waitForEnabled(button)
  await button.click()
  const section = page
    .locator('section')
    .filter({ has: page.getByRole('heading', { name: /^后端内省凭据 / }) })
    .first()
  await section.waitFor({ state: 'visible' })
  const heading = ((await section.getByRole('heading').textContent()) ?? '').trim()
  const readerId = heading.replace('后端内省凭据 ', '').trim()
  const secret = ((await section.locator('code').textContent()) ?? '').trim()
  await section.getByRole('button', { name: '隐藏密钥' }).click()
  await section.waitFor({ state: 'detached' })
  return { readerId, secret }
}

async function getApproval(
  request: APIRequestContext,
  cfg: SsoConfig,
  appId: number,
  approvalId: string,
): Promise<Approval> {
  return json<Approval>(
    await request.get(
      cfg.opOrigin + `/api/v1/admin/apps/${appId}/approvals/${approvalId}`,
    ),
  )
}

async function requireConsentAndSignIn(
  page: Page,
  account: OnboardedAccount,
): Promise<void> {
  const username = page.getByLabel('用户名', { exact: true })
  const consent = page.getByRole('button', { name: '同意授权' })

  await Promise.race([
    username.waitFor({ state: 'visible' }),
    consent.waitFor({ state: 'visible' }),
  ])

  if (await username.isVisible()) {
    await username.fill(account.username)
    await page.getByLabel('密码', { exact: true }).fill(account.password)
    await page.getByRole('button', { name: '登录', exact: true }).click()
  }

  await consent.waitFor({ state: 'visible' })
  await consent.click()
  await page.getByTestId('display-name').waitFor({ state: 'visible' })
}

export async function runProductionGovernanceScenario(browser: Browser, engine: string): Promise<void> {
  const cfg = config()
  const suffix = `${engine}-${randomUUID().slice(0, 8)}`
  const password = `Governance-${suffix}-Passw0rd!`
  const developer: Account = {
    username: `c70-developer-${suffix}`,
    password,
    nickname: 'Production Developer',
    kind: 'employee',
    developer: true,
  }
  const reviewer: Account = {
    username: `c70-admin-${suffix}`,
    password,
    nickname: 'Production Reviewer',
    kind: 'employee',
    developer: false,
  }
  const assignedUser: Account = {
    username: `c70-assigned-${suffix}`,
    password,
    nickname: 'Assigned User',
    kind: 'employee',
    developer: false,
  }
  const unassignedUser: Account = {
    username: `c70-unassigned-${suffix}`,
    password,
    nickname: 'Unassigned User',
    kind: 'employee',
    developer: false,
  }
  const onboarded: OnboardedAccount[] = []
  let examples: GeneratedExamples | undefined
  let assignedContext: BrowserContext | undefined
  let unassignedContext: BrowserContext | undefined
  const rootContext = await browser.newContext({ ignoreHTTPSErrors: true })
  try {
    const rootPage = await rootContext.newPage()
    rootPage.setDefaultTimeout(30_000)
    await centerLogin(rootPage, cfg, cfg.username, cfg.password)
    for (const account of [developer, reviewer, assignedUser, unassignedUser]) {
      const secret = await issueInvitation(rootPage.request, cfg, account)
      const context = await browser.newContext({ ignoreHTTPSErrors: true })
      const page = await context.newPage()
      page.setDefaultTimeout(30_000)
      const userId = await acceptInvitation(context, cfg, account, secret)
      onboarded.push({ ...account, context, page, userId })
    }
    const developerAccount = onboarded.find((account) => account.username === developer.username)!
    const reviewerAccount = onboarded.find((account) => account.username === reviewer.username)!
    const assigned = onboarded.find((account) => account.username === assignedUser.username)!
    const unassigned = onboarded.find((account) => account.username === unassignedUser.username)!
    await grantEnterpriseAdministrator(rootPage.request, cfg, cfg.password, developerAccount.userId)
    await grantEnterpriseAdministrator(rootPage.request, cfg, cfg.password, reviewerAccount.userId)

    const group = await createGroup(rootPage.request, cfg, `Production users ${suffix}`)
    await addGroupMember(rootPage.request, cfg, group.groupId, assigned.userId, group.revision)

    await centerLogin(developerAccount.page, cfg, developerAccount.username, developerAccount.password)
    const origins = await allocateExampleOrigins()
    const webAudience = `urn:c70:web:${suffix}`
    const spaAudience = `urn:c70:spa:${suffix}`
    const web = await registerApplication(developerAccount.page, developerAccount.context.request, cfg, {
      key: `c70-web-${suffix}`,
      name: 'C70 Web A',
      description: 'Production governance Web application',
      thirdParty: false,
      kind: 'server_web',
      origin: origins.webOrigin,
      audience: webAudience,
    })
    const spa = await registerApplication(developerAccount.page, developerAccount.context.request, cfg, {
      key: `c70-spa-${suffix}`,
      name: 'C70 SPA B',
      description: 'Production governance third-party SPA',
      thirdParty: true,
      kind: 'browser_spa',
      origin: origins.spaOrigin,
      audience: spaAudience,
    })
    assert.equal((await getApp(developerAccount.context.request, cfg, false, spa.appId)).third_party, true)

    await assignGroup(rootPage, cfg, web.appId, group.groupId)
    await assignGroup(rootPage, cfg, spa.appId, group.groupId)

    await centerLogin(reviewerAccount.page, cfg, reviewerAccount.username, reviewerAccount.password)
    const firstApprovals: { appId: number; environmentId: string; approvalId: string }[] = []
    for (const application of [web, spa]) {
      const approvalId = await submitProduction(
        developerAccount.page,
        cfg,
        application.appId,
        application.production.environment_id,
      )
      firstApprovals.push({
        appId: application.appId,
        environmentId: application.production.environment_id,
        approvalId,
      })
    }
    for (const approval of firstApprovals) {
      const result = await reviewProduction(
        reviewerAccount.page,
        cfg,
        approval.appId,
        approval.environmentId,
        approval.approvalId,
        false,
      )
      assert.equal(result.status, 200, 'the first online production version must be approvable')
    }

    const pendingApprovals: {
      appId: number
      environmentId: string
      approvalId: string
      clientId: string
    }[] = []
    for (const application of [web, spa]) {
      await proposeLocalOnly(
        developerAccount.page,
        cfg,
        application.appId,
        application.production.environment_id,
      )
      const approvalId = await submitProduction(
        developerAccount.page,
        cfg,
        application.appId,
        application.production.environment_id,
      )
      const current = await getApp(developerAccount.context.request, cfg, false, application.appId)
      const environment = current.environments.find(
        (candidate) => candidate.environment_id === application.production.environment_id,
      )!
      assert.equal(environment.active_config?.logout_capability, 'online')
      assert.equal(environment.latest.config.logout_capability, 'local_only')
      assert.ok(environment.pending_approval)
      pendingApprovals.push({
        appId: application.appId,
        environmentId: environment.environment_id,
        approvalId,
        clientId: environment.client_id,
      })
    }

    const selfReviewStatuses: number[] = []
    const noAcceptStatuses: number[] = []
    for (const approval of pendingApprovals) {
      await selfReviewMustFail(
        developerAccount.context.request,
        cfg,
        approval.appId,
        approval.environmentId,
        approval.approvalId,
      )
      selfReviewStatuses.push(403)
      const denied = await reviewProduction(
        reviewerAccount.page,
        cfg,
        approval.appId,
        approval.environmentId,
        approval.approvalId,
        false,
      )
      assert.equal(denied.status, 400, 'LocalOnly approval without acknowledgement must fail')
      noAcceptStatuses.push(denied.status)
      const stillPending = await getApp(reviewerAccount.context.request, cfg, true, approval.appId)
      const environment = stillPending.environments.find(
        (candidate) => candidate.environment_id === approval.environmentId,
      )!
      assert.equal(environment.pending_approval, approval.approvalId)
      assert.equal(environment.active_config?.logout_capability, 'online')
    }

    for (const approval of pendingApprovals) {
      const approved = await reviewProduction(
        reviewerAccount.page,
        cfg,
        approval.appId,
        approval.environmentId,
        approval.approvalId,
        true,
      )
      assert.equal(approved.status, 200, 'LocalOnly approval must succeed after acknowledgement')
      const detail = await getApproval(reviewerAccount.context.request, cfg, approval.appId, approval.approvalId)
      assert.equal(detail.status, 'approved')
      assert.equal(detail.logout_limitation_accepted, true)
      const current = await getApp(reviewerAccount.context.request, cfg, true, approval.appId)
      const environment = current.environments.find(
        (candidate) => candidate.environment_id === approval.environmentId,
      )!
      assert.equal(environment.pending_approval, null)
      assert.equal(environment.active_config?.logout_capability, 'local_only')
    }

    const webReader = await createReader(
      developerAccount.page,
      cfg,
      web.appId,
      web.production.environment_id,
    )
    const spaReader = await createReader(
      developerAccount.page,
      cfg,
      spa.appId,
      spa.production.environment_id,
    )
    examples = await startGeneratedExamples(cfg, origins, {
      clientId: web.production.client_id,
      clientSecret: web.productionSecret!,
      readerClientId: webReader.readerId,
      readerClientSecret: webReader.secret,
      sessionSecret: `c70-session-${suffix}-at-least-32-characters`,
    }, {
      clientId: spa.production.client_id,
      apiAudience: spaAudience,
      readerClientId: spaReader.readerId,
      readerClientSecret: spaReader.secret,
    })

    unassignedContext = await browser.newContext({ ignoreHTTPSErrors: true })
    const unassignedPage = await unassignedContext.newPage()
    unassignedPage.setDefaultTimeout(30_000)
    await unassignedPage.goto(examples.webOrigin + '/')
    await unassignedPage.getByRole('link', { name: /Sign in with Cobotrix/ }).click()
    await unassignedPage.getByLabel('用户名', { exact: true }).fill(unassigned.username)
    await unassignedPage.getByLabel('密码', { exact: true }).fill(unassigned.password)
    await unassignedPage.getByRole('button', { name: '登录', exact: true }).click()
    await unassignedPage.getByRole('heading', { name: 'Sign-in failed' }).waitFor({ state: 'visible' })
    await waitForTextContaining(unassignedPage.locator('body'), 'access_denied')
    assert.equal(await unassignedPage.getByTestId('display-name').count(), 0)
    assert.equal(
      (await unassignedContext.cookies(examples.webOrigin)).some(
        (cookie) => cookie.name === 'cobotrix_session',
      ),
      false,
    )

    assignedContext = await browser.newContext({ ignoreHTTPSErrors: true })
    const webPage = await assignedContext.newPage()
    webPage.setDefaultTimeout(30_000)
    await webPage.goto(examples.webOrigin + '/')
    await webPage.getByRole('link', { name: /Sign in with Cobotrix/ }).click()
    await requireConsentAndSignIn(webPage, assigned)
    await waitForTextContaining(webPage.getByTestId('display-name'), assigned.username)
    await waitForTextContaining(webPage.getByTestId('local-subject'), String(assigned.userId))
    const csrfValue = await webPage
      .locator('form')
      .filter({ hasText: 'Run protected operation' })
      .locator('input[name="_csrf"]')
      .inputValue()
    const webOperation = await assignedContext.request.post(examples.webOrigin + '/api/operation', {
      headers: { origin: examples.webOrigin },
      form: { _csrf: csrfValue },
    })
    assert.equal(webOperation.status(), 200, await webOperation.text())

    const spaPage = await assignedContext.newPage()
    spaPage.setDefaultTimeout(30_000)
    await spaPage.goto(examples.spaOrigin + '/')
    await spaPage.getByRole('button', { name: 'Sign in with Cobotrix' }).click()
    await requireConsentAndSignIn(spaPage, assigned)
    await waitForTextContaining(spaPage.getByTestId('display-name'), assigned.username)
    await waitForTextContaining(spaPage.getByTestId('local-subject'), String(assigned.userId))
    await spaPage.getByRole('button', { name: 'Run protected operation' }).click()
    await waitForTextContaining(spaPage.getByRole('status'), 'Operation accepted.')

    const evidence = {
      engine,
      applications: pendingApprovals.map((approval) => ({
        appId: approval.appId,
        clientId: approval.clientId,
      })),
      thirdPartySpa: true,
      groupAssignment: group.groupId,
      oldActiveLogout: 'online',
      pendingLogout: 'local_only',
      selfReviewStatuses,
      noAcceptStatuses,
      finalApprovals: pendingApprovals.length,
      webSecretDeliveredOnce: Boolean(web.productionSecret),
      webReader: webReader.readerId,
      spaReader: spaReader.readerId,
      unassignedUser: {
        userId: unassigned.userId,
        loginRejected: true,
        applicationSessionCookie: false,
      },
      assignedUser: {
        userId: assigned.userId,
        webConsentShown: true,
        spaConsentShown: true,
        webProtectedOperation: webOperation.status(),
        spaProtectedOperation: 'accepted',
      },
    }
    console.log(`E3 production governance passed: ${JSON.stringify(evidence)}`)
  } finally {
    await assignedContext?.close()
    await unassignedContext?.close()
    await examples?.stop()
    await rootContext.close()
    await Promise.all(onboarded.map((account) => account.context.close()))
  }
}
