// WebKit fallback for hosts where the Playwright 1.65 driver and the available
// WebKit build disagree on protocol settings. Mirrors the three Playwright
// specs with the 1.61 driver already used by real-auth.mjs.
import assert from 'node:assert/strict'
import { log } from 'node:console'
import { randomUUID } from 'node:crypto'
import { setTimeout as delay } from 'node:timers/promises'

import { webkit } from 'playwright-webkit'

import setup, {
  allocateExampleOrigins,
  awaitConsentIfShown,
  config,
  loginAtCenter,
  signToken,
  startGeneratedExamples,
} from './enterprise-sso-fixture.ts'
import { runCrossSiteProtocolScenario } from './enterprise-sso-cross-site-protocol.ts'
import { runProductionGovernanceScenario } from './enterprise-sso-production-governance.ts'

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

async function selectSearchOption(page, inputSelector, query, value) {
  const input = page.locator(inputSelector)
  await input.click()
  await input.fill(query)
  await page.locator(`.entity-picker-option[data-value="${value}"]`).first().click()
}

async function waitForTextContaining(locator, expected, timeout = 20_000) {
  const deadline = Date.now() + timeout
  let observed = ''
  while (Date.now() < deadline) {
    observed = ((await locator.textContent().catch(() => null)) ?? '').trim()
    if (observed.includes(expected)) return
    await delay(100)
  }
  throw new Error(
    `expected text containing ${JSON.stringify(expected)}, observed ${JSON.stringify(observed)}`,
  )
}

async function json(response, expected = 200) {
  const body = await response.text()
  assert.equal(response.status(), expected, body)
  return JSON.parse(body)
}

async function csrf(request, path = '/api/v1/identity/csrf') {
  const response = await request.get(cfg.opOrigin + path, {
    headers: { origin: cfg.opOrigin },
  })
  return (await json(response)).csrf_token
}

async function write(request, method, path, data, headers = {}) {
  return request.fetch(cfg.opOrigin + '/api/v1' + path, {
    method,
    headers: {
      origin: cfg.opOrigin,
      'x-csrf-token': await csrf(request),
      ...headers,
    },
    data,
  })
}

async function centerSignIn(page, username, password) {
  await page.goto(cfg.opOrigin + '/login')
  await page.getByLabel('用户名', { exact: true }).fill(username)
  await page.getByLabel('密码', { exact: true }).fill(password)
  await page.getByRole('button', { name: '登录', exact: true }).click()
  await page.waitForURL(
    (url) => url.origin === cfg.opOrigin && url.pathname === '/account',
    { timeout: 30_000 },
  )
  await page.locator('.nav-group-title').first().waitFor()
}

async function navigationGroups(page) {
  await page.locator('.nav-group-title').first().waitFor()
  return (await page.locator('.nav-group-title').allTextContents()).map((title) => title.trim())
}

async function issueInvitation(request, account) {
  const response = await write(request, 'POST', '/admin/invitations', {
    purpose: 'onboarding',
    person_kind: account.kind,
    developer_grant: account.developer,
    target_user_id: null,
  })
  return (await json(response)).secret
}

async function acceptInvitation(context, account, secret) {
  const exchange = await write(context.request, 'POST', '/identity/actions/exchange', {
    kind: 'invitation',
    secret,
  })
  const action = await json(exchange)
  const accepted = await write(context.request, 'POST', '/identity/invitations/accept', {
    action_id: action.action_id,
    existing_account: false,
    username: account.username,
    password: account.password,
    nickname: account.nickname,
  })
  const receipt = await json(accepted)
  assert.equal(receipt.requires_login, true)
  return receipt.user_id
}

async function createGroup(request, name) {
  const response = await write(
    request,
    'POST',
    '/admin/groups',
    { name, description: 'C68 enterprise onboarding fixture' },
    { 'idempotency-key': randomUUID() },
  )
  const receipt = await json(response)
  return { groupId: receipt.summary.group_id, revision: receipt.summary.revision }
}

