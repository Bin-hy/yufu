import { test, expect } from './fixture'
test('Tab 地址与页状态保留，初访只请求一次', async ({ page, mock }) => {
    await page.goto('/users?page_number=3&page_size=10')
    await expect(page.getByText('user21', { exact: false }).first()).toBeVisible()
    expect(
        mock.calls.filter((call) => call.path === '/users' && call.query.includes('page_size=10')),
    ).toHaveLength(1)
    await page
        .locator('.workspace-sidebar')
        .getByRole('link', { name: '角色管理', exact: true })
        .click()
    await expect(page.getByRole('heading', { name: '角色管理', exact: true })).toBeVisible()
    await page.locator('.tags-bar').getByRole('link', { name: '用户管理', exact: true }).click()
    await expect(page).toHaveURL(/page_number=3/)
    await expect(page.getByText('user21', { exact: false }).first()).toBeVisible()
    expect(
        mock.calls.filter((call) => call.path === '/users' && call.query.includes('page_size=10')),
    ).toHaveLength(1)
})
test('慢旧请求无法覆盖快的新页，前进后退与地址一致', async ({ page, mock }) => {
    mock.delays.set('/users?page_number=2&page_size=10', 650)
    await page.goto('/users')
    await expect(page.locator('tbody tr')).toHaveCount(10)
    await page.getByRole('button', { name: '第 2 页', exact: true }).click()
    await page.getByRole('button', { name: '第 3 页', exact: true }).click()
    await expect(page.getByText('user21', { exact: false }).first()).toBeVisible()
    await page.waitForTimeout(750)
    await expect(page).toHaveURL(/page_number=3/)
    await expect(page.getByText('user21', { exact: false }).first()).toBeVisible()
    await page.goBack()
    await expect(page).toHaveURL(/page_number=2/)
    await expect(page.getByText('user11', { exact: false }).first()).toBeVisible()
})
test('首页别名只产生一个固定标签', async ({ page }) => {
    await page.goto('/dashboard/introduction')
    await expect(page.getByRole('heading', { name: /你好/ })).toBeVisible()
    await expect(page.locator('.workspace-tab')).toHaveCount(1)
    await expect(page).toHaveURL(/\/dashboard$/)
})
test('折叠侧栏可以直接进入创建用户', async ({ page }) => {
    await page.goto('/dashboard')
    await page.locator('.workspace-topbar').getByRole('button', { name: '切换侧栏' }).click()
    await page
        .locator('.workspace-sidebar')
        .getByRole('button', { name: '用户管理', exact: true })
        .click()
    await page.getByRole('menuitem', { name: '创建用户', exact: true }).click()
    await expect(page.getByRole('heading', { name: '创建用户', exact: true })).toBeVisible()
})
test('关闭标签先完成导航，重新打开得到新实例', async ({ page }) => {
    await page.goto('/users?page_number=3')
    await expect(page.locator('tbody tr')).toHaveCount(10)
    await page
        .locator('.workspace-sidebar')
        .getByRole('link', { name: '角色管理', exact: true })
        .click()
    await page
        .locator('.tags-bar')
        .getByRole('button', { name: '关闭角色管理', exact: true })
        .click()
    await expect(page).toHaveURL(/\/users/)
    await expect(
        page.locator('.tags-bar').getByRole('link', { name: '角色管理', exact: true }),
    ).toHaveCount(0)
    await page
        .locator('.tags-bar')
        .getByRole('button', { name: '关闭用户管理', exact: true })
        .click()
    await expect(page).toHaveURL(/\/dashboard$/)
    await page
        .locator('.workspace-sidebar')
        .getByRole('link', { name: '用户管理', exact: true })
        .click()
    await expect(page.getByText('user1 · #1', { exact: true })).toBeVisible()
})
test('新建用户默认角色不会产生脏表单，输入后可取消关闭', async ({ page }) => {
    await page.goto('/users/create')
    await expect(page.getByLabel('角色', { exact: true })).toHaveValue('1')
    await page
        .locator('.tags-bar')
        .getByRole('button', { name: '关闭创建用户', exact: true })
        .click()
    await expect(page).toHaveURL(/\/dashboard$/)
    await page
        .locator('.workspace-sidebar')
        .getByRole('link', { name: '创建用户', exact: true })
        .click()
    await page.getByRole('textbox', { name: '用户名', exact: true }).fill('draft-user')
    await page
        .locator('.tags-bar')
        .getByRole('button', { name: '关闭创建用户', exact: true })
        .click()
    await expect(page.getByRole('alertdialog')).toBeVisible()
    await page.getByRole('button', { name: '继续编辑', exact: true }).click()
    await expect(page.getByRole('textbox', { name: '用户名', exact: true })).toHaveValue(
        'draft-user',
    )
    await page
        .locator('.workspace-sidebar')
        .getByRole('link', { name: '角色管理', exact: true })
        .click()
    await page.getByRole('button', { name: '保留并切换', exact: true }).click()
    await page.locator('.tags-bar').getByRole('link', { name: '创建用户', exact: true }).click()
    await expect(page.getByRole('textbox', { name: '用户名', exact: true })).toHaveValue(
        'draft-user',
    )
})
test('390px 导航抽屉可用且页面不横向溢出', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto('/users')
    await expect(page.getByRole('heading', { name: '用户管理', exact: true })).toBeVisible()
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390)
    await page.getByRole('button', { name: '打开导航' }).click()
    const dialog = page.getByRole('dialog')
    await expect(dialog).toBeVisible()
    await dialog.getByRole('link', { name: '角色管理', exact: true }).click()
    await expect(dialog).toHaveCount(0)
    await expect(page.getByRole('heading', { name: '角色管理', exact: true })).toBeVisible()
})
test('用户菜单为有效菜单，退出清空标签与快照', async ({ page }) => {
    await page.goto('/users')
    await expect(page.locator('tbody tr')).toHaveCount(10)
    await page.getByRole('button', { name: '用户菜单' }).click()
    await page.getByRole('menuitem', { name: '退出登录' }).click()
    await expect(page).toHaveURL(/\/login$/)
    await page.getByRole('textbox', { name: '用户名', exact: true }).fill('tester')
    await page.getByLabel('密码', { exact: true }).fill('password')
    await page.getByRole('button', { name: '登录', exact: true }).click()
    await expect(page).toHaveURL(/\/dashboard$/)
    await expect(page.locator('.workspace-tab')).toHaveCount(1)
    expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain(
        'TEST_LOGIN_TOKEN',
    )
})
test('刷新恢复同一 owner 标签，工作台 localStorage 失败仍可使用', async ({ page }) => {
    await page.goto('/users?page_number=3')
    await expect(page.locator('tbody tr')).toHaveCount(10)
    await page.reload()
    await expect(page).toHaveURL(/page_number=3/)
    await expect(page.locator('.workspace-tab')).toHaveCount(2)
    await page.addInitScript(() => {
        const setItem = Storage.prototype.setItem
        const getItem = Storage.prototype.getItem
        Storage.prototype.setItem = function (key, value) {
            if (this === localStorage) throw Error('quota')
            return setItem.call(this, key, value)
        }
        Storage.prototype.getItem = function (key) {
            if (this === localStorage) throw Error('blocked')
            return getItem.call(this, key)
        }
    })
    await page.reload()
    await expect(page.getByRole('heading', { name: '用户管理', exact: true })).toBeVisible()
    await page
        .locator('.workspace-sidebar')
        .getByRole('link', { name: '角色管理', exact: true })
        .click()
    await expect(page.getByRole('heading', { name: '角色管理', exact: true })).toBeVisible()
})

