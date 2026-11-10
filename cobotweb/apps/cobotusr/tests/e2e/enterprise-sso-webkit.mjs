// WebKit fallback for hosts where the Playwright 1.65 driver and the available
// WebKit build disagree on protocol settings. Mirrors the three Playwright
// specs with the 1.61 driver already used by real-auth.mjs.
import assert from 'node:assert/strict'
import { log } from 'node:console'
import { randomUUID } from 'node:crypto'
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

const teardown = await setup()
let browser
try {
  browser = await webkit.launch({ headless: true })
  await assertAdmission(browser)
  await assertEnterpriseOnboarding(browser)
  await assertLoginAndSso(browser)
  await assertLocalAndCentralLogout(browser)
  await assertBackchannelLogout(browser)
  log('WebKit (1.61 driver): enterprise SSO scenarios passed')
} finally {
  await browser?.close()
  await teardown()
}