async function addGroupMember(request, groupId, userId, revision) {
  const response = await write(
    request,
    'PATCH',
    `/admin/groups/${groupId}/members/${userId}`,
    { present: true, expected_revision: revision },
    { 'idempotency-key': randomUUID() },
  )
  return (await json(response)).summary.revision
}

async function grantEnterpriseAdministrator(request, username, password, userId) {
  const login = await request.post(cfg.opOrigin + '/api/v1/login', {
    headers: {
      origin: cfg.opOrigin,
      'x-csrf-token': await csrf(request, '/api/v1/csrf'),
    },
    data: { username, password },
  })
  const session = await json(login)
  const token = session.access_token
  const listed = await request.get(
    cfg.opOrigin + '/api/v1/authority?page_number=1&page_size=500',
    { headers: { authorization: 'Bearer ' + token } },
  )
  const authorities = (await json(listed)).data_list.filter(
    (authority) =>
      String(authority.name).startsWith('identity.uri:') &&
      String(authority.authority_uri).startsWith('/admin/'),
  )
  assert.ok(authorities.length > 0)
  for (const authority of authorities) {
    const response = await request.post(cfg.opOrigin + '/api/v1/user_authority_override', {
      headers: {
        authorization: 'Bearer ' + token,
        origin: cfg.opOrigin,
        'x-csrf-token': session.csrf_token,
      },
      data: { user_id: userId, authority_id: authority.id, allow: true },
    })
    await json(response)
  }
  return authorities.length
}

async function expectStatus(request, path, expected) {
  const response = await request.get(cfg.opOrigin + path)
  assert.equal(response.status(), expected, await response.text())
}

async function waitForEnabled(locator) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    if (await locator.isEnabled()) return
    await delay(100)
  }
  throw new Error('expected control to become enabled')
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

