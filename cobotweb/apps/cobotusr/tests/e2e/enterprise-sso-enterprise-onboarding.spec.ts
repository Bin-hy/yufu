import { randomUUID } from 'node:crypto'

import {
  expect,
  test,
  type APIRequestContext,
  type APIResponse,
  type BrowserContext,
  type Page,
} from '@playwright/test'

import { config, type SsoConfig } from './enterprise-sso-fixture'

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

interface AccountRow {
  username: string
  person_kind: string
  developer_enabled: boolean
  enabled: boolean
}

interface GroupRow {
  group_id: string
  members: number[]
}

interface AuthorityRow {
  id: number
  name: string
  authority_uri: string
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
    { name, description: 'C68 enterprise onboarding fixture' },
    { 'idempotency-key': randomUUID() },
  )
  const receipt = await json<{
    summary: { group_id: string; revision: number }
  }>(response)
  return {
    groupId: receipt.summary.group_id,
    revision: receipt.summary.revision,
  }
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

async function accounts(
  request: APIRequestContext,
  cfg: SsoConfig,
): Promise<AccountRow[]> {
  const response = await request.get(cfg.opOrigin + '/api/v1/admin/accounts?limit=100')
  return (await json<{ items: AccountRow[] }>(response)).items
}

async function grantEnterpriseAdministrator(
  request: APIRequestContext,
  cfg: SsoConfig,
  username: string,
  password: string,
  userId: number,
): Promise<number> {
  const login = await request.post(cfg.opOrigin + '/api/v1/login', {
    headers: {
      origin: cfg.opOrigin,
      'x-csrf-token': await csrf(request, cfg, '/api/v1/csrf'),
    },
    data: { username, password },
  })
  const session = await json<{ access_token: string; csrf_token: string }>(login)
  const token = session.access_token
  const listed = await request.get(
    cfg.opOrigin + '/api/v1/authority?page_number=1&page_size=500',
    { headers: { authorization: 'Bearer ' + token } },
  )
  const authorities = (await json<{ data_list: AuthorityRow[] }>(listed)).data_list.filter(
    (authority) =>
      String(authority.name).startsWith('identity.uri:') &&
      String(authority.authority_uri).startsWith('/admin/'),
  )
  expect(authorities.length).toBeGreaterThan(0)
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

async function navigationGroups(page: Page): Promise<string[]> {
  await expect(page.locator('.nav-group-title').first()).toBeVisible()
  return (await page.locator('.nav-group-title').allTextContents()).map((title) => title.trim())
}

async function expectStatus(
  request: APIRequestContext,
  url: string,
  status: number,
): Promise<void> {
  const response = await request.get(url)
  expect(response.status(), await response.text()).toBe(status)
}

test('a real administrator establishes the enterprise directory and its access boundaries', async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(180_000)
  const cfg = config()
  const suffix = `${testInfo.project.name}-${randomUUID().slice(0, 8)}`
  const password = `Enterprise-${suffix}-Passw0rd!`
  const secondAdmin: Account = {
    username: `c68-admin-${suffix}`,
    password,
    nickname: 'Second Administrator',
    kind: 'employee',
    developer: false,
  }
  const employee: Account = {
    username: `c68-employee-${suffix}`,
    password,
    nickname: 'Employee',
    kind: 'employee',
    developer: false,
  }
  const developers: Account[] = [
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
  const collaborator: Account = {
    username: `c68-partner-${suffix}`,
    password,
    nickname: 'Partner',
    kind: 'collaborator',
    developer: false,
  }
  const onboarded: OnboardedAccount[] = []

  try {
    await centerLogin(page, cfg, cfg.username, cfg.password)
    const adminRequest = page.request
    const all = [secondAdmin, employee, ...developers, collaborator]
    for (const account of all) {
      const secret = await issueInvitation(adminRequest, cfg, account)
      const context = await browser.newContext({ ignoreHTTPSErrors: true })
      const accountPage = await context.newPage()
      accountPage.setDefaultTimeout(30_000)
      const userId = await acceptInvitation(context, cfg, account, secret)
      onboarded.push({ ...account, context, page: accountPage, userId })
    }

    const directory = await accounts(adminRequest, cfg)
    for (const account of onboarded) {
      const row = directory.find((candidate) => candidate.username === account.username)
      expect(row, account.username).toBeTruthy()
      expect(row!.person_kind).toBe(account.kind)
      expect(row!.developer_enabled).toBe(account.developer)
      expect(row!.enabled).toBe(true)
    }

    const groupName = `Employees ${suffix}`
    const group = await createGroup(adminRequest, cfg, groupName)
    let revision = group.revision
    const employeeMembers = onboarded.filter((account) => account.kind === 'employee')
    for (const member of employeeMembers) {
      revision = await addGroupMember(adminRequest, cfg, group.groupId, member.userId, revision)
    }

    const groups = await json<{ items: GroupRow[] }>(
      await adminRequest.get(cfg.opOrigin + '/api/v1/admin/groups?limit=50'),
    )
    const employeeGroup = groups.items.find(
      (candidate) => candidate.group_id === group.groupId,
    )
    expect(employeeGroup, groupName).toBeTruthy()
    expect(employeeGroup!.members).toEqual(
      expect.arrayContaining(employeeMembers.map((member) => member.userId)),
    )
    const partner = onboarded.find((account) => account.username === collaborator.username)!
    expect(employeeGroup!.members).not.toContain(partner.userId)

    const second = onboarded.find((account) => account.username === secondAdmin.username)!
    const grantedAuthorities = await grantEnterpriseAdministrator(
      adminRequest,
      cfg,
      cfg.username,
      cfg.password,
      second.userId,
    )

    for (const account of onboarded) {
      await centerLogin(account.page, cfg, account.username, account.password)
      await expectStatus(account.context.request, cfg.opOrigin + '/api/v1/account/apps', 200)
      const apps = await json<{ items: unknown[] }>(
        await account.context.request.get(cfg.opOrigin + '/api/v1/account/apps'),
      )
      expect(apps.items).toEqual([])
    }

    const employeeAccount = onboarded.find((account) => account.username === employee.username)!
    const developerAccounts = onboarded.filter((account) => account.developer)
    const secondAdministrator = onboarded.find(
      (account) => account.username === secondAdmin.username,
    )!

    for (const account of [employeeAccount, partner]) {
      expect(await navigationGroups(account.page)).toEqual(
        expect.arrayContaining(['账户中心', '快捷工作流']),
      )
      expect(await navigationGroups(account.page)).not.toContain('开发者控制台')
      expect(await navigationGroups(account.page)).not.toContain('企业管理')
      await expectStatus(account.context.request, cfg.opOrigin + '/api/v1/developer/apps', 403)
      await expectStatus(account.context.request, cfg.opOrigin + '/api/v1/admin/apps', 403)
    }

    for (const account of developerAccounts) {
      expect(await navigationGroups(account.page)).toEqual(
        expect.arrayContaining(['账户中心', '开发者控制台']),
      )
      expect(await navigationGroups(account.page)).not.toContain('企业管理')
      await expectStatus(account.context.request, cfg.opOrigin + '/api/v1/developer/apps', 200)
      await expectStatus(account.context.request, cfg.opOrigin + '/api/v1/admin/apps', 403)
    }

    expect(await navigationGroups(secondAdministrator.page)).toEqual(
      expect.arrayContaining(['账户中心', '企业管理']),
    )
    expect(await navigationGroups(secondAdministrator.page)).not.toContain('开发者控制台')
    await expectStatus(
      secondAdministrator.context.request,
      cfg.opOrigin + '/api/v1/admin/accounts',
      200,
    )
    await expectStatus(
      secondAdministrator.context.request,
      cfg.opOrigin + '/api/v1/admin/groups',
      200,
    )
    // Administrators may read the developer list to inspect apps they own, but
    // the console stays hidden and they own no applications here.
    const adminDeveloperApps = await json<{ items: unknown[] }>(
      await secondAdministrator.context.request.get(cfg.opOrigin + '/api/v1/developer/apps'),
    )
    expect(adminDeveloperApps.items).toEqual([])
    await expectStatus(
      secondAdministrator.context.request,
      cfg.opOrigin + '/api/v1/admin/apps',
      200,
    )

    const evidence = {
      engine: testInfo.project.name,
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
    }
    testInfo.annotations.push({
      type: 'C68 desensitized evidence',
      description: JSON.stringify(evidence),
    })
    console.log('[C68] ' + JSON.stringify(evidence))
  } finally {
    await Promise.all(onboarded.map((account) => account.context.close()))
  }
})
