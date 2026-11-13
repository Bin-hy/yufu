import { chromium, expect } from '@playwright/test'
import process from 'node:process'
import { URL } from 'node:url'
import { log } from 'node:console'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
// Start the local Vite server on port 5179 first. All API responses below are fixtures.
const baseURL = process.env.FEEDBACK_BASE_URL ?? 'http://127.0.0.1:5179'
const screenshot = join(tmpdir(), 'cobotrix-feedback-dialog.png')
const browser = await chromium.launch({ headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
const identity = { user_id: 12, sub: '12', username: 'operator', nickname: '操作人', person_kind: 'employee', developer_enabled: true, administrator: true, sso_session_id: 'root-browser', auth_time: 10, expires_at: 9999999999, csrf_token: 'fixture-proof' }
let writeCount = 0
let releaseWrite
const writeGate = new Promise(resolve => { releaseWrite = resolve })
const errors = []
page.on('pageerror', e => errors.push(e.message))
await page.route('**/api/v1/**', async route => {
    const request = route.request(), path = new URL(request.url()).pathname
    const json = (body, status=200) => route.fulfill({ status, json: body })
    if (path.endsWith('/account/session')) return json(identity)
    if (path.endsWith('/identity/csrf')) return json({ csrf_token: 'fixture-proof' })
    if (path.endsWith('/admin/accounts') && request.method() === 'GET') return json({ items: [{ user_id: 21, username: 'last-admin', nickname: '管理员甲', enabled: true, person_kind: 'employee', developer_enabled: true, revision: 3 }], next: null })
    if (request.method() !== 'GET') {
        writeCount++
        await writeGate
        return json({ code: 'last_administrator', request_id: 'browser-feedback-1' }, 409)
    }
    return json({ items: [], next: null })
})
try {
    await page.goto(new URL('/admin/accounts', baseURL).href)
    const stop = page.locator('.desktop-table').getByRole('button', { name: '停用', exact: true })
    await stop.click()
    const dialog = page.getByRole('alertdialog')
    await expect(dialog).toContainText('管理员甲')
    await expect(dialog.getByRole('button', { name: '取消', exact: true })).toBeFocused()
    await page.keyboard.press('Tab')
    await expect(dialog.getByRole('button', { name: '停用并撤销登录', exact: true })).toBeFocused()
    await page.keyboard.press('Enter')
    await expect(dialog.getByRole('button', { name: '停用并撤销登录', exact: true })).toBeDisabled()
    await expect(dialog.getByRole('button', { name: '取消', exact: true })).toBeDisabled()
    await page.keyboard.press('Escape')
    await expect(dialog).toBeVisible()
    releaseWrite()
    await expect(dialog).toContainText('最后一个有效管理员')
    await expect(dialog).toContainText('browser-feedback-1')
    await expect(page.locator('.toast-item')).toHaveCount(0)
    await page.screenshot({ path: screenshot, fullPage: true })
    await dialog.getByRole('button', { name: '取消', exact: true }).focus()
    await page.keyboard.press('Enter')
    await expect(dialog).toHaveCount(0)
    await expect(stop).toBeFocused()
    await expect(page.getByRole('alert')).toContainText('最后一个有效管理员')
    if (writeCount !== 1 || errors.length) throw Error(JSON.stringify({ writeCount, errors }))
    log(JSON.stringify({ result: 'passed', writeCount, checks: ['实际页面弹窗', '取消按钮初始焦点', 'Tab 切换焦点', 'Enter 提交', '约束原因与请求标识', '无重复 Toast', '执行中防止关闭和重复提交', '失败保持打开', 'Enter 取消', '焦点回到触发按钮', '页面保留错误'], pageErrors: errors, screenshot }))
} finally { await browser.close() }