async function assertEnterpriseOnboarding(browser) {
  const suffix = `webkit-${randomUUID().slice(0, 8)}`
  const password = `Enterprise-${suffix}-Passw0rd!`
  const secondAdmin = {
    username: `c68-admin-${suffix}`,
    password,
    nickname: 'Second Administrator',
    kind: 'employee',
    developer: false,
  }
  const employee = {
    username: `c68-employee-${suffix}`,
    password,
    nickname: 'Employee',
    kind: 'employee',
    developer: false,
  }
  const developers = [
    {
      username: `c68-developer-a-${suffix}`,
      password,
      nickname: 'Developer A',
      kind: 'employee',
      developer: true,
    },
    {
      username: `c68-developer-b-${suffix}`,
      password,
      nickname: 'Developer B',
      kind: 'employee',
      developer: true,
    },
  ]
  const collaborator = {
    username: `c68-partner-${suffix}`,
    password,
    nickname: 'Partner',
    kind: 'collaborator',
    developer: false,
  }
  const onboarded = []
  try {
    const adminContext = await openContext(browser)
    const adminPage = await adminContext.newPage()
    adminPage.setDefaultTimeout(30_000)
    await centerSignIn(adminPage, cfg.username, cfg.password)

    for (const account of [secondAdmin, employee, ...developers, collaborator]) {
      const secret = await issueInvitation(adminPage.request, account)
      const context = await openContext(browser)
      const page = await context.newPage()
      page.setDefaultTimeout(30_000)
      const userId = await acceptInvitation(context, account, secret)
      onboarded.push({ ...account, context, page, userId })
    }

    const directory = await json(
      await adminPage.request.get(cfg.opOrigin + '/api/v1/admin/accounts?limit=100'),
    )
    for (const account of onboarded) {
      const row = directory.items.find((candidate) => candidate.username === account.username)
      assert.ok(row, account.username)
      assert.equal(row.person_kind, account.kind)
      assert.equal(row.developer_enabled, account.developer)
      assert.equal(row.enabled, true)
    }

    const groupName = `Employees ${suffix}`
    const group = await createGroup(adminPage.request, groupName)
    let revision = group.revision
    const employeeMembers = onboarded.filter((account) => account.kind === 'employee')
    for (const member of employeeMembers) {
      revision = await addGroupMember(
        adminPage.request,
        group.groupId,
        member.userId,
        revision,
      )
    }

    const groups = await json(
      await adminPage.request.get(cfg.opOrigin + '/api/v1/admin/groups?limit=50'),
    )
    const employeeGroup = groups.items.find((candidate) => candidate.group_id === group.groupId)
    assert.ok(employeeGroup, groupName)
    for (const member of employeeMembers) {
      assert.ok(employeeGroup.members.includes(member.userId))
    }
    const partner = onboarded.find((account) => account.username === collaborator.username)
    assert.ok(partner)
    assert.ok(!employeeGroup.members.includes(partner.userId))

    const second = onboarded.find((account) => account.username === secondAdmin.username)
    assert.ok(second)
    const grantedAuthorities = await grantEnterpriseAdministrator(
      adminPage.request,
      cfg.username,
      cfg.password,
      second.userId,
    )

    for (const account of onboarded) {
      await centerSignIn(account.page, account.username, account.password)
      await expectStatus(account.context.request, '/api/v1/account/apps', 200)
      const apps = await json(
        await account.context.request.get(cfg.opOrigin + '/api/v1/account/apps'),
      )
      assert.deepEqual(apps.items, [])
    }

    const employeeAccount = onboarded.find((account) => account.username === employee.username)
    assert.ok(employeeAccount)
    for (const account of [employeeAccount, partner]) {
      const groups = await navigationGroups(account.page)
      assert.ok(groups.includes('账户中心'))
      assert.ok(groups.includes('快捷工作流'))
      assert.ok(!groups.includes('开发者控制台'))
      assert.ok(!groups.includes('企业管理'))
      await expectStatus(account.context.request, '/api/v1/developer/apps', 403)
      await expectStatus(account.context.request, '/api/v1/admin/apps', 403)
    }

    for (const account of onboarded.filter((candidate) => candidate.developer)) {
      const groups = await navigationGroups(account.page)
      assert.ok(groups.includes('账户中心'))
      assert.ok(groups.includes('开发者控制台'))
      assert.ok(!groups.includes('企业管理'))
      await expectStatus(account.context.request, '/api/v1/developer/apps', 200)
      await expectStatus(account.context.request, '/api/v1/admin/apps', 403)
    }

    const secondGroups = await navigationGroups(second.page)
    assert.ok(secondGroups.includes('账户中心'))
    assert.ok(secondGroups.includes('企业管理'))
    assert.ok(!secondGroups.includes('开发者控制台'))
    await expectStatus(second.context.request, '/api/v1/admin/accounts', 200)
    await expectStatus(second.context.request, '/api/v1/admin/groups', 200)
    // Administrators may read the developer list to inspect apps they own, but
    // the console stays hidden and they own no applications here.
    const adminDeveloperApps = await json(
      await second.context.request.get(cfg.opOrigin + '/api/v1/developer/apps'),
    )
    assert.deepEqual(adminDeveloperApps.items, [])
    await expectStatus(second.context.request, '/api/v1/admin/apps', 200)

    log(
      'WebKit: C68 enterprise onboarding passed ' +
        JSON.stringify({
          engine: 'webkit-1.61',
          accounts: {
            secondAdministrator: secondAdmin.username,
            employee: employee.username,
            developers: developers.map((account) => account.username),
            collaborator: collaborator.username,
          },
          group: {
            name: groupName,
            members: employeeMembers.length,
            collaboratorIncluded: false,
          },
          grantedAuthorities,
          boundaries: {
            ordinaryApplications: 0,
            collaboratorDeveloperAccess: 403,
            collaboratorAdministration: 403,
            secondAdministratorAccounts: 200,
          },
        }),
    )
    await adminContext.close()
  } finally {
    await Promise.all(onboarded.map((account) => account.context.close()))
  }
}

