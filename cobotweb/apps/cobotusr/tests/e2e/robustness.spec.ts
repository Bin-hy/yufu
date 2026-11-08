import { test, expect } from './fixture'
import type { Page } from '@playwright/test'

async function nav(page: Page, name: string) {
    await page.locator('.workspace-sidebar').getByRole('link', { name, exact: true }).click()
}

test('权限与应用字段错误定位、关联可读，修正后保留后续服务错误', async ({ page, mock }) => {
    await page.goto('/authority')
    await page.getByRole('button', { name: '新增权限', exact: true }).click()
    await page.getByRole('dialog').getByRole('button', { name: '确认', exact: true }).click()
    await expect(page.getByLabel('权限名称', { exact: true })).toBeFocused()
    await expect(page.getByLabel('权限名称', { exact: true })).toHaveAttribute('aria-describedby', 'catalog-validation')
    await page.getByLabel('权限名称', { exact: true }).fill('校验权限')
    await page.getByRole('dialog').getByRole('button', { name: '确认', exact: true }).click()
    await expect(page.getByLabel('权限标识', { exact: true })).toBeFocused()
    await page.getByLabel('权限标识', { exact: true }).fill('/validated')
    mock.failures.set('POST /authority', 1)
    await page.getByRole('dialog').getByRole('button', { name: '确认', exact: true }).click()
    await expect(page.getByRole('dialog').getByRole('alert')).toContainText(mock.message)
    await expect(page.getByText('请填写权限标识', { exact: true })).toHaveCount(0)
    await page.getByRole('dialog').getByRole('button', { name: '确认', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await nav(page, '应用注册')
    await expect(page.getByRole('heading', { name: '应用注册', exact: true })).toBeVisible()
    await page.getByRole('button', { name: '注册应用', exact: true }).click()
    await page.getByRole('dialog').getByRole('button', { name: '保存应用', exact: true }).click()
    await expect(page.getByLabel('应用名称', { exact: true })).toBeFocused()
    await page.getByLabel('应用名称', { exact: true }).fill('校验应用')
    await page.getByLabel('授权回调地址', { exact: true }).fill('invalid-callback')
    await page.getByRole('dialog').getByRole('button', { name: '保存应用', exact: true }).click()
    await expect(page.getByLabel('授权回调地址', { exact: true })).toBeFocused()
    await expect(page.getByLabel('授权回调地址', { exact: true })).toHaveAttribute(
        'aria-describedby',
        'callback-help application-validation',
    )
    await page.getByLabel('授权回调地址', { exact: true }).fill('https://example.test/validated')
    mock.failures.set('POST /oidc/clients', 1)
    await page.getByRole('dialog').getByRole('button', { name: '保存应用', exact: true }).click()
    await expect(page.getByRole('dialog').getByRole('alert')).toContainText(mock.message)
    await expect(page.getByText('回调地址应为完整的 HTTP 或 HTTPS 地址', { exact: true })).toHaveCount(0)
})

test('用户、权限和应用写成功后刷新失败，已更新或删除的对象不倒退', async ({ page, mock }) => {
    await page.goto('/users')
    const row = page.locator('tbody tr').first()
    await row.getByRole('combobox').selectOption('2')
    mock.failures.set('GET /users', 1)
    await row.getByRole('button', { name: '保存', exact: true }).click()
    await expect(page.locator('.inline-error')).toContainText('刷新失败')
    await expect(row.getByRole('combobox')).toHaveValue('2')
    await expect(row.getByRole('button', { name: '保存', exact: true })).toHaveCount(0)
    mock.failures.set('GET /users', 1)
    await row.getByRole('button', { name: '删除', exact: true }).click()
    await page.getByRole('alertdialog').getByRole('button', { name: '删除用户', exact: true }).click()
    await expect(page.getByRole('alertdialog')).toHaveCount(0)
    await expect(page.getByText('user1 · #1', { exact: true })).toHaveCount(0)
    await nav(page, '资源权限')
    await expect(page.locator('tbody tr')).toHaveCount(4)
    mock.failures.set('GET /permission', 1)
    await page.locator('tbody tr').first().getByRole('button', { name: '删除', exact: true }).click()
    await page.getByRole('alertdialog').getByRole('button', { name: '删除权限', exact: true }).click()
    await expect(page.getByRole('alertdialog')).toHaveCount(0)
    await expect(page.getByText('资源权限 1', { exact: true })).toHaveCount(0)
    await nav(page, '应用注册')
    await expect(page.getByRole('heading', { name: '应用注册', exact: true })).toBeVisible()
    await expect(page.getByRole('button', { name: '删除', exact: true })).toBeVisible()
    mock.failures.set('GET /application', 1)
    await page.getByRole('button', { name: '删除', exact: true }).click()
    await page.getByRole('alertdialog').getByRole('button', { name: '删除应用', exact: true }).click()
    await expect(page.getByRole('alertdialog')).toHaveCount(0)
    await expect(page.getByRole('button', { name: '删除', exact: true })).toHaveCount(0)
    await expect(page.locator('.inline-error')).toContainText('刷新失败')
    expect(mock.calls.filter((call) => call.path === '/user/1' && call.method === 'DELETE')).toHaveLength(1)
    expect(mock.calls.filter((call) => call.path === '/permission' && call.method === 'DELETE')).toHaveLength(1)
    expect(mock.calls.filter((call) => call.path === '/application' && call.method === 'DELETE')).toHaveLength(1)
})

test('刷新恢复当前页滚动，等待异步列表高度就绪', async ({ page, mock }) => {
    await page.setViewportSize({ width: 1280, height: 500 })
    await page.goto('/users?page_number=3&q=user2')
    await expect(page.locator('tbody tr')).toHaveCount(9)
    await page.locator('#workspace-content').evaluate((el) => {
        el.scrollTop = 200
    })
    const top = await page.locator('#workspace-content').evaluate((el) => el.scrollTop)
    await expect
        .poll(() =>
            page.evaluate(
                () =>
                    JSON.parse(localStorage.getItem('cobotusr:workspace:v1:1')!).tabs.find(
                        (tab: { fullPath: string }) => tab.fullPath.startsWith('/users'),
                    ).scrollTop,
            ),
        )
        .toBe(top)
    mock.delays.set('GET /users', 450)
    await page.reload()
    await expect(page.locator('tbody tr')).toHaveCount(9)
    await expect.poll(() => page.locator('#workspace-content').evaluate((el) => el.scrollTop)).toBe(top)
    await expect(page).toHaveURL(/page_number=3&q=user2/)
})

test('撤销成功后的读取失败保留成功事实，不再提供重复撤销', async ({ page, mock }) => {
    await page.goto('/authorized-apps')
    await expect(page.getByRole('button', { name: '撤销', exact: true })).toBeVisible()
    mock.failures.set('GET /oidc/authorized-apps', 1)
    await page.getByRole('button', { name: '撤销', exact: true }).click()
    await page.getByRole('alertdialog').getByRole('button', { name: '撤销授权', exact: true }).click()
    await expect(page.getByRole('alertdialog')).toHaveCount(0)
    await expect(page.getByRole('heading', { name: '尚未授权任何应用' })).toBeVisible()
    await expect(page.getByRole('button', { name: '撤销', exact: true })).toHaveCount(0)
    await expect(page.locator('.inline-error')).toContainText('刷新失败')
    await page.getByRole('button', { name: '重试', exact: true }).click()
    await expect(page.getByText(/刷新失败/)).toHaveCount(0)
    expect(mock.calls.filter((call) => call.method === 'DELETE')).toHaveLength(1)
})

test('个人中心校验定位首错误，后续服务失败不被旧校验遮住', async ({ page, mock }) => {
    await page.goto('/profile')
    await page.getByLabel('新密码', { exact: true }).fill('new-password')
    await page.getByLabel('确认新密码', { exact: true }).fill('different-password')
    await page.getByRole('button', { name: '更新密码', exact: true }).click()
    await expect(page.getByLabel('确认新密码', { exact: true })).toBeFocused()
    await expect(page.getByRole('alert')).toContainText('两次密码不一致')
    await expect(page.getByLabel('确认新密码', { exact: true })).toHaveAttribute(
        'aria-describedby',
        'profile-password-error',
    )
    await page.getByLabel('确认新密码', { exact: true }).fill('new-password')
    mock.failures.set('PATCH /user', 1)
    await page.getByRole('button', { name: '更新密码', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText(mock.message)
    await expect(page.getByLabel('新密码', { exact: true })).toHaveValue('new-password')
})

test('慢旧读取在账号B登录后返回，不恢复A草稿；随后完成OIDC同意', async ({ page, mock }) => {
    await page.goto('/users/create')
    await expect(page.getByLabel('角色', { exact: true })).toHaveValue('1')
    await page.getByRole('textbox', { name: '用户名', exact: true }).fill('ACCOUNT_A_DRAFT')
    let release!: () => void
    const held = {
        response: {
            page_number: 1,
            page_size: 10,
            total_pages: 1,
            data_list: [{ ...mock.users[0], username: 'OLD_ACCOUNT_RESULT' }],
        },
        release: new Promise<void>((resolve) => {
            release = resolve
        }),
        completed: false,
    }
    mock.heldReads.set('GET /users', held)
    mock.statuses.set('GET /role/modifiable', { status: 401, remaining: 1 })
    mock.statuses.set('POST /refresh', { status: 401, remaining: 1 })
    await nav(page, '用户管理')
    await page.getByRole('button', { name: '保留并切换', exact: true }).click()
    await expect(page).toHaveURL(/\/login/)
    expect(mock.calls.some((call) => call.path === '/users')).toBe(true)
    await page.getByLabel('用户名', { exact: true }).fill('accountB')
    await page.getByLabel('密码', { exact: true }).fill('password')
    await page.getByRole('button', { name: '登录', exact: true }).click()
    await expect(page).toHaveURL(/\/dashboard$/)
    release()
    await expect.poll(() => held.completed).toBe(true)
    await expect(page.locator('.workspace-tab')).toHaveCount(1)
    await nav(page, '用户管理')
    await expect(page.locator('tbody tr')).toHaveCount(10)
    await expect(page.getByText('OLD_ACCOUNT_RESULT')).toHaveCount(0)
    expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain('ACCOUNT_A_DRAFT')
    await page.goto(
        '/oidc/consent?client_id=client-console&redirect_uri=https://example.test/callback&state=account-b-state&code_challenge=example&code_challenge_method=S256',
    )
    await page.getByRole('button', { name: '同意授权', exact: true }).click()
    await expect(page).toHaveURL(/mock-callback\?code=test_code&state=account-b-state/)
    expect(mock.identity.id).toBe(2)
})

test('多个实际页面标签在小屏单行横滚，链接与键盘焦点可达', async ({ page }) => {
    await page.goto('/dashboard')
    for (const name of [
        '用户管理',
        '角色管理',
        'API 权限',
        'API 权限分配',
        '资源权限',
        '资源权限分配',
        '应用注册',
        '操作记录',
    ])
        await nav(page, name)
    await page.setViewportSize({ width: 360, height: 800 })
    const strip = page.locator('.tags-scroll')
    const geometry = await strip.evaluate((el) => ({
        client: el.clientWidth,
        scroll: el.scrollWidth,
    }))
    expect(geometry.scroll).toBeGreaterThan(geometry.client)
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(360)
    const last = strip.getByRole('link', { name: '操作记录', exact: true })
    await last.focus()
    await expect(last).toBeFocused()
    await expect(last).toHaveAttribute('href', '/recordings')
    const rect = await last.boundingBox(),
        bounds = await strip.boundingBox()
    expect(rect!.x).toBeGreaterThanOrEqual(bounds!.x - 1)
    expect(rect!.x + rect!.width).toBeLessThanOrEqual(bounds!.x + bounds!.width + 1)
    const heights = await strip
        .locator('.workspace-tab')
        .evaluateAll((nodes) => nodes.map((el) => el.getBoundingClientRect().height))
    expect(new Set(heights).size).toBe(1)
})

test('完整列表流程保留分页，乱序结果正确，修改角色后关闭到邻页', async ({ page, mock }) => {
    await page.goto('/dashboard')
    await nav(page, '用户管理')
    await page.getByRole('button', { name: '第 3 页', exact: true }).click()
    await expect(page.getByText('user21 · #21', { exact: true })).toBeVisible()
    await nav(page, '角色管理')
    await page.locator('.tags-bar').getByRole('link', { name: '用户管理', exact: true }).click()
    await expect(page).toHaveURL(/page_number=3/)
    mock.delays.set('/users?page_number=2&page_size=10', 650)
    await page.getByRole('button', { name: '第 2 页', exact: true }).click()
    await page.getByRole('button', { name: '第 3 页', exact: true }).click()
    await expect(page.getByText('user21 · #21', { exact: true })).toBeVisible()
    const row = page.locator('tbody tr').first()
    await row.getByRole('combobox').selectOption('2')
    await row.getByRole('button', { name: '保存', exact: true }).click()
    await expect(row.getByRole('combobox')).toHaveValue('2')
    await expect(row.getByRole('button', { name: '保存', exact: true })).toHaveCount(0)
    expect(mock.users[20]!.role_id).toBe(2)
    await page.getByRole('button', { name: '关闭用户管理', exact: true }).click()
    await expect(page).toHaveURL(/\/dashboard$/)
    await expect(page.locator('.tags-bar').getByRole('link', { name: '角色管理', exact: true })).toBeVisible()
})

test('完整应用流程分步重试、密钥确认及撤销失败恢复', async ({ page, mock }) => {
    mock.failures.set('POST /application', 1)
    await page.goto('/applications')
    await page.getByRole('button', { name: '注册应用', exact: true }).click()
    await page.getByLabel('应用名称', { exact: true }).fill('完整流程应用')
    await page.getByLabel('授权回调地址', { exact: true }).fill('https://example.test/flow')
    await page.getByRole('dialog').getByRole('button', { name: '保存应用', exact: true }).click()
    await expect(page.getByRole('dialog').getByRole('alert')).toBeVisible()
    await page.getByRole('button', { name: '重试剩余步骤', exact: true }).click()
    await page.getByRole('button', { name: '显示密钥', exact: true }).click()
    await expect(page.getByText('TEST_SECRET_MEMORY_ONLY', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: '已保存，完成', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await nav(page, '已授权应用')
    mock.failures.set('DELETE /oidc/authorized-apps/40', 1)
    await page.getByRole('button', { name: '撤销', exact: true }).click()
    await page.getByRole('alertdialog').getByRole('button', { name: '撤销授权', exact: true }).click()
    await expect(page.getByRole('alertdialog').getByRole('alert')).toBeVisible()
    await page.getByRole('alertdialog').getByRole('button', { name: '撤销授权', exact: true }).click()
    await expect(page.getByRole('alertdialog')).toHaveCount(0)
    expect(mock.calls.filter((call) => call.method === 'POST' && call.path === '/oidc/clients')).toHaveLength(1)
    expect(mock.calls.filter((call) => call.method === 'POST' && call.path === '/application')).toHaveLength(2)
    expect(mock.calls.filter((call) => call.method === 'DELETE')).toHaveLength(2)
    expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain('TEST_SECRET')
})
