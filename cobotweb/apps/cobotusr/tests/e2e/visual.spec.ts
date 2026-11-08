import { test, expect, keyboardTab } from './fixture'
import { mkdir, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
const evidence = fileURLToPath(
    new URL('../../../../../docs/specs/cobotusr-frontend-redesign/evidence/after/', import.meta.url),
)
const sizes = [
    { width: 1440, height: 960 },
    { width: 1280, height: 800 },
    { width: 768, height: 1024 },
    { width: 390, height: 844 },
    { width: 360, height: 800 },
]
for (const size of sizes)
    for (const theme of ['light', 'dark'] as const)
        test(`${size.width}×${size.height} ${theme}：布局、触控和实际截图`, async ({ page }, info) => {
            await page.setViewportSize(size)
            await page.addInitScript((theme) => localStorage.setItem('cobotusr:theme', theme), theme)
            await page.goto('/users?page_number=3')
            await expect(page.locator('tbody tr')).toHaveCount(10)
            expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(size.width)
            await expect(page.getByRole('heading', { level: 1, name: '用户管理', exact: true })).toBeVisible()
            expect(await page.evaluate(() => document.documentElement.classList.contains('dark'))).toBe(
                theme === 'dark',
            )
            const main = await page.locator('#workspace-content').boundingBox()
            expect(main!.width).toBeGreaterThan(size.width < 1024 ? size.width - 2 : size.width - 225)
            const table = await page
                .locator('.table-scroll')
                .evaluate((el) => ({ client: el.clientWidth, scroll: el.scrollWidth }))
            if (size.width < 768) expect(table.scroll).toBeGreaterThan(table.client)
            const targets = await page.locator('button:visible, .tags-bar a:visible').evaluateAll(
                (nodes, width) =>
                    nodes
                        .filter((node) => !(node as HTMLButtonElement).disabled)
                        .map((node) => {
                            const r = node.getBoundingClientRect()
                            return {
                                name: node.getAttribute('aria-label') ?? node.textContent?.trim(),
                                width: r.width,
                                height: r.height,
                                minimum: Number(width) < 640 ? 44 : 24,
                            }
                        }),
                size.width,
            )
            for (const target of targets) {
                expect(target.width, target.name).toBeGreaterThanOrEqual(target.minimum)
                expect(target.height, target.name).toBeGreaterThanOrEqual(target.minimum)
            }
            if (info.project.name === 'chromium') {
                await mkdir(evidence, { recursive: true })
                await page.screenshot({
                    path: evidence + `users-${size.width}-${theme}.png`,
                    fullPage: true,
                    animations: 'disabled',
                })
            }
        })
test('所有业务模块及入口使用统一布局，加载、空和失败有不同实际截图', async ({ page, mock }, info) => {
    await page.setViewportSize({ width: 1440, height: 960 })
    await page.addInitScript(() => localStorage.setItem('cobotusr:theme', 'light'))
    for (const [path, name] of [
        ['/dashboard', 'dashboard'],
        ['/users/create', 'create-user'],
        ['/roles', 'roles'],
        ['/authority', 'authority'],
        ['/authority/assign?roleId=1', 'authority-assignment'],
        ['/permissions', 'permissions'],
        ['/permissions/assign?roleId=1', 'permission-assignment'],
        ['/applications', 'applications'],
        ['/authorized-apps', 'authorized-apps'],
        ['/recordings', 'recordings'],
        ['/profile', 'profile'],
        ['/login', 'login'],
        ['/register', 'register'],
        ['/403', '403'],
        ['/404', '404'],
        ['/oidc/consent?client_id=client-console&redirect_uri=https://example.test/callback', 'consent'],
    ]) {
        await page.goto(path)
        await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
        await expect(page.getByRole('heading', { name: '正在加载', exact: true })).toHaveCount(0)
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(1440)
        if (info.project.name === 'chromium') {
            await mkdir(evidence, { recursive: true })
            await page.screenshot({
                path: evidence + name + '-1440-light.png',
                fullPage: true,
                animations: 'disabled',
            })
        }
    }
    mock.delays.set('GET /users', 1000)
    await page.goto('/users')
    await expect(page.getByRole('heading', { name: '正在加载' }).first()).toBeVisible()
    if (info.project.name === 'chromium')
        await page.screenshot({ path: evidence + 'users-loading.png', animations: 'disabled' })
    await expect(page.locator('tbody tr')).toHaveCount(10)
    mock.delays.delete('GET /users')
    mock.failures.set('GET /users', 1)
    await page.reload()
    await expect(page.getByRole('heading', { name: '加载失败', exact: true })).toBeVisible()
    if (info.project.name === 'chromium')
        await page.screenshot({ path: evidence + 'users-error.png', animations: 'disabled' })
    mock.users.splice(0)
    await page.getByRole('button', { name: '重新加载', exact: true }).click()
    await expect(page.getByRole('heading', { name: '暂无数据' })).toBeVisible()
    if (info.project.name === 'chromium')
        await page.screenshot({ path: evidence + 'users-empty.png', animations: 'disabled' })
})
test('应用失败、一次性密钥及移动认证的实际实现截图', async ({ page, mock }, info) => {
    async function screenshot(name: string) {
        if (info.project.name === 'chromium')
            await page.screenshot({ path: evidence + name + '.png', animations: 'disabled' })
    }
    await page.addInitScript(() => localStorage.setItem('cobotusr:theme', 'system'))
    await page.emulateMedia({ colorScheme: 'light' })
    await page.setViewportSize({ width: 1440, height: 960 })
    await page.goto('/applications')
    await page.getByRole('button', { name: '注册应用', exact: true }).click()
    await page.getByLabel('应用名称', { exact: true }).fill('验收示例应用')
    await page.getByLabel('授权回调地址', { exact: true }).fill('https://example.test/evidence')
    mock.failures.set('POST /application', 1)
    await page.getByRole('dialog').getByRole('button', { name: '保存应用', exact: true }).click()
    await expect(page.getByRole('dialog').getByRole('alert')).toBeVisible()
    await screenshot('application-partial-failure-1440-light')
    await page.getByRole('button', { name: '重试剩余步骤', exact: true }).click()
    await page.getByRole('button', { name: '显示密钥', exact: true }).click()
    await expect(page.getByText('TEST_SECRET_MEMORY_ONLY', { exact: true })).toBeVisible()
    await screenshot('application-secret-1440-light')
    await page.setViewportSize({ width: 390, height: 844 })
    await page.emulateMedia({ colorScheme: 'dark' })
    await expect(page.locator('html')).toHaveClass(/dark/)
    const secretRect = await page.getByText('TEST_SECRET_MEMORY_ONLY', { exact: true }).boundingBox()
    expect(secretRect!.y).toBeGreaterThanOrEqual(0)
    expect(secretRect!.y + secretRect!.height).toBeLessThanOrEqual(844)
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390)
    await screenshot('application-secret-390-dark')
    await page.evaluate(() =>
        Object.defineProperty(navigator, 'clipboard', {
            configurable: true,
            value: {
                writeText: async () => {
                    throw new Error('mock clipboard denied')
                },
            },
        }),
    )
    await page.getByRole('button', { name: '复制密钥', exact: true }).click()
    await expect(page.getByRole('dialog').getByRole('alert')).toContainText('复制失败')
    await page.keyboard.press('Escape')
    await expect(page.getByRole('dialog', { name: '关闭密钥结果？', exact: true })).toBeVisible()
    await page.getByRole('button', { name: '继续查看', exact: true }).click()
    await expect(page.getByText('TEST_SECRET_MEMORY_ONLY', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: '已保存，完成', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    for (const [path, name] of [
        ['/login', 'login-390-dark'],
        ['/register', 'register-390-dark'],
        ['/oidc/login?client_id=client-console&state=visual-state', 'oidc-login-390-dark'],
        ['/oidc/consent?client_id=client-console&redirect_uri=https://example.test/callback', 'consent-390-dark'],
    ]) {
        await page.goto(path)
        await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
        await expect(page.getByRole('heading', { name: '正在加载', exact: true })).toHaveCount(0)
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390)
        await screenshot(name)
    }
})
test('主题跟随系统，手动选择保持；小屏可通过用户菜单切换', async ({ page }) => {
    await page.emulateMedia({ colorScheme: 'light' })
    await page.addInitScript(() => {
        if (!localStorage.getItem('cobotusr:theme')) localStorage.setItem('cobotusr:theme', 'system')
    })
    await page.goto('/dashboard')
    await expect(page.locator('html')).not.toHaveClass(/dark/)
    await page.emulateMedia({ colorScheme: 'dark' })
    await expect(page.locator('html')).toHaveClass(/dark/)
    await page.getByRole('button', { name: '浅色', exact: true }).click()
    await page.emulateMedia({ colorScheme: 'light' })
    await page.emulateMedia({ colorScheme: 'dark' })
    await expect(page.locator('html')).not.toHaveClass(/dark/)
    await page.reload()
    await expect(page.locator('html')).not.toHaveClass(/dark/)
    await page.setViewportSize({ width: 390, height: 844 })
    await page.getByRole('button', { name: '用户菜单' }).click()
    await page.getByRole('menuitem', { name: '主题：暗色', exact: true }).click()
    await expect(page.locator('html')).toHaveClass(/dark/)
    await page.getByRole('button', { name: '用户菜单' }).click()
    await page.getByRole('menuitem', { name: '主题：跟随系统', exact: true }).click()
    await page.emulateMedia({ colorScheme: 'light' })
    await expect(page.locator('html')).not.toHaveClass(/dark/)
})
test('键盘跳转、焦点可见、减少动画及异步语义', async ({ page, browserName }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' })
    await page.goto('/users')
    await expect(page.locator('tbody tr')).toHaveCount(10)
    await keyboardTab(page, browserName)
    await expect(page.getByRole('link', { name: '跳到主内容' })).toBeFocused()
    await page.keyboard.press('Enter')
    await expect(page.locator('#workspace-content')).toBeFocused()
    await keyboardTab(page, browserName)
    const style = await page.evaluate(() => {
        const el = document.activeElement!
        const s = getComputedStyle(el),
            r = el.getBoundingClientRect(),
            main = document.querySelector('#workspace-content')!.getBoundingClientRect()
        return {
            outline: s.outlineStyle,
            width: s.outlineWidth,
            transition: s.transitionDuration,
            top: r.top,
            bottom: r.bottom,
            mainTop: main.top,
            mainBottom: main.bottom,
        }
    })
    expect(style.outline).not.toBe('none')
    expect(parseFloat(style.width)).toBeGreaterThanOrEqual(2)
    expect(style.top).toBeGreaterThanOrEqual(style.mainTop)
    expect(style.bottom).toBeLessThanOrEqual(style.mainBottom)
    expect(parseFloat(style.transition)).toBeLessThanOrEqual(0.001)
    await page.getByRole('button', { name: '刷新', exact: true }).click()
    await expect(page.locator('[aria-live]')).toHaveCount(2)
})
for (const theme of ['light', 'dark'] as const)
    test(theme + '实际文本与背景对比度至少4.5', async ({ page }, info) => {
        await page.addInitScript((theme) => localStorage.setItem('cobotusr:theme', theme), theme)
        const samples: { text: string; ratio: number; color: string; background: number[] }[] = []
        for (const path of ['/dashboard', '/users', '/applications', '/login']) {
            await page.goto(path)
            await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
            await expect(page.getByRole('heading', { name: '正在加载', exact: true })).toHaveCount(0)
            samples.push(
                ...(await page.evaluate(() => {
                    const rgb = (value: string) => {
                        const match = value.match(/[\d.]+/g)?.map(Number) ?? [0, 0, 0]
                        return [match[0]!, match[1]!, match[2]!, match.length > 3 ? match[3]! : 1]
                    }
                    const blend = (top: number[], bottom: number[]) =>
                        top.slice(0, 3).map((v, i) => v * top[3]! + bottom[i]! * (1 - top[3]!))
                    const luminance = (c: number[]) =>
                        c
                            .map((v) => {
                                v /= 255
                                return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
                            })
                            .reduce((s, v, i) => s + v * [0.2126, 0.7152, 0.0722][i]!, 0)
                    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT),
                        results: {
                            text: string
                            ratio: number
                            color: string
                            background: number[]
                        }[] = []
                    while (walker.nextNode()) {
                        const node = walker.currentNode,
                            el = node.parentElement,
                            text = node.textContent?.trim()
                        if (!el || !text || el.closest('script,style,[aria-hidden="true"],.sr-only,:disabled')) continue
                        const rect = el.getBoundingClientRect(),
                            style = getComputedStyle(el)
                        if (
                            rect.width === 0 ||
                            rect.height === 0 ||
                            rect.bottom < 0 ||
                            rect.top > innerHeight ||
                            style.visibility === 'hidden' ||
                            style.display === 'none'
                        )
                            continue
                        let background = [255, 255, 255],
                            opacity = 1
                        const chain: Element[] = []
                        for (let parent: Element | null = el; parent; parent = parent.parentElement)
                            chain.unshift(parent)
                        for (const parent of chain) {
                            const s = getComputedStyle(parent)
                            background = blend(rgb(s.backgroundColor), background)
                            opacity *= Number(s.opacity)
                        }
                        const foreground = rgb(style.color)
                        foreground[3] *= opacity
                        const effective = blend(foreground, background),
                            a = luminance(effective),
                            b = luminance(background)
                        results.push({
                            text: text.slice(0, 60),
                            ratio: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05),
                            color: style.color,
                            background,
                        })
                    }
                    return results
                })),
            )
        }
        const failures = samples.filter((s) => s.ratio < 4.5)
        expect(failures).toEqual([])
        if (info.project.name === 'chromium') {
            await mkdir(evidence, { recursive: true })
            await writeFile(
                evidence + 'contrast-' + theme + '.json',
                JSON.stringify({ theme, minimum: Math.min(...samples.map((s) => s.ratio)), samples }, null, 2),
            )
        }
    })