async function openRegistration(page) {
  await page.getByRole('button', { name: '注册应用', exact: true }).click()
  await page.locator('#app-key').waitFor({ state: 'visible' })
}

async function fillBasic(page, key, name, description) {
  await page.locator('#app-key').fill(key)
  await page.locator('#app-name').fill(name)
  await page.locator('#app-description').fill(description)
}

async function nextStep(page, title) {
  await page.getByRole('button', { name: '下一步', exact: true }).click()
  await waitForText(page.locator('#registration-step-title'), title)
}

async function fillWebDevelopment(page, origin, audience) {
  await page.selectOption('#registration-dev-kind', 'server_web')
  await page.locator('#registration-dev-redirects').fill(origin + '/callback')
  await page.locator('#registration-dev-logout-redirects').fill(origin + '/')
  await page.locator('#registration-dev-audience').fill(audience)
  await page.selectOption('#registration-dev-scopes', 'openid profile email')
  await page.selectOption('#registration-dev-logout', 'backchannel_and_online')
  await page.locator('#registration-dev-backchannel').fill(origin + '/oidc/backchannel-logout')
}

async function fillSpaDevelopment(page, origin, audience) {
  await page.selectOption('#registration-dev-kind', 'browser_spa')
  await page.locator('#registration-dev-redirects').fill(origin + '/callback')
  await page.locator('#registration-dev-logout-redirects').fill(origin + '/')
  await page.locator('#registration-dev-origins').fill(origin)
  await page.locator('#registration-dev-audience').fill(audience)
  await page.selectOption('#registration-dev-scopes', 'openid profile email')
  await page.selectOption('#registration-dev-logout', 'online')
}

async function fillProductionDraft(page, kind, audience) {
  await page.selectOption('#registration-prod-kind', kind)
  await page.locator('#registration-prod-audience').fill(audience)
}

async function confirmRegistration(page) {
  await page.getByRole('button', { name: '确认并建立应用', exact: true }).click()
  await page.getByRole('heading', { name: '应用已建立' }).waitFor({ state: 'visible' })
}

async function readReceipt(page) {
  const receipt = page.locator('section').filter({ hasText: '应用已建立' }).first()
  const href = await receipt
    .getByRole('link', { name: '查看接入配置' })
    .getAttribute('href')
  assert.match(href ?? '', /^\/developer\/apps\/\d+$/)
  const appId = Number(href.split('/').pop())
  const codes = await receipt.locator('code').allTextContents()
  const secret = codes.length > 1 ? codes[1].trim() : null
  return { appId, secret }
}

async function getApp(request, administrator, id) {
  return json(
    await request.get(
      cfg.opOrigin + `/api/v1/${administrator ? 'admin' : 'developer'}/apps/${id}`,
    ),
  )
}

async function createReader(page, appId, environmentId) {
  await page.goto(cfg.opOrigin + `/developer/apps/${appId}`)
  await page.locator('#detail-environment').waitFor({ state: 'visible' })
  await page.selectOption('#detail-environment', environmentId)
  const button = page.getByRole('button', { name: '为生效 Audience 创建后端凭据' })
  await waitForEnabled(button)
  await button.click()
  const section = page
    .locator('section')
    .filter({ has: page.getByRole('heading', { name: /^后端内省凭据 / }) })
    .first()
  await section.waitFor({ state: 'visible' })
  const heading = (await section.getByRole('heading').textContent()).trim()
  const readerId = heading.replace('后端内省凭据 ', '').trim()
  const secret = (await section.locator('code').textContent()).trim()
  await section.getByRole('button', { name: '隐藏密钥' }).click()
  assert.equal(await section.count(), 0)
  return { readerId, secret }
}

async function addTestUser(request, appId, userId) {
  const app = await getApp(request, true, appId)
  const environment = app.environments.find((candidate) => candidate.kind === 'development')
  assert.ok(environment, `application ${appId} has no development environment`)
  const response = await write(
    request,
    'PATCH',
    `/admin/apps/${appId}/environments/${environment.environment_id}/test-users/${userId}`,
    { present: true, expected_revision: app.revision },
    { 'idempotency-key': randomUUID() },
  )
  await json(response)
  return environment.environment_id
}

