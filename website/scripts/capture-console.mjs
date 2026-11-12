// Capture the real Vue UI with isolated, fictional API responses.
// Start Vite on 127.0.0.1:5174 before running this script.
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
const require = createRequire(new URL('../../web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const output = fileURLToPath(new URL('../src/assets/', import.meta.url));
const origin = 'http://127.0.0.1:5174';
const now = Date.parse('2026-11-12T02:00:00Z');
const session = {
  user_id: 1, sub: 'demo-admin', username: 'demo.admin', nickname: '演示管理员',
  email: 'admin@example.test', email_verified: false, person_kind: 'employee',
  developer_enabled: true, administrator: true, sso_session_id: 'demo-session',
  auth_time: now / 1000, expires_at: now / 1000 + 28800, csrf_token: 'demo-csrf-not-a-credential',
};
const accounts = [
  { user_id: 1, username: 'demo.admin', nickname: '演示管理员', enabled: true, person_kind: 'employee', developer_enabled: true, revision: 1 },
  { user_id: 2, username: 'demo.developer', nickname: '演示开发者', enabled: true, person_kind: 'employee', developer_enabled: true, revision: 1 },
  { user_id: 3, username: 'demo.member', nickname: '演示成员', enabled: true, person_kind: 'employee', developer_enabled: false, revision: 1 },
  { user_id: 4, username: 'demo.partner', nickname: '演示合作方', enabled: false, person_kind: 'collaborator', developer_enabled: false, revision: 1 },
];
const config = { client_kind: 'server_web', redirect_uris: ['https://app.example.test/callback'], post_logout_redirect_uris: ['https://app.example.test/'], browser_origins: [], access_audience: 'urn:demo:app', identity_scopes: 'openid profile email', consent_policy: 'user_consent', logout_capability: 'online', backchannel_logout_uri: null };
const apps = ['协作控制台（演示）', '知识库（演示）'].map((name, index) => ({
  application_id: index + 1, app_key: ['demo-workspace', 'demo-knowledge'][index], name,
  description: ['团队协作与项目管理', '企业内部知识与文档'][index], management_mode: 'developer_managed',
  third_party: false, active: true, revision: 1, owner_user_id: 1, members: [],
  environments: ['development', 'production'].map((kind) => ({
    environment_id: `demo-${index}-${kind}`, kind, client_id: `demo-${index}-${kind}`,
    primary_client_kind: 'server_web', secret_revision: 1, revision: 1, lifecycle_epoch: 1,
    active_version_id: kind === 'development' || index === 1 ? 'demo-v1' : null,
    latest: { version_id: 'demo-v1', version: 1, config, candidate_client_id: null },
    active_config: config, pending_approval: kind === 'production' && index === 0 ? 'demo-approval' : null,
  })),
}));
const fixtures = {
  '/api/v1/account/session': session,
  '/api/v1/admin/accounts': { items: accounts, next: null },
  '/api/v1/developer/apps': { items: apps, next: null },
  '/api/v1/identity/flows/website-demo': {
    flow_id: 'website-demo', application_name: '协作控制台（演示）', scopes: 'openid profile email',
    interaction: 'consent', expires_at: now / 1000 + 600, username: 'demo.admin', csrf_token: 'demo-csrf-not-a-credential',
  },
};
const browser = await chromium.launch(process.env.CAPTURE_CHROME_CHANNEL ? { channel: process.env.CAPTURE_CHROME_CHANNEL } : {});
try {
  for (const [path, filename, heading] of [
    ['/dashboard', 'console-dashboard.png', '工作台'],
    ['/account', 'console-account.png', '账户中心'],
    ['/developer/apps', 'console-developer.png', '开发者控制台'],
    ['/admin/accounts', 'console-admin.png', '企业账户与邀请'],
    ['/oidc/consent?flow_id=website-demo', 'console-consent.png', '授权 协作控制台（演示）'],
  ]) {
    const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1, locale: 'zh-CN', timezoneId: 'Asia/Shanghai', colorScheme: 'light', reducedMotion: 'reduce', serviceWorkers: 'block' });
    const unexpected = [];
    const page = await context.newPage();
    await page.clock.install({ time: now });
    await page.addInitScript(() => localStorage.setItem('yufu:theme', 'light'));
    await context.route('**/*', async (route) => {
      const request = route.request();
      const url = new URL(request.url());
      if (url.origin !== origin) { unexpected.push(request.url()); return route.abort(); }
      if (!['GET', 'HEAD'].includes(request.method())) { unexpected.push(request.method() + ' ' + url.pathname); return route.abort(); }
      if (url.pathname.startsWith('/api/') || url.pathname.startsWith('/.well-known/')) {
        if (!(url.pathname in fixtures)) { unexpected.push(url.pathname); return route.abort(); }
        return route.fulfill({ json: fixtures[url.pathname] });
      }
      return route.continue();
    });
    page.on('pageerror', (error) => unexpected.push(error.message));
    await page.goto(origin + path);
    await page.getByRole('heading', { name: heading, exact: true }).waitFor();
    if (path === '/admin/accounts') await page.getByText('demo.partner · #4', { exact: true }).first().waitFor();
    if (path === '/developer/apps') await page.getByText('知识库（演示）', { exact: true }).first().waitFor();
    if (path.startsWith('/oidc/')) await page.getByRole('button', { name: '同意授权' }).waitFor();
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: resolve(output, filename), animations: 'disabled' });
    if (unexpected.length) throw new Error(JSON.stringify(unexpected));
    console.log(`${filename} ← ${path}`);
    await context.close();
  }
} finally { await browser.close(); }
