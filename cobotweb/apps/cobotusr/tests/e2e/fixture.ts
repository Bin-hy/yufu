import { test as base, expect, chromium, firefox, webkit, type Page, type Browser } from '@playwright/test'
import { webkit as compatibleWebkit } from 'playwright-webkit'
import { release } from 'node:os'
// Playwright 1.65 sends PushAPIEnabled to macOS 14's frozen WebKit build, which rejects it.
// Keep the same tests and fixture; use the matching 1.61 protocol on Darwin 23 only.
export const needsCompatibleWebkit = process.platform === 'darwin' && release().split('.')[0] === '23'
export async function keyboardTab(page: Page, browserName: string) {
    // Option+Tab traverses links and buttons with macOS WebKit's default keyboard setting.
    await page.keyboard.press(process.platform === 'darwin' && browserName === 'webkit' ? 'Alt+Tab' : 'Tab')
}
export const stamp = '2026-11-08 09:30:00'
export async function mockBusiness(page: Page, authenticated = true) {
    const calls: { path: string; method: string; body: Record<string, unknown>; query: string }[] = [],
        unhandled: string[] = []
    const pageErrors: string[] = []
    page.on('pageerror', (error) => pageErrors.push(error.message))
    const identity = { id: 1, username: 'tester', nickname: '测试管理员' }
    const consent = { already_consented: false }
    const statuses = new Map<string, { status: number; remaining: number }>()
    const lostResponses = new Map<string, number>(),
        lostBeforeCommit = new Map<string, number>()
    const heldReads = new Map<string, { response: unknown; release: Promise<void>; completed: boolean }>()
    const roles = Array.from({ length: 105 }, (_, i) => ({
        id: i + 1,
        name: i ? '角色 ' + (i + 1) : '管理员',
        create_time: stamp,
        update_time: stamp,
    }))
    const users = Array.from({ length: 30 }, (_, i) => ({
        id: i + 1,
        username: 'user' + (i + 1),
        nickname: '用户 ' + (i + 1),
        role_id: 1,
        role: roles[0],
        create_time: stamp,
        update_time: stamp,
    }))
    const authorities = Array.from({ length: 4 }, (_, i) => ({
        id: i + 1,
        name: '接口权限 ' + (i + 1),
        authority_uri: '/service/' + (i + 1),
        create_time: stamp,
        update_time: stamp,
    }))
    const permissions = Array.from({ length: 4 }, (_, i) => ({
        id: i + 1,
        name: '资源权限 ' + (i + 1),
        application_id: 1,
        action: 'read',
        resource_type: 'document',
        resource_pattern: 'document:*',
        create_time: stamp,
        update_time: stamp,
    }))
    const applications = [
        {
            id: 1,
            name: '协作控制台',
            app_key: 'client-console',
            description: '平台接入与协作管理',
            oidc_client_id: 1,
            create_time: stamp,
            update_time: stamp,
        },
    ]
    const clients = [
        {
            id: 1,
            client_id: 'client-console',
            name: '协作控制台',
            redirect_uris: 'https://example.test/callback',
            allowed_scopes: 'openid,profile',
            grant_types: 'authorization_code,refresh_token',
            public_client: false,
            create_time: stamp,
            update_time: stamp,
        },
    ]
    const authorized = [
        {
            consent_id: 40,
            client_id: 'client-docs',
            client_name: '文档协作应用',
            scope: 'openid profile',
            authorized_at: stamp,
        },
    ]
    const assigned = new Set([1, 2]),
        assignedPermissions = new Set([1, 2])
    const overrides: {
            user_vo: (typeof users)[number]
            authority_vo: (typeof authorities)[number]
            allow: boolean
            create_time: string
            update_time: string
        }[] = [],
        permOverrides: { user_id: number; permission_id: number; allow: boolean }[] = []
    const delays = new Map<string, number>(),
        failures = new Map<string, number>(),
        message = '模拟服务暂不可用'
    if (authenticated)
        await page.context().addCookies([
            {
                name: 'cobotusr-e2e-token-key',
                value: 'TEST_TOKEN',
                url: 'http://127.0.0.1:5188',
            },
        ])
    await page.route('**/*', async (route) => {
        const request = route.request()
        if (request.resourceType() === 'document' && request.url().includes('/mock-callback')) {
            await route.fulfill({
                contentType: 'text/html',
                body: '<main><h1>授权结果已返回</h1></main>',
            })
            return
        }
        if (!['xhr', 'fetch'].includes(request.resourceType())) {
            await route.continue()
            return
        }
        const url = new URL(request.url()),
            path = url.pathname.replace(/^\/api\/v1/, '')
        const method = request.method(),
            body = request.postDataJSON() ?? {}
        calls.push({ path, method, body, query: url.search })
        const failureKey = method + ' ' + path,
            delay = delays.get(failureKey) ?? delays.get(path + url.search) ?? 0
        const held = heldReads.get(failureKey)
        if (held) {
            heldReads.delete(failureKey)
            await held.release
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify(held.response),
            })
            held.completed = true
            return
        }
        if (delay) await new Promise((resolve) => setTimeout(resolve, delay))
        const status = statuses.get(failureKey + url.search) ?? statuses.get(failureKey)
        if (status && status.remaining > 0) {
            status.remaining--
            await route.fulfill({ status: status.status, body: message })
            return
        }
        if ((lostBeforeCommit.get(failureKey) ?? 0) > 0) {
            lostBeforeCommit.set(failureKey, lostBeforeCommit.get(failureKey)! - 1)
            await route.abort('failed')
            return
        }
        if ((failures.get(failureKey) ?? 0) > 0) {
            failures.set(failureKey, failures.get(failureKey)! - 1)
            await route.fulfill({ status: 503, body: message })
            return
        }
        const pageData = <T>(rows: T[]) => {
            const pn = Number(url.searchParams.get('page_number') ?? 1),
                size = Number(url.searchParams.get('page_size') ?? 10)
            return {
                page_number: pn,
                page_size: size,
                total_pages: Math.max(1, Math.ceil(rows.length / size)),
                data_list: rows.slice((pn - 1) * size, pn * size),
            }
        }
        let data: unknown
        if (path === '/user' && method === 'GET') data = { ...users[0], ...identity }
        else if (path === '/user' && method === 'PATCH') data = {}
        else if (path === '/user' && method === 'POST') {
            const user = {
                ...users[0]!,
                ...body,
                id: users.length + 1,
                role_id: Number(body.role_id),
                role: roles.find((role) => role.id === body.role_id),
                username: String(body.username),
                nickname: String(body.nickname),
            }
            users.push(user)
            data = user
        } else if (path === '/users') data = pageData(users)
        else if (path === '/user/role') {
            const user = users.find((user) => user.id === body.user_id)!
            user.role_id = Number(body.new_role_id)
            user.role = roles.find((role) => role.id === user.role_id)
            data = user
        } else if (/^\/user\/\d+$/.test(path)) {
            users.splice(
                users.findIndex((user) => user.id === Number(path.split('/').at(-1))),
                1,
            )
            data = 'deleted'
        } else if (path === '/role/modifiable') data = pageData(roles)
        else if (path === '/role' && method === 'GET') data = pageData(roles)
        else if (path === '/role' && method === 'POST') {
            data = { ...roles[0], ...body, id: roles.length + 1 }
            roles.push(data as (typeof roles)[number])
        } else if (path === '/role' && method === 'PATCH') {
            const role = roles.find((role) => role.id === body.id)!
            role.name = String(body.name)
            data = role
        } else if (path === '/authority' && method === 'GET') data = pageData(authorities)
        else if (path === '/authority') {
            data = { ...body, id: authorities.length + 1, create_time: stamp, update_time: stamp }
            authorities.push(data as (typeof authorities)[number])
        } else if (/^\/role_authority\/\d+$/.test(path))
            data = pageData(authorities.filter((item) => assigned.has(item.id)))
        else if (path === '/role_authority/batch') {
            for (const id of body.authority_ids as number[]) {
                if (method === 'POST') assigned.add(id)
                else assigned.delete(id)
            }
            data = 'saved'
        } else if (path === '/permission' && method === 'GET')
            data = pageData(
                permissions.filter(
                    (row) =>
                        !url.searchParams.has('application_id') ||
                        row.application_id === Number(url.searchParams.get('application_id')),
                ),
            )
        else if (path === '/permission' && method === 'POST') {
            data = { ...body, id: permissions.length + 1, create_time: stamp, update_time: stamp }
            permissions.push(data as (typeof permissions)[number])
        } else if (path === '/permission' && method === 'PATCH') {
            const row = permissions.find((row) => row.id === body.id)!
            Object.assign(row, body)
            data = row
        } else if (path === '/permission' && method === 'DELETE') {
            permissions.splice(
                permissions.findIndex((row) => row.id === body.id),
                1,
            )
            data = 'deleted'
        } else if (/^\/role_permission\/\d+$/.test(path))
            data = pageData(permissions.filter((item) => assignedPermissions.has(item.id)))
        else if (path === '/role_permission/batch') {
            for (const id of body.permission_ids as number[]) {
                if (method === 'POST') assignedPermissions.add(id)
                else assignedPermissions.delete(id)
            }
            data = 'saved'
        } else if (path === '/application' && method === 'GET') data = pageData(applications)
        else if (path === '/application' && method === 'POST') {
            data = { ...body, id: applications.length + 1, create_time: stamp, update_time: stamp }
            applications.push(data as (typeof applications)[number])
        } else if (path === '/application' && method === 'PATCH') {
            const app = applications.find((app) => app.id === body.id)!
            Object.assign(app, body)
            data = app
        } else if (path === '/application' && method === 'DELETE') {
            applications.splice(
                applications.findIndex((app) => app.id === body.id),
                1,
            )
            data = 'deleted'
        } else if (path === '/oidc/clients' && method === 'GET') data = clients
        else if (path === '/oidc/clients' && method === 'POST') {
            data = {
                ...body,
                id: clients.length + 1,
                client_id: 'new-client-' + (clients.length + 1),
                client_secret: body.public_client ? null : 'TEST_SECRET_MEMORY_ONLY',
                redirect_uris: (body.redirect_uris as string[]).join(','),
                grant_types: (body.grant_types as string[]).join(','),
                allowed_scopes: (body.allowed_scopes as string[]).join(','),
                create_time: stamp,
                update_time: stamp,
            }
            clients.push(data as (typeof clients)[number])
        } else if (/^\/oidc\/client\/\d+\/secret$/.test(path)) data = { client_secret: 'TEST_REGENERATED_SECRET' }
        else if (/^\/oidc\/client\/\d+$/.test(path)) {
            const index = clients.findIndex((client) => client.id === Number(path.split('/').at(-1)))
            if (method === 'DELETE') {
                clients.splice(index, 1)
                data = {}
            } else {
                const client = clients[index]!
                for (const [key, value] of Object.entries(body))
                    Object.assign(client, { [key]: Array.isArray(value) ? value.join(',') : value })
                data = client
            }
        } else if (path === '/oidc/scope')
            data = [
                { id: 1, name: 'openid', description: '身份验证' },
                { id: 2, name: 'custom_scope', description: '自定义权限' },
            ]
        else if (path === '/oidc/authorized-apps') data = authorized
        else if (path === '/oidc/authorized-apps/40') {
            authorized.splice(0, 1)
            data = {}
        } else if (path === '/oidc/authorize/client-info')
            data = {
                client_name: '协作控制台',
                client_id: 'client-console',
                scopes: [{ name: 'openid', description: '读取账户身份' }],
                already_consented: consent.already_consented,
            }
        else if (path === '/oidc/authorize/consent')
            data = {
                redirect_url:
                    'http://127.0.0.1:5188/mock-callback?' +
                    (body.approved ? 'code=test_code' : 'error=access_denied') +
                    '&state=' +
                    String(body.state),
            }
        else if (path === '/user_authority_override' && method === 'GET') data = pageData(overrides)
        else if (path === '/user_authority_override') {
            const index = overrides.findIndex(
                (row) => row.user_vo.id === body.user_id && row.authority_vo.id === body.authority_id,
            )
            if (method === 'DELETE') {
                if (index >= 0) overrides.splice(index, 1)
            } else {
                const row = {
                    user_vo: users.find((user) => user.id === body.user_id)!,
                    authority_vo: authorities.find((item) => item.id === body.authority_id)!,
                    allow: !!body.allow,
                    create_time: stamp,
                    update_time: stamp,
                }
                if (index >= 0) overrides[index] = row
                else overrides.push(row)
            }
            data = {}
        } else if (path === '/user_permission_override' && method === 'GET') data = pageData(permOverrides)
        else if (path === '/user_permission_override') {
            const index = permOverrides.findIndex(
                (row) => row.user_id === body.user_id && row.permission_id === body.permission_id,
            )
            if (method === 'DELETE') {
                if (index >= 0) permOverrides.splice(index, 1)
            } else {
                const row = {
                    user_id: Number(body.user_id),
                    permission_id: Number(body.permission_id),
                    allow: !!body.allow,
                }
                if (index >= 0) permOverrides[index] = row
                else permOverrides.push(row)
            }
            data = {}
        } else if (/^\/recordings/.test(path))
            data = pageData(
                Array.from({ length: 30 }, (_, i) => ({
                    id: i + 1,
                    name: '读取账户',
                    method: 'GET',
                    authority_uri: '/user',
                    user_id: Number(path.split('/')[2] ?? 1),
                    user_vo: users[0],
                    create_time: stamp,
                })),
            )
        else if (path === '/login') {
            identity.id = body.username === 'accountB' ? 2 : 1
            identity.username = String(body.username)
            data = {
                username: String(body.username),
                nickname: '测试管理员',
                access_token: 'TEST_LOGIN_TOKEN',
            }
        } else if (['/register', '/kickout', '/logout'].includes(path)) data = {}
        else if (path === '/refresh') data = { access_token: 'TEST_REFRESH_TOKEN' }
        else {
            unhandled.push(method + ' ' + url.href)
            await route.fulfill({ status: 501, body: 'Missing mock: ' + path })
            return
        }
        if ((lostResponses.get(failureKey) ?? 0) > 0) {
            lostResponses.set(failureKey, lostResponses.get(failureKey)! - 1)
            await route.abort('failed')
            return
        }
        await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify(data),
        })
    })
    return {
        identity,
        consent,
        statuses,
        lostResponses,
        lostBeforeCommit,
        pageErrors,
        heldReads,
        calls,
        unhandled,
        delays,
        failures,
        users,
        roles,
        authorities,
        permissions,
        assigned,
        assignedPermissions,
        applications,
        clients,
        authorized,
        message,
    }
}
export const test = base.extend<{ mock: Awaited<ReturnType<typeof mockBusiness>> }>({
    browser: [
        async ({ browserName, launchOptions, headless }, use) => {
            const type =
                browserName === 'webkit' && needsCompatibleWebkit
                    ? compatibleWebkit
                    : { chromium, firefox, webkit }[browserName]
            const browser = await type.launch({ headless, ...launchOptions })
            // Browser protocol is owned by its matching driver. Public fixture APIs are shared.
            await use(browser as unknown as Browser)
            await browser.close()
        },
        { scope: 'worker' },
    ],
    context: async (
        {
            context,
            browser,
            browserName,
            contextOptions,
            baseURL,
            viewport,
            userAgent,
            deviceScaleFactor,
            isMobile,
            hasTouch,
            colorScheme,
            reducedMotion,
            locale,
            ignoreHTTPSErrors,
        },
        use,
        info,
    ) => {
        if (browserName !== 'webkit' || !needsCompatibleWebkit) {
            await use(context)
            return
        }
        // The newer runner's implicit context defaults belong to its own driver.
        // Pass public options explicitly when using the frozen-build driver.
        const compatible = await browser.newContext({
            ...contextOptions,
            baseURL,
            viewport,
            userAgent,
            deviceScaleFactor,
            isMobile,
            hasTouch,
            colorScheme,
            reducedMotion,
            locale,
            ignoreHTTPSErrors,
        })
        await compatible.tracing.start({ screenshots: true, snapshots: true, sources: true })
        try {
            await use(compatible)
        } finally {
            if (info.status !== info.expectedStatus) {
                const path = info.outputPath('compatible-webkit-trace.zip')
                await compatible.tracing.stop({ path })
                await info.attach('compatible-webkit-trace', {
                    path,
                    contentType: 'application/zip',
                })
            } else await compatible.tracing.stop()
            await compatible.close()
        }
    },
    mock: [
        async ({ page }, use) => {
            const mock = await mockBusiness(page)
            await use(mock)
            expect(mock.unhandled).toEqual([])
            expect(mock.pageErrors).toEqual([])
        },
        { auto: true },
    ],
})
export { expect }
