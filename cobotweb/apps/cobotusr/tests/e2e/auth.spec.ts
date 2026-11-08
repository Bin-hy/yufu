import { test, expect } from './fixture'
import type { Page } from '@playwright/test'
async function login(page: Page, username = 'tester') {
    await page.getByLabel('用户名', { exact: true }).fill(username)
    await page.getByLabel('密码', { exact: true }).fill('password')
    await page.getByRole('button', { name: '登录', exact: true }).click()
}
const flow = {
    client_id: 'client-console',
    redirect_uri: 'https://example.test/callback',
    response_type: 'code',
    scope: 'openid profile',
    state: 'TEST_OIDC_STATE',
    nonce: 'TEST_NONCE',
    code_challenge: 'TEST_PKCE',
    code_challenge_method: 'S256',
}
const query = new URLSearchParams(flow).toString()
test('普通登录失败保留用户名，成功恢复站内返回目标', async ({ page, mock }) => {
    await page.context().clearCookies()
    mock.failures.set('POST /login', 1)
    await page.goto('/login?redirect=' + encodeURIComponent('/users?page_number=3'))
    await login(page)
    await expect(page.getByRole('alert')).toContainText(mock.message)
    await expect(page.getByLabel('用户名', { exact: true })).toHaveValue('tester')
    await page.getByRole('button', { name: '登录', exact: true }).click()
    await expect(page).toHaveURL(/\/users\?page_number=3$/)
    await expect(page.getByText('user21 · #21', { exact: true })).toBeVisible()
})
for (const target of ['https://outside.test/path', '//outside.test', '/oidc/consent?client_id=x'])
    test('登录忽略不适合返回的目标 ' + target, async ({ page }) => {
        await page.context().clearCookies()
        await page.goto('/login?redirect=' + encodeURIComponent(target))
        await login(page)
        await expect(page).toHaveURL(/\/dashboard$/)
    })
test('注册校验、服务失败保留输入，成功后可直接登录', async ({ page, mock }) => {
    await page.context().clearCookies()
    mock.failures.set('POST /register', 1)
    await page.goto('/register')
    await page.getByLabel('用户名', { exact: true }).fill('new-account')
    await page.getByLabel('昵称', { exact: true }).fill('新账户')
    await page.getByLabel('密码', { exact: true }).fill('new-password')
    await page.getByLabel('确认密码', { exact: true }).fill('new-password')
    await page.getByRole('button', { name: '创建账户', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText('请先同意')
    await page.getByRole('checkbox').check()
    await page.getByRole('button', { name: '创建账户', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText(mock.message)
    await expect(page.getByLabel('用户名', { exact: true })).toHaveValue('new-account')
    await page.getByRole('button', { name: '创建账户', exact: true }).click()
    await expect(page).toHaveURL(/\/login$/)
    expect(mock.calls.filter((call) => call.path === '/register')).toHaveLength(2)
})
for (const approved of [true, false])
    test(
        'OIDC 登录与' + (approved ? '同意' : '拒绝') + '保留授权参数且按响应回跳',
        async ({ page, mock }) => {
            await page.context().clearCookies()
            await page.goto('/oidc/consent?' + query)
            await expect(page).toHaveURL(/\/oidc\/login\?/)
            for (const [key, value] of Object.entries(flow))
                expect(new URL(page.url()).searchParams.get(key)).toBe(value)
            await login(page)
            await expect(page.getByRole('heading', { name: '授权 协作控制台' })).toBeVisible()
            mock.delays.set('POST /oidc/authorize/consent', 450)
            await page
                .getByRole('button', { name: approved ? '同意授权' : '拒绝授权', exact: true })
                .click()
            await expect(page.getByRole('button', { name: '拒绝授权', exact: true })).toBeDisabled()
            await expect(page).toHaveURL(
                approved
                    ? /mock-callback\?code=test_code&state=TEST_OIDC_STATE/
                    : /mock-callback\?error=access_denied&state=TEST_OIDC_STATE/,
            )
            const calls = mock.calls.filter((call) => call.path === '/oidc/authorize/consent')
            expect(calls).toHaveLength(1)
            expect(calls[0]?.body).toMatchObject({ ...flow, approved })
            expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain(
                'TEST_OIDC_STATE',
            )
        },
    )
test('OIDC 同意失败留在原页可重试，已有同意自动返回', async ({ page, mock }) => {
    mock.failures.set('POST /oidc/authorize/consent', 1)
    await page.goto('/oidc/consent?' + query)
    await page.getByRole('button', { name: '同意授权', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText(mock.message)
    await expect(page).toHaveURL(/\/oidc\/consent\?/)
    await page.getByRole('button', { name: '同意授权', exact: true }).click()
    await expect(page).toHaveURL(/mock-callback\?code=/)
    mock.consent.already_consented = true
    await page.goto('/oidc/consent?' + query)
    await expect(page).toHaveURL(/mock-callback\?code=/)
    expect(mock.calls.filter((call) => call.path === '/oidc/authorize/consent')).toHaveLength(3)
})
test('OIDC 刷新认证失败回对应登录入口并保留原参数', async ({ page, mock }) => {
    mock.statuses.set('GET /oidc/authorize/client-info', { status: 401, remaining: 1 })
    mock.statuses.set('POST /refresh', { status: 401, remaining: 1 })
    await page.goto('/oidc/consent?' + query)
    await expect(page).toHaveURL(/\/oidc\/login\?/)
    for (const [key, value] of Object.entries(flow))
        expect(new URL(page.url()).searchParams.get(key)).toBe(value)
    await expect(page.getByRole('heading', { name: '登录以继续授权' })).toBeVisible()
    expect(mock.calls.filter((call) => call.path === '/refresh')).toHaveLength(1)
})
test('旧地址和安全 redirect 兼容，403/404直达有可用返回入口', async ({ page }) => {
    for (const path of ['/oauth', '/oauth/create', '/oauth/detail/1']) {
        await page.goto(path)
        await expect(page).toHaveURL(/\/applications$/)
        await expect(
            page.getByRole('heading', { name: '应用注册', level: 1, exact: true }),
        ).toBeVisible()
        await expect(page.locator('.workspace-tab')).toHaveCount(2)
    }
    await page.goto('/redirect/users?page_number=3')
    await expect(page).toHaveURL(/\/users\?page_number=3$/)
    await expect(page.locator('tbody tr')).toHaveCount(10)
    for (const path of ['/403', '/404', '/does-not-exist']) {
        await page.goto(path)
        await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
        await page.getByRole('link', { name: '返回工作台' }).click()
        await expect(page).toHaveURL(/\/dashboard$/)
        await expect(
            page.getByRole('heading', { name: '工作台', level: 1, exact: true }),
        ).toBeVisible()
    }
})
