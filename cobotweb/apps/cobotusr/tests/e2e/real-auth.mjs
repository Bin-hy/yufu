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
const pathname = (url) => new URL(url).pathname
if (process.env.COBOTUSR_AUTH_TEST_MODE === 'secure-cookie-diagnostic') {
    const browser = await webkit.launch({ headless: true })
    try {
        const context = await browser.newContext({ baseURL: root.origin })
        const page = await context.newPage()
        await page.goto('/login')
        await page.getByLabel('用户名', { exact: true }).fill('isolated-nonexistent')
        await page.getByLabel('密码', { exact: true }).fill('wrong password')
        const challenge = page.waitForResponse((response) => pathname(response.url()) === '/api/v1/identity/csrf')
        const login = page.waitForResponse((response) => pathname(response.url()) === '/api/v1/identity/login')
        await page.getByRole('button', { name: '登录', exact: true }).click()
        assert.equal((await challenge).status(), 200)
        const rejected = await login
        assert.equal(rejected.status(), 403)
        assert.equal((await rejected.json()).code, 'csrf_cookie_requires_https')
        await page.getByRole('alert').filter({ hasText: '--allow-insecure-loopback-cookies' }).waitFor()
        assert.equal(await page.getByLabel('用户名', { exact: true }).inputValue(), 'isolated-nonexistent')
        assert.ok(!(await context.cookies()).some((cookie) => cookie.name.startsWith('__Host-cobotusr-')))
        log('WebKit: default Secure policy, GET /identity/csrf 200 then login 403, actionable UI message passed')
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
        page.on('response', (response) => events.push([pathname(response.url()), response.status()]))
        page.on('request', (request) => {
            if (pathname(request.url()).startsWith('/api/v1/')) requests.push(request)
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

        await page.getByLabel('用户名', { exact: true }).fill('isolated-test-admin')
        await page.getByLabel('密码', { exact: true }).fill('isolated test password')
        const challengeResponse = page.waitForResponse((response) => pathname(response.url()) === '/api/v1/identity/csrf')
        const loginResponse = page.waitForResponse((response) => pathname(response.url()) === '/api/v1/identity/login')
        await page.getByRole('button', { name: '登录', exact: true }).click()
        const challenge = await challengeResponse
        assert.equal(challenge.status(), 200, `${name}: csrf challenge`)
        const logged = await loginResponse
        assert.equal(logged.status(), 200, `${name}: login`)
        const loginBody = await logged.json()
        assert.ok(loginBody.csrf_token, `${name}: center CSRF proof`)
        await page.waitForURL(/\/account$/)
        const loginRequest = requests.find((request) => pathname(request.url()) === '/api/v1/identity/login')
        assert.equal((await loginRequest.allHeaders())['x-csrf-token'], (await challenge.json()).csrf_token, `${name}: login CSRF binding`)
        const sessionCookies = await context.cookies()
        const ssoCookie = sessionCookies.find((cookie) => cookie.name === 'cobotusr-dev-sso')
        assert.ok(ssoCookie?.httpOnly && !ssoCookie.secure, `${name}: HttpOnly center session cookie`)
        assert.ok(sessionCookies.some((cookie) => cookie.name === 'cobotusr-dev-center-csrf' && !cookie.httpOnly && !cookie.secure), `${name}: center CSRF cookie`)
        log(`${name}: center login cookies=${JSON.stringify(sessionCookies.map((cookie) => [cookie.name, cookie.value.length]))}`)

        const restored = page.waitForResponse((response) => pathname(response.url()) === '/api/v1/account/session' && response.status() === 200)
        await page.reload()
        assert.equal((await restored).status(), 200, `${name}: session restore`)
        await page.getByRole('button', { name: '用户菜单' }).waitFor()

        const logoutResponse = page.waitForResponse((response) => pathname(response.url()) === '/api/v1/identity/logout')
        await page.getByRole('button', { name: '用户菜单' }).click()
        await page.getByRole('menuitem', { name: '退出登录' }).click()
        const loggedOut = await logoutResponse
        assert.equal(loggedOut.status(), 200, `${name}: logout`)
        assert.equal((await loggedOut.request().allHeaders())['x-csrf-token'], loginBody.csrf_token, `${name}: logout CSRF binding`)
        await page.waitForURL(/\/login$/)
        const remaining = await context.cookies()
        assert.ok(!remaining.some((cookie) => ['cobotusr-dev-sso', 'cobotusr-dev-center-csrf'].includes(cookie.name)), `${name}: center cookies cleared`)
        assert.ok(!(await page.evaluate(() => JSON.stringify(globalThis.localStorage))).includes(loginBody.csrf_token))
        assert.deepEqual(pageErrors, [])
        log(`${name}: real registration, center UI login, session restore, UI logout and browser cookies passed`)
        await context.close()
    } catch (error) {
        log(`${name} response diagnostics: ${JSON.stringify(events.filter(([path]) => !path.startsWith('/assets/')))}`)
        throw error
    } finally { await browser.close() }
}
}