async function assertDeveloperOnboarding(browser) {
  const suffix = `webkit-${randomUUID().slice(0, 8)}`
  const password = `Developer-${suffix}-Passw0rd!`
  const developerA = {
    username: `c69-dev-a-${suffix}`,
    password,
    nickname: 'Developer A',
    kind: 'employee',
    developer: true,
  }
  const developerB = {
    username: `c69-dev-b-${suffix}`,
    password,
    nickname: 'Developer B',
    kind: 'employee',
    developer: true,
  }
  const employee = {
    username: `c69-user-${suffix}`,
    password,
    nickname: 'Test User',
    kind: 'employee',
    developer: false,
  }
  const onboarded = []
  let examples
  let userContext
  try {
    const adminContext = await openContext(browser)
    const adminPage = await adminContext.newPage()
    adminPage.setDefaultTimeout(30_000)
    await centerSignIn(adminPage, cfg.username, cfg.password)

    for (const account of [developerA, developerB, employee]) {
      const secret = await issueInvitation(adminPage.request, account)
      const context = await openContext(browser)
      const page = await context.newPage()
      page.setDefaultTimeout(30_000)
      const userId = await acceptInvitation(context, account, secret)
      onboarded.push({ ...account, context, page, userId })
    }
    const a = onboarded.find((account) => account.username === developerA.username)
    const b = onboarded.find((account) => account.username === developerB.username)
    const user = onboarded.find((account) => account.username === employee.username)
    assert.ok(a && b && user)

    await json(
      await write(adminPage.request, 'PATCH', '/admin/collaboration-candidates', {
        requester_user_id: a.userId,
        candidate_user_id: b.userId,
        allowed: true,
      }),
    )

    await centerSignIn(a.page, a.username, a.password)
    const origins = await allocateExampleOrigins()
    const webAudience = `urn:c69:web:${suffix}`
    const spaAudience = `urn:c69:spa:${suffix}`

    await a.page.goto(cfg.opOrigin + '/developer/apps')
    await a.page.getByRole('heading', { name: '开发者控制台' }).waitFor({ state: 'visible' })
    await openRegistration(a.page)
    await fillBasic(a.page, `c69-web-${suffix}`, 'C69 Web A', 'Server-side web example')
    await nextStep(a.page, '开发环境')
    await fillWebDevelopment(a.page, origins.webOrigin, webAudience)
    await nextStep(a.page, '生产环境')
    await fillProductionDraft(a.page, 'server_web', webAudience + ':production')
    await nextStep(a.page, '确认建立')
    await confirmRegistration(a.page)
    const webReceipt = await readReceipt(a.page)
    assert.ok(webReceipt.secret, 'Web A must receive a one-time client secret')
    await a.page.getByText('开发环境密钥（仅此次展示）').waitFor({ state: 'visible' })
    await a.page.getByRole('button', { name: '我已保存，隐藏回执' }).click()
    assert.equal(
      await a.page.getByRole('heading', { name: '应用已建立' }).count(),
      0,
    )

    await a.page.goto(cfg.opOrigin + `/developer/apps/${webReceipt.appId}`)
    await a.page.getByText(/Client ID：/).waitFor({ state: 'visible' })
    const webApp = await getApp(a.context.request, false, webReceipt.appId)
    const webDev = webApp.environments.find((environment) => environment.kind === 'development')
    assert.ok(webDev, `application ${webReceipt.appId} has no development environment`)
    assert.equal(webDev.primary_client_kind, 'server_web')
    assert.ok(webDev.active_version_id)
    assert.ok(!JSON.stringify(webApp).includes(webReceipt.secret))
    const webReader = await createReader(a.page, webReceipt.appId, webDev.environment_id)
    const webReaders = await json(
      await a.context.request.get(
        cfg.opOrigin +
          `/api/v1/developer/apps/${webReceipt.appId}/environments/${webDev.environment_id}/integration/readers`,
      ),
    )
    assert.ok(
      webReaders.items.some(
        (reader) => reader.reader_id === webReader.readerId && reader.enabled,
      ),
    )

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
    assert.equal(spaReceipt.secret, null, 'SPA B must not receive a client secret')
    assert.equal(await a.page.getByText('密钥（仅此次展示）').count(), 0)
    await a.page.getByRole('button', { name: '我已保存，隐藏回执' }).click()

    await a.page.goto(cfg.opOrigin + `/developer/apps/${spaReceipt.appId}`)
    const spaApp = await getApp(a.context.request, false, spaReceipt.appId)
    const spaDev = spaApp.environments.find((environment) => environment.kind === 'development')
    assert.ok(spaDev, `application ${spaReceipt.appId} has no development environment`)
    assert.equal(spaDev.primary_client_kind, 'browser_spa')
    assert.ok(spaDev.active_version_id)
    assert.ok(!JSON.stringify(spaApp).includes('client_secret'))
    const spaReader = await createReader(a.page, spaReceipt.appId, spaDev.environment_id)
    assert.notEqual(webDev.client_id, spaDev.client_id)
    assert.notEqual(origins.webOrigin, origins.spaOrigin)

    await a.page.goto(cfg.opOrigin + `/developer/apps/${webReceipt.appId}`)
    const candidate = a.page.locator('#detail-candidate')
    await candidate.waitFor({ state: 'visible' })
    await selectSearchOption(a.page, '#detail-candidate', String(b.userId), String(b.userId))
    await a.page.getByRole('button', { name: '添加维护者' }).click()
    await waitForTextContaining(
      a.page.getByText(`账户 ${b.userId} · 维护者`),
      `账户 ${b.userId} · 维护者`,
    )

    await centerSignIn(b.page, b.username, b.password)
    await b.page.goto(cfg.opOrigin + `/developer/apps/${webReceipt.appId}`)
    await b.page.getByRole('heading', { name: 'C69 Web A' }).waitFor({ state: 'visible' })
    await b.page.locator('#detail-description').fill('Maintained by Developer B')
    const metadataResponse = b.page.waitForResponse(
      (response) =>
        response.url() ===
          cfg.opOrigin + `/api/v1/developer/apps/${webReceipt.appId}` &&
        response.request().method() === 'PATCH',
      { timeout: 30_000 },
    )
    await b.page.getByRole('button', { name: '保存资料' }).click()
    await json(await metadataResponse)
    await b.page.reload()
    assert.equal(
      await b.page.locator('#detail-description').inputValue(),
      'Maintained by Developer B',
    )

    await a.page.reload()
    await a.page.locator('#detail-candidate').waitFor({ state: 'visible' })
    await selectSearchOption(a.page, '#detail-candidate', String(b.userId), String(b.userId))
    await a.page.getByRole('button', { name: '交接负责人' }).click()
    await waitForTextContaining(
      a.page.getByText(`负责人账户 ${b.userId}`),
      `负责人账户 ${b.userId}`,
    )
    await waitForTextContaining(
      a.page.getByText(`账户 ${a.userId} · 维护者`),
      `账户 ${a.userId} · 维护者`,
    )
    assert.equal(await a.page.getByRole('button', { name: '交接负责人' }).count(), 0)
    await b.page.reload()
    await waitForTextContaining(
      b.page.getByText(`负责人账户 ${b.userId}`),
      `负责人账户 ${b.userId}`,
    )
    assert.equal(await b.page.getByRole('button', { name: '交接负责人' }).count(), 1)

    const webEnvironmentId = await addTestUser(adminPage.request, webReceipt.appId, user.userId)
    const spaEnvironmentId = await addTestUser(adminPage.request, spaReceipt.appId, user.userId)
    assert.equal(webEnvironmentId, webDev.environment_id)
    assert.equal(spaEnvironmentId, spaDev.environment_id)

    examples = await startGeneratedExamples(
      cfg,
      origins,
      {
        clientId: webDev.client_id,
        clientSecret: webReceipt.secret,
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

    userContext = await openContext(browser)
    const webPage = await userContext.newPage()
    webPage.setDefaultTimeout(30_000)
    await webPage.goto(examples.webOrigin + '/')
    await webPage.getByRole('link', { name: /Sign in with Cobotrix/ }).click()
    await webPage.getByLabel('用户名', { exact: true }).fill(user.username)
    await webPage.getByLabel('密码', { exact: true }).fill(user.password)
    await webPage.getByRole('button', { name: '登录', exact: true }).click()
    await awaitConsentIfShown(webPage, cfg)
    await waitForText(webPage.getByTestId('display-name'), user.username)
    await waitForText(webPage.getByTestId('local-subject'), String(user.userId))
    assert.equal(await webPage.evaluate(() => globalThis.sessionStorage.length), 0)
    const csrfValue = await webPage
      .locator('form')
      .filter({ hasText: 'Run protected operation' })
      .locator('input[name="_csrf"]')
      .inputValue()
    const webOperation = await userContext.request.post(examples.webOrigin + '/api/operation', {
      headers: { origin: examples.webOrigin },
      form: { _csrf: csrfValue },
    })
    assert.equal(webOperation.status(), 200, await webOperation.text())
    assert.deepEqual(await webOperation.json(), { ok: true, operation: 'example-write' })
    const webCookies = await userContext.cookies(examples.webOrigin)
    assert.ok(webCookies.some((cookie) => cookie.name === 'cobotrix_session'))
    assert.deepEqual(await userContext.cookies(examples.spaOrigin), [])

    const spaPage = await userContext.newPage()
    spaPage.setDefaultTimeout(30_000)
    await spaPage.goto(examples.spaOrigin + '/')
    await spaPage.getByRole('button', { name: 'Sign in with Cobotrix' }).click()
    await awaitConsentIfShown(spaPage, cfg)
    await waitForText(spaPage.getByTestId('display-name'), user.username)
    await waitForText(spaPage.getByTestId('local-subject'), String(user.userId))
    assert.equal(await spaPage.getByLabel('密码', { exact: true }).count(), 0)
    const spaStorageKeys = await spaPage.evaluate(() =>
      Object.keys(globalThis.sessionStorage),
    )
    assert.ok(spaStorageKeys.length > 0)
    const spaCookies = await userContext.cookies(examples.spaOrigin)
    assert.ok(!spaCookies.some((cookie) => cookie.name === 'cobotrix_session'))
    await spaPage.getByRole('button', { name: 'Run protected operation' }).click()
    await waitForText(spaPage.getByRole('status'), 'Operation accepted.')

    log(
      'WebKit: C69 developer onboarding passed ' +
        JSON.stringify({
          engine: 'webkit-1.61',
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
        }),
    )
    await adminContext.close()
  } finally {
    await userContext?.close()
    await examples?.stop()
    await Promise.all(onboarded.map((account) => account.context.close()))
  }
}

const teardown = await setup()
let browser
try {
  browser = await webkit.launch({ headless: true })
  await assertAdmission(browser)
  await assertEnterpriseOnboarding(browser)
  await assertDeveloperOnboarding(browser)
  await runProductionGovernanceScenario(browser, 'webkit-1.61')
  await runCrossSiteProtocolScenario(browser, 'webkit-1.61')
  await assertLoginAndSso(browser)
  await assertLocalAndCentralLogout(browser)
  await assertBackchannelLogout(browser)
  log('WebKit (1.61 driver): enterprise SSO scenarios passed')
} finally {
  log('WebKit: closing browser')
  await browser?.close()
  log('WebKit: browser closed')
  log('WebKit: closing fixture')
  await teardown()
  log('WebKit: fixture closed')
}
