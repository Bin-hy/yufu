const { chromium } = require('/tmp/cobotusr-audit-tools/node_modules/playwright');
const fs = require('node:fs');

(async () => {
  const browser = await chromium.launch({ channel: 'chrome', headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 960 } });
  const page = await context.newPage();
  const requests = [];
  const errors = [];
  let revokeMode = false;
  let raceMode = false;
  const role = { id: 1, name: '管理员', description: '平台管理员' };
  const user = { id: 1, username: 'audit-user', nickname: '审查示例用户', role, created_at: '2026-11-08T08:00:00Z' };
  page.on('pageerror', e => errors.push(e.message));
  await page.route('**/*', async route => {
    const req = route.request();
    if (!['xhr', 'fetch'].includes(req.resourceType())) return route.continue();
    const url = new URL(req.url());
    requests.push({ method: req.method(), path: url.pathname, query: url.search });
    if (revokeMode && req.method() === 'DELETE') {
      await new Promise(resolve => setTimeout(resolve, 1000));
      return route.fulfill({ status: 500, contentType: 'application/json', body: JSON.stringify('模拟撤销失败') });
    }
    let body = { data_list: [role], total_pages: 5, total_items: 50, page_number: Number(url.searchParams.get('page_number') || 1), page_size: 10 };
    if (url.pathname.endsWith('/users')) {
      const pn = Number(url.searchParams.get('page_number') || 1);
      if (raceMode) await new Promise(resolve => setTimeout(resolve, pn === 2 ? 650 : 30));
      body.data_list = [{ ...user, username: `audit-page-${pn}` }];
    }
    if (/consent|authorized/.test(url.pathname)) body = [{ consent_id: 1, client_name: '审查示例应用', authorized_at: '2026-11-08', scope: 'openid profile' }];
    return route.fulfill({ contentType: 'application/json', body: JSON.stringify(body) });
  });
  await page.goto('http://127.0.0.1:5178/login');
  await page.evaluate(async () => {
    const { setToken } = await import('/src/common/cache/cookies.ts');
    setToken('local-audit-dummy-token');
    const { router } = await import('/src/router/index.ts');
    await router.push('/dashboard');
  });
  await page.waitForTimeout(350);
  const result = {};
  const state = () => page.evaluate(async () => {
    const { useTabsStore } = await import('/src/pinia/stores/tabs.ts');
    const { router } = await import('/src/router/index.ts');
    const tabs = useTabsStore();
    return { route: router.currentRoute.value.fullPath, tabs: JSON.parse(JSON.stringify(tabs.visited)), activePath: tabs.activePath };
  });
  const navigate = path => page.evaluate(async path => {
    const { router } = await import('/src/router/index.ts');
    await router.push(path);
  }, path);
  result.home = await state();
  await page.screenshot({ path: '/tmp/cobotusr-before-dashboard.png', fullPage: true });
  const before = requests.length;
  await navigate('/users?page_number=3&page_size=10');
  await page.waitForTimeout(700);
  result.initialUserRequests = requests.slice(before).filter(r => r.path.endsWith('/users'));
  result.usersBeforeSwitch = await state();
  await page.screenshot({ path: '/tmp/cobotusr-before-users.png', fullPage: true });
  await navigate('/roles');
  await page.waitForTimeout(350);
  await page.locator('header').getByText('用户列表', { exact: true }).click();
  await page.waitForTimeout(650);
  result.usersAfterTabReturn = await state();
  await page.getByRole('button', { name: '切换侧栏' }).click();
  const sidebar = page.locator('main > aside');
  result.collapsedSidebar = await sidebar.evaluate(el => ({ width: el.getBoundingClientRect().width, visibleLinks: [...el.querySelectorAll('a')].filter(a => a.getClientRects().length).map(a => ({ text: a.textContent.trim(), href: a.getAttribute('href') })), buttons: [...el.querySelectorAll('button')].map(b => b.textContent.trim()) }));
  await sidebar.locator('button').first().click();
  result.collapsedLinksAfterClick = await sidebar.locator('a:visible').count();
  await page.getByRole('button', { name: '切换侧栏' }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: '/tmp/cobotusr-before-mobile.png', fullPage: true });
  result.mobile = await page.evaluate(() => ({ viewport: innerWidth, documentWidth: document.documentElement.scrollWidth, bodyWidth: document.body.scrollWidth, content: (() => { const r = document.querySelector('main > section').getBoundingClientRect(); return { x:r.x, width:r.width, right:r.right }; })() }));
  await page.setViewportSize({ width: 1440, height: 960 });
  raceMode = true;
  await page.getByRole('button', { name: '下一页', exact: true }).last().click();
  await page.waitForTimeout(60);
  await page.getByRole('button', { name: '下一页', exact: true }).last().click();
  await page.waitForTimeout(800);
  result.outOfOrderPagination = { state: await state(), tableText: await page.locator('table').first().innerText() };
  raceMode = false;
  await navigate('/authorized-apps');
  await page.waitForTimeout(400);
  await page.getByRole('button', { name: '撤销', exact: true }).click();
  result.dialogBeforeConfirm = { semanticDialogCount: await page.getByRole('dialog').count(), titleVisible: await page.getByText('撤销授权', { exact: true }).isVisible() };
  revokeMode = true;
  await page.getByRole('button', { name: '确认', exact: true }).click();
  await page.waitForTimeout(250);
  result.dialogWhilePending = { titleVisible: await page.getByText('撤销授权', { exact: true }).isVisible() };
  await page.waitForTimeout(1000);
  result.dialogAfterFailure = { titleVisible: await page.getByText('撤销授权', { exact: true }).isVisible(), appStillVisible: await page.getByText('审查示例应用', { exact: true }).isVisible() };
  revokeMode = false;
  await navigate('/users');
  await page.waitForTimeout(500);
  result.beforeLogout = await state();
  await page.evaluate(async () => {
    const { useUserStore } = await import('/src/pinia/stores/user.ts');
    const { router } = await import('/src/router/index.ts');
    useUserStore().logout();
    await router.push('/login');
    const { setToken } = await import('/src/common/cache/cookies.ts');
    setToken('local-audit-dummy-second-user');
    await router.push('/dashboard');
  });
  await page.waitForTimeout(350);
  result.afterSecondLogin = await state();
  result.pageErrors = errors;
  fs.writeFileSync('/tmp/cobotusr-browser-baseline.json', JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
  await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