test('所有 Storage 禁用时可内存登录、导航和退出，重新加载需要登录', async ({ page, mock }) => {
    await page.context().clearCookies()
    await page.addInitScript(() => {
        for (const method of ['getItem', 'setItem', 'removeItem'] as const)
            Storage.prototype[method] = () => { throw Error('storage blocked') }
    })
    await page.goto('/login')
    const signIn = async () => {
        await page.getByLabel('用户名', { exact: true }).fill('memory-user')
        await page.getByLabel('密码', { exact: true }).fill('test password')
        await page.getByRole('button', { name: '登录', exact: true }).click()
        await expect(page).toHaveURL(/\/dashboard$/)
    }
    await signIn()
    await page.locator('.workspace-sidebar').getByRole('link', { name: '用户管理', exact: true }).click()
    await expect(page.locator('tbody tr')).toHaveCount(10)
    await page.reload()
    await expect(page).toHaveURL(/\/login\?redirect=/)
    await page.getByLabel('用户名', { exact: true }).fill('memory-user')
    await page.getByLabel('密码', { exact: true }).fill('test password')
    await page.getByRole('button', { name: '登录', exact: true }).click()
    await expect(page.getByRole('heading', { name: '用户管理', exact: true })).toBeVisible()
    await page.getByRole('button', { name: '用户菜单' }).click()
    await page.getByRole('menuitem', { name: '退出登录' }).click()
    await expect(page).toHaveURL(/\/login$/)
    expect(mock.calls.filter((call) => call.path === '/logout')).toHaveLength(1)
})
