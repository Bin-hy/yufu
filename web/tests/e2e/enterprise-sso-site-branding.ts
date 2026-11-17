import assert from 'node:assert/strict'
import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'
import type { Browser, BrowserContext, Page } from '@playwright/test'
import { config, loginAtCenter, awaitConsentIfShown } from './enterprise-sso-fixture.ts'
async function visibleText(page: Page, selector: string, expected: string) {
  await page.waitForFunction(({ selector, expected }) => Array.from(document.querySelectorAll(selector)).some(el => el.textContent?.includes(expected)), { selector, expected })
}
async function csrf(context: BrowserContext, origin: string) {
  const response = await context.request.get(origin + '/api/v1/identity/csrf')
  assert.equal(response.status(), 200)
  return (await response.json()).csrf_token as string
}
export async function runSiteBrandingScenario(browser: Browser, engine: string) {
  const cfg = config(), context = await browser.newContext({ ignoreHTTPSErrors: true, viewport: { width: 1440, height: 1000 } })
  const visitor = await browser.newContext({ ignoreHTTPSErrors: true, viewport: { width: 1440, height: 1000 } })
  const page = await context.newPage(), publicPage = await visitor.newPage()
  const evidence = join(cfg.repoRoot, 'docs/specs/site-branding/evidence', engine)
  await mkdir(evidence, { recursive: true })
  let changed = false
  const save = async () => {
    const response = page.waitForResponse(r => r.url().endsWith('/api/v1/admin/site-settings') && r.request().method() === 'PUT')
    await page.getByRole('button', { name: '保存设置', exact: true }).click()
    assert.equal((await response).status(), 200)
    await page.getByText('已保存', { exact: false }).first().waitFor()
  }
  try {
    await page.goto(cfg.opOrigin + '/login')
    await page.getByLabel('用户名', { exact: true }).fill(cfg.username)
    await page.getByLabel('密码', { exact: true }).fill(cfg.password)
    await page.getByRole('button', { name: '登录', exact: true }).click()
    await page.waitForURL('**/account')
    const authRequests: string[] = []
    page.on('request', request => { if (/\/(identity\/login|oidc\/)/.test(request.url())) authRequests.push(request.url()) })
    await page.goto(cfg.opOrigin + '/admin/site-settings')
    await page.getByLabel('网站名称', { exact: true }).waitFor()
    const oldName = await page.getByLabel('网站名称', { exact: true }).inputValue()
    await page.getByLabel('网站名称', { exact: true }).fill('企业身份平台 ' + engine)
    await page.getByLabel('网站简介', { exact: true }).fill('企业统一入口')
    await page.getByLabel('欢迎标题', { exact: true }).fill('欢迎进入企业工作空间')
    await page.getByLabel('欢迎说明', { exact: true }).fill('使用企业账户访问获准的应用。')
    await page.getByLabel('版权文字', { exact: true }).fill('企业平台 · 自主管理')
    await page.getByLabel('备案号', { exact: true }).fill('测试备案 123')
    await page.getByLabel('备案链接', { exact: true }).fill('https://example.com/filing')
    await page.getByRole('button', { name: '添加页脚链接', exact: true }).click()
    await page.getByLabel('链接名称 1', { exact: true }).fill('帮助中心')
    await page.getByLabel('链接地址 1', { exact: true }).fill('https://example.com/help')
    await page.getByLabel('上传网站 Logo', { exact: true }).setInputFiles(join(cfg.repoRoot, 'tests/fixtures/site-branding/logo.png'))
    await page.getByText('上传中…', { exact: true }).waitFor({ state: 'hidden' })
    await page.getByLabel('上传全站认证封面', { exact: true }).setInputFiles(join(cfg.repoRoot, 'tests/fixtures/site-branding/logo.png'))
    await page.getByText('上传中…', { exact: true }).waitFor({ state: 'hidden' })
    await page.getByLabel('上传浏览器图标', { exact: true }).setInputFiles(join(cfg.repoRoot, 'tests/fixtures/site-branding/logo.png'))
    await page.getByText('上传中…', { exact: true }).waitFor({ state: 'hidden' })
    await publicPage.goto(cfg.opOrigin + '/login')
    await publicPage.waitForFunction(oldName => document.title.includes(oldName), oldName)
    const staging = await context.request.get(cfg.opOrigin + '/api/v1/admin/site-settings')
    assert.equal((await staging.json()).settings.document.branding.site_name, oldName)
    await save(); changed = true
    assert.deepEqual(authRequests, [])
    await page.screenshot({ path: join(evidence, 'settings-1440-light.png'), fullPage: true })
    await publicPage.reload(); await visibleText(publicPage, '.site-brand-name', '企业身份平台 ' + engine)
    await publicPage.waitForFunction(name => document.title.includes(name), '企业身份平台 ' + engine)
    await visibleText(publicPage, '.auth-visual h2', '欢迎进入企业工作空间')
    await visibleText(publicPage, '.site-footer', '测试备案 123')
    await publicPage.waitForFunction(() => document.querySelector<HTMLLinkElement>('link[rel="icon"]')?.getAttribute('href')?.includes('/api/v1/site/assets/'))
    const favicon = await publicPage.locator('link[rel="icon"]').getAttribute('href')
    const icon = await publicPage.request.get(new URL(favicon!, cfg.opOrigin).toString())
    assert.equal(icon.status(), 200); assert.equal(icon.headers()['content-type'], 'image/png')
    const help = publicPage.locator('.site-footer a', { hasText: '帮助中心' })
    assert.equal(await help.getAttribute('href'), 'https://example.com/help')
    await publicPage.waitForFunction(() => Array.from(document.images).filter(i => i.src.includes('/site/assets/')).every(i => i.complete && i.naturalWidth > 0))
    for (const width of [1440, 390, 360]) {
      await publicPage.setViewportSize({ width, height: width === 1440 ? 1000 : 844 })
      for (const theme of ['light', 'dark']) {
        await publicPage.evaluate((dark) => document.documentElement.classList.toggle('dark', dark), theme === 'dark')
        await publicPage.getByLabel('用户名', { exact: true }).waitFor({ state: 'visible' })
        assert.equal(await publicPage.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true)
        await publicPage.screenshot({ path: join(evidence, `login-${width}-${theme}.png`), fullPage: true })
      }
    }
    await publicPage.setViewportSize({ width: 1440, height: 1000 })
    await publicPage.evaluate(() => document.documentElement.classList.remove('dark'))
    await page.goto(cfg.opOrigin + '/account')
    await visibleText(page, '.site-brand-name', '企业身份平台 ' + engine)
    await visibleText(page, '.site-footer', '测试备案 123')
    await page.locator('.site-footer a', { hasText: '帮助中心' }).waitFor()
    await page.goto(cfg.opOrigin + '/admin/site-settings')
    await page.getByLabel('网站名称', { exact: true }).waitFor()
    await page.getByText('自定义 OIDC 欢迎标题', { exact: true }).check()
    await page.getByLabel('OIDC 欢迎标题', { exact: true }).fill('应用专属欢迎 ' + engine)
    await save()
    const oidc = await visitor.newPage(); await oidc.goto(cfg.aOrigin + '/')
    await oidc.getByRole('link', { name: /Sign in with Yufu/ }).click()
    await visibleText(oidc, '.auth-visual h2', '应用专属欢迎 ' + engine)
    await loginAtCenter(oidc, cfg)
    assert.equal(await oidc.getByTestId('local-subject').textContent(), cfg.subject)
    await page.getByText('自定义 OIDC 欢迎标题', { exact: true }).uncheck(); await save()
    const fresh = await browser.newContext({ ignoreHTTPSErrors: true }); const next = await fresh.newPage()
    try {
      await next.goto(cfg.aOrigin + '/'); await next.getByRole('link', { name: /Sign in with Yufu/ }).click()
      await visibleText(next, '.auth-visual h2', '欢迎进入企业工作空间')
      await next.route('**/api/v1/site/branding', route => route.abort())
      await next.goto(cfg.opOrigin + '/login'); await visibleText(next, '.site-brand-name', 'yufu')
      await next.getByLabel('用户名', { exact: true }).fill(cfg.username); await next.getByLabel('密码', { exact: true }).fill(cfg.password)
      await next.getByRole('button', { name: '登录', exact: true }).click(); await next.waitForURL('**/account')
      await next.unroute('**/api/v1/site/branding'); await next.reload(); await visibleText(next, '.site-brand-name', '企业身份平台 ' + engine)
    } finally { await fresh.close() }
    // A separate unsaved draft must survive a conflicting save.
    const token = await csrf(context, cfg.opOrigin)
    const current = await (await context.request.get(cfg.opOrigin + '/api/v1/admin/site-settings')).json()
    const external = await context.request.put(cfg.opOrigin + '/api/v1/admin/site-settings', { headers: { origin: cfg.opOrigin, 'x-csrf-token': token }, data: { expected_revision: current.settings.revision, document: current.settings.document } })
    assert.equal(external.status(), 200)
    await page.getByLabel('网站名称', { exact: true }).fill('保留的冲突草稿')
    const conflict = page.waitForResponse(r => r.request().method() === 'PUT' && r.url().endsWith('/api/v1/admin/site-settings'))
    await page.getByRole('button', { name: '保存设置', exact: true }).click(); assert.equal((await conflict).status(), 409)
    await visibleText(page, '[role="alert"]', '草稿已保留'); assert.equal(await page.getByLabel('网站名称', { exact: true }).inputValue(), '保留的冲突草稿')
    await awaitConsentIfShown(oidc, cfg)
  } finally {
    if (changed) {
      const token = await csrf(context, cfg.opOrigin)
      const current = await (await context.request.get(cfg.opOrigin + '/api/v1/admin/site-settings')).json()
      const reset = await context.request.post(cfg.opOrigin + '/api/v1/admin/site-settings/reset', { headers: { origin: cfg.opOrigin, 'x-csrf-token': token }, data: { expected_revision: current.settings.revision } })
      assert.equal(reset.status(), 200)
    }
    await visitor.close(); await context.close()
  }
}
