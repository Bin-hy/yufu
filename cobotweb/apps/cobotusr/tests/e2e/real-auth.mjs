// Invoked only by the Rust test that owns an isolated PostgreSQL / HTTP / NTP fixture.
import assert from 'node:assert/strict'
import process from 'node:process'
import { URL } from 'node:url'
import { log } from 'node:console'
import { chromium, firefox, webkit as currentWebkit } from '@playwright/test'
import { webkit as compatibleWebkit } from 'playwright-webkit'
import { release } from 'node:os'

const root = new URL(process.env.COBOTUSR_AUTH_TEST_URL)
assert.equal(root.protocol, 'http:')
assert.equal(root.hostname, '127.0.0.1')
assert.notEqual(root.port, '4000')
const webkit = process.platform === 'darwin' && Number(release().split('.')[0]) < 24
    ? compatibleWebkit : currentWebkit
if (process.env.COBOTUSR_AUTH_TEST_MODE === 'secure-cookie-diagnostic') {
    const browser = await webkit.launch({ headless: true })
    try {
        const context = await browser.newContext({ baseURL: root.origin })
        const page = await context.newPage()
        await page.goto('/login')
        await page.getByLabel('用户名', { exact: true }).fill('isolated-nonexistent')
        await page.getByLabel('密码', { exact: true }).fill('wrong password')
        const challenge = page.waitForResponse((response) => new URL(response.url()).pathname === '/api/v1/csrf')
        const login = page.waitForResponse((response) => new URL(response.url()).pathname === '/api/v1/login')
        await page.getByRole('button', { name: '登录', exact: true }).click()
        assert.equal((await challenge).status(), 200)
        const rejected = await login
        assert.equal(rejected.status(), 403)
        assert.equal((await rejected.json()).code, 'csrf_cookie_requires_https')
        await page.getByRole('alert').filter({ hasText: '--allow-insecure-loopback-cookies' }).waitFor()
        assert.equal(await page.getByLabel('用户名', { exact: true }).inputValue(), 'isolated-nonexistent')
        assert.ok(!(await context.cookies()).some((cookie) => cookie.name === '__Host-cobotusr-csrf'))
        log('WebKit: default Secure policy, GET /csrf 200 then login 403, actionable UI message passed')
        await context.close()
    } finally { await browser.close() }
    process.exit(0)
}
for (const [engineName, engine] of Object.entries({ chromium, firefox, webkit })) {
for (const [site, baseURL] of Object.entries({ ipv4: root.origin, localhost: `http://localhost:${root.port}` })) {
    const name = `${engineName}/${site}`
    const browser = await engine.launch({ headless: true })
    const events = []
    try {
        const context = await browser.newContext({ baseURL })
        const page = await context.newPage()
        page.setDefaultTimeout(12000)
        const pageErrors = []
        const requests = []
        page.on('pageerror', (error) => pageErrors.push(error.message))
        page.on('response', (response) => events.push([new URL(response.url()).pathname, response.status()]))
        page.on('request', (request) => {
            if (new URL(request.url()).pathname.startsWith('/api/v1/')) requests.push(request)
        })
        await page.goto('/login')
        const username = `isolated-${engineName}-${site}`
        const registration = await page.evaluate(async (username) => {
            const challenge = await globalThis.fetch('/api/v1/csrf', { credentials: 'include' })
            const { csrf_token } = await challenge.json()
            const response = await globalThis.fetch('/api/v1/register', {
                method: 'POST', credentials: 'include',
                headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': csrf_token },
                body: JSON.stringify({ username, nickname: '隔离测试账户', password: 'isolated test password' }),
            })
            return response.status
        }, username)
        assert.equal(registration, 200, `${name}: registration`)
        // Check the default HTTPS policy separately in backend tests; this fixture opts into HTTP loopback.
        const initialCookies = await context.cookies()
        const refreshCookie = initialCookies.find((cookie) => cookie.name === 'refresh_token')
        assert.ok(refreshCookie?.httpOnly && !refreshCookie.secure)
        assert.ok(initialCookies.some((cookie) => cookie.name === 'cobotusr-dev-csrf' && !cookie.secure))

        await page.getByLabel('用户名', { exact: true }).fill('isolated-test-admin')
        await page.getByLabel('密码', { exact: true }).fill('isolated test password')
        const loginResponse = page.waitForResponse((response) => new URL(response.url()).pathname === '/api/v1/login')
        const identityResponse = page.waitForResponse((response) => new URL(response.url()).pathname === '/api/v1/user')
        await page.getByRole('button', { name: '登录', exact: true }).click()
        const logged = await loginResponse
        assert.equal(logged.status(), 200, `${name}: login`)
        const loginBody = await logged.json()
        assert.equal((await identityResponse).status(), 200, `${name}: current identity`)
        await page.waitForURL(/\/dashboard$/)
        log(`${name}: access token length=${loginBody.access_token.length}; browser cookie lengths=${JSON.stringify((await context.cookies()).map((cookie) => [cookie.name, cookie.value.length]))}`)
        const loggedRequest = requests.find((request) => new URL(request.url()).pathname === '/api/v1/login')
        assert.ok((await loggedRequest.allHeaders())['x-csrf-token'])

        // Force an access-token failure only in the browser; the actual backend handles refresh.
        let failed = false
        await page.route('**/api/v1/user', async (route) => {
            if (!failed && route.request().method() === 'GET') {
                failed = true
                await route.fulfill({ status: 401, body: 'isolated expired access' })
            } else await route.continue()
        })
        const responses = Promise.all([
            page.waitForResponse((response) => new URL(response.url()).pathname === '/api/v1/csrf'),
            page.waitForResponse((response) => new URL(response.url()).pathname === '/api/v1/refresh'),
        ])
        await page.reload()
        const [challengeResponse, refreshed] = await responses
        assert.equal(refreshed.status(), 200, `${name}: refresh`)
        const refreshBody = await refreshed.json()
        const refreshRequest = requests.find((request) => new URL(request.url()).pathname === '/api/v1/refresh')
        assert.ok((await refreshRequest.allHeaders())['x-csrf-token'] === (await challengeResponse.json()).csrf_token, `${name}: refresh CSRF binding`)
        assert.ok(loginBody.csrf_token)
        await page.getByRole('button', { name: '用户菜单' }).waitFor()

        const logoutResponse = page.waitForResponse((response) => new URL(response.url()).pathname === '/api/v1/logout')
        await page.getByRole('button', { name: '用户菜单' }).click()
        await page.getByRole('menuitem', { name: '退出登录' }).click()
        const loggedOut = await logoutResponse
        assert.equal(loggedOut.status(), 200, `${name}: logout`)
        assert.ok((await loggedOut.request().allHeaders())['x-csrf-token'] === refreshBody.csrf_token, `${name}: logout CSRF binding`)
        await page.waitForURL(/\/login$/)
        assert.ok(!(await context.cookies()).some((cookie) => ['refresh_token', 'cobotusr-dev-csrf'].includes(cookie.name)))
        assert.ok(!(await page.evaluate(() => JSON.stringify(globalThis.localStorage))).includes(refreshBody.csrf_token))
        assert.deepEqual(pageErrors, [])
        log(`${name}: real registration, UI login, real refresh, UI logout and browser cookies passed`)
        await context.close()
    } catch (error) {
        log(`${name} response diagnostics: ${JSON.stringify(events.filter(([path]) => !path.startsWith('/assets/')))}`)
        throw error
    } finally { await browser.close() }
}
}
