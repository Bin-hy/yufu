import { test, expect } from './fixture'
import type { Page } from '@playwright/test'
async function nav(page: Page, name: string) {
    await page.locator('.workspace-sidebar').getByRole('link', { name, exact: true }).click()
}
async function closeMenu(page: Page, name: string) {
    await page.getByRole('button', { name: '页面标签操作' }).click()
    await page.getByRole('menuitem', { name, exact: true }).click()
}
test('关闭右侧和其他标签保留首页；被关闭页重开建立新状态', async ({ page }) => {
    await page.goto('/users?page_number=3')
    await nav(page, '角色管理')
    await nav(page, 'API 权限')
    await page.locator('.tags-bar').getByRole('link', { name: '用户管理', exact: true }).click()
    await closeMenu(page, '关闭右侧')
    await expect(page.locator('.workspace-tab')).toHaveCount(2)
    await expect(page).toHaveURL(/page_number=3/)
    await nav(page, '角色管理')
    await closeMenu(page, '关闭其他')
    await expect(page.locator('.workspace-tab')).toHaveCount(2)
    await nav(page, '用户管理')
    await expect(page).toHaveURL(/\/users$/)
})
test('完整草稿流程：保留、后退取消、批量关闭、放弃后重开清空', async ({ page }) => {
    await page.goto('/users/create')
    await page.getByRole('textbox', { name: '用户名', exact: true }).fill('TEST_DRAFT_PRIVATE')
    await nav(page, '角色管理')
    await page.getByRole('button', { name: '保留并切换', exact: true }).click()
    await page.locator('.tags-bar').getByRole('link', { name: '创建用户', exact: true }).click()
    await expect(page.getByRole('textbox', { name: '用户名', exact: true })).toHaveValue(
        'TEST_DRAFT_PRIVATE',
    )
    await page.goBack()
    await page.getByRole('button', { name: '继续编辑', exact: true }).click()
    await expect(page).toHaveURL(/\/users\/create$/)
    await closeMenu(page, '关闭其他')
    await expect(page.locator('.workspace-tab')).toHaveCount(2)
    await page.getByRole('button', { name: '关闭创建用户', exact: true }).click()
    await page.getByRole('button', { name: '继续编辑', exact: true }).click()
    await page.getByRole('button', { name: '关闭创建用户', exact: true }).click()
    await page.getByRole('button', { name: '放弃修改', exact: true }).click()
    await expect(page).toHaveURL(/\/dashboard$/)
    await nav(page, '创建用户')
    await expect(page.getByRole('textbox', { name: '用户名', exact: true })).toHaveValue('')
    expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain(
        'TEST_DRAFT_PRIVATE',
    )
})
test('脏表单批量关闭取消保持所有标签；主动退出取消保留登录', async ({ page }) => {
    await page.goto('/users/create')
    await page.getByRole('textbox', { name: '用户名', exact: true }).fill('keep-input')
    await nav(page, '角色管理')
    await page.getByRole('button', { name: '保留并切换', exact: true }).click()
    await closeMenu(page, '关闭其他')
    await page.getByRole('button', { name: '继续编辑', exact: true }).click()
    await expect(page.locator('.workspace-tab')).toHaveCount(3)
    await page.getByRole('button', { name: '用户菜单' }).click()
    await page.getByRole('menuitem', { name: '退出登录' }).click()
    await page.getByRole('button', { name: '继续编辑', exact: true }).click()
    await expect(page).toHaveURL(/\/roles$/)
    await page.locator('.tags-bar').getByRole('link', { name: '创建用户', exact: true }).click()
    await expect(page.getByRole('textbox', { name: '用户名', exact: true })).toHaveValue(
        'keep-input',
    )
})
test('移动抽屉取消草稿导航仍打开，保留并切换后才关闭', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto('/users/create')
    await page.getByRole('textbox', { name: '用户名', exact: true }).fill('mobile-draft')
    await page.getByRole('button', { name: '打开导航' }).click()
    const drawer = page.getByRole('dialog', { name: '工作台导航' })
    await drawer.getByRole('link', { name: '角色管理', exact: true }).click()
    await page.getByRole('button', { name: '继续编辑', exact: true }).click()
    await expect(drawer).toBeVisible()
    await expect(page).toHaveURL(/\/users\/create$/)
    await drawer.getByRole('link', { name: '角色管理', exact: true }).click()
    await page.getByRole('button', { name: '保留并切换', exact: true }).click()
    await expect(drawer).toHaveCount(0)
    await expect(page).toHaveURL(/\/roles$/)
})
test('滚动、筛选和分页独立恢复；无效页码仅修正一次', async ({ page, mock }) => {
    await page.setViewportSize({ width: 1280, height: 500 })
    await page.goto('/users?page_number=3&q=user2')
    await expect(page.locator('tbody tr')).toHaveCount(9)
    await page.locator('#workspace-content').evaluate((el) => {
        el.scrollTop = 200
    })
    const top = await page.locator('#workspace-content').evaluate((el) => el.scrollTop)
    expect(top).toBeGreaterThan(0)
    await nav(page, '角色管理')
    await page.locator('.tags-bar').getByRole('link', { name: '用户管理', exact: true }).click()
    await expect(page).toHaveURL(/page_number=3&q=user2/)
    await expect
        .poll(() => page.locator('#workspace-content').evaluate((el) => el.scrollTop))
        .toBe(top)
    await page.goto('/users?page_number=999')
    await expect(page).toHaveURL(/page_number=3/)
    expect(
        mock.calls.filter(
            (call) => call.path === '/users' && call.query.includes('page_number=999'),
        ),
    ).toHaveLength(1)
    await page.goto('/users?page_number=-2&page_size=1000')
    await expect(page).toHaveURL(/page_number=1&page_size=10/)
    await expect(page.locator('tbody tr')).toHaveCount(10)
})
test('保存角色后失活用户列表下一次激活刷新，目录更新不丢权限草稿', async ({ page, mock }) => {
    await page.goto('/users')
    await expect(page.locator('tbody tr')).toHaveCount(10)
    await nav(page, '角色管理')
    await page.getByRole('button', { name: '重命名', exact: true }).first().click()
    await page.getByLabel('角色名称', { exact: true }).fill('新的管理员')
    await page.getByRole('dialog').getByRole('button', { name: '确认', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await page.locator('.tags-bar').getByRole('link', { name: '用户管理', exact: true }).click()
    await expect(page.locator('tbody tr').first().getByRole('combobox')).toContainText('新的管理员')
    expect(mock.calls.filter((call) => call.path === '/users')).toHaveLength(2)
    await nav(page, 'API 权限分配')
    await page.getByLabel('选择角色', { exact: true }).selectOption('1')
    await page.getByRole('checkbox').nth(2).check()
    await nav(page, 'API 权限')
    await page.getByRole('button', { name: '保留并切换', exact: true }).click()
    await page.getByRole('button', { name: '新增权限' }).click()
    await page.getByLabel('权限名称', { exact: true }).fill('新目录项')
    await page.getByLabel('权限标识', { exact: true }).fill('/fresh')
    await page.getByRole('dialog').getByRole('button', { name: '确认', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await page.locator('.tags-bar').getByRole('link', { name: 'API 权限分配', exact: true }).click()
    await expect(page.getByRole('checkbox')).toHaveCount(5)
    await expect(page.getByRole('checkbox').nth(2)).toBeChecked()
    await expect(page.getByRole('button', { name: '保存权限', exact: true })).toBeEnabled()
})
test('403保留登录，logout接口失败仍清空本地状态', async ({ page, mock }) => {
    mock.statuses.set('GET /users', { status: 403, remaining: 1 })
    await page.goto('/users')
    await expect(page.getByRole('heading', { name: '加载失败' })).toBeVisible()
    await expect(page).toHaveURL(/\/users$/)
    await page.getByRole('button', { name: '重新加载', exact: true }).click()
    await expect(page.locator('tbody tr')).toHaveCount(10)
    mock.failures.set('POST /logout', 1)
    await page.getByRole('button', { name: '用户菜单' }).click()
    await page.getByRole('menuitem', { name: '退出登录' }).click()
    await expect(page).toHaveURL(/\/login$/)
    expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain(
        'cobotusr:workspace',
    )
})
test('并发401单次refresh；失效清除脏表单，账号B不受旧响应影响', async ({ page, mock }) => {
    mock.statuses.set('GET /users', { status: 401, remaining: 1 })
    mock.statuses.set('GET /role/modifiable', { status: 401, remaining: 1 })
    mock.delays.set('POST /refresh', 400)
    await page.goto('/users')
    await expect(page.locator('tbody tr')).toHaveCount(10)
    expect(mock.calls.filter((call) => call.path === '/refresh')).toHaveLength(1)
    await nav(page, '创建用户')
    await page.getByRole('textbox', { name: '用户名', exact: true }).fill('OLD_ACCOUNT_DRAFT')
    await nav(page, '操作记录')
    await page.getByRole('button', { name: '保留并切换', exact: true }).click()
    mock.statuses.set('GET /recordings', { status: 401, remaining: 1 })
    mock.statuses.set('POST /refresh', { status: 401, remaining: 1 })
    await page.getByRole('button', { name: '刷新', exact: true }).click()
    await expect(page).toHaveURL(/\/login/)
    await expect(page.getByRole('alertdialog')).toHaveCount(0)
    await page.getByLabel('用户名', { exact: true }).fill('accountB')
    await page.getByLabel('密码', { exact: true }).fill('password')
    await page.getByRole('button', { name: '登录', exact: true }).click()
    await expect(page).toHaveURL(/\/dashboard$/)
    await expect(page.locator('.workspace-tab')).toHaveCount(1)
    expect(mock.identity.id).toBe(2)
    expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain(
        'OLD_ACCOUNT_DRAFT',
    )
})
test('脏表单刷新保护与提交pending导航保护', async ({ page, mock }) => {
    await page.goto('/users/create')
    for (const [name, value] of [
        ['用户名', 'pending-user'],
        ['昵称', '待创建用户'],
        ['密码', 'password'],
        ['确认密码', 'password'],
    ])
        await page.getByLabel(new RegExp('^' + name)).fill(value)
    expect(
        await page.evaluate(() => {
            const event = new Event('beforeunload', { cancelable: true })
            window.dispatchEvent(event)
            return event.defaultPrevented
        }),
    ).toBe(true)
    mock.delays.set('POST /user', 700)
    await page.getByRole('button', { name: '创建用户', exact: true }).click()
    await nav(page, '角色管理')
    await expect(page).toHaveURL(/\/users\/create$/)
    await expect(page.getByText('正在提交，请等待操作完成', { exact: true })).toBeVisible()
    await expect(page).toHaveURL(/\/users$/)
})
