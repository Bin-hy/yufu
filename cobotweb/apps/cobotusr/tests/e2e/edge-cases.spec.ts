import { test, expect, stamp, keyboardTab } from './fixture'
test('资源授权完整多页基线与增量候选，部分失败不删除未显示项目', async ({ page, mock }) => {
    mock.permissions.splice(
        0,
        mock.permissions.length,
        ...Array.from({ length: 210 }, (_, i) => ({
            ...mock.permissions[0],
            id: i + 1,
            name: '资源权限 ' + (i + 1),
            application_id: 1,
            action: 'read',
            resource_type: 'document',
            resource_pattern: 'document:*',
            create_time: stamp,
            update_time: stamp,
        })),
    )
    mock.assignedPermissions.clear()
    for (let id = 1; id <= 205; id++) if (id !== 3) mock.assignedPermissions.add(id)
    mock.failures.set('DELETE /role_permission/batch', 1)
    await page.goto('/permissions/assign?roleId=1')
    await expect(page.getByRole('checkbox')).toHaveCount(100)
    await expect(page.getByText(/已选 204 项 · 已授权 204 项/)).toBeVisible()
    await page.getByRole('checkbox').nth(0).uncheck()
    await page.getByRole('checkbox').nth(2).check()
    await page.getByRole('button', { name: '加载更多权限', exact: true }).click()
    await expect(page.getByRole('checkbox')).toHaveCount(200)
    await page.getByRole('button', { name: '加载更多权限', exact: true }).click()
    await expect(page.getByRole('checkbox')).toHaveCount(210)
    await expect(page.getByRole('checkbox').nth(204)).toBeChecked()
    await page.getByRole('button', { name: '保存权限', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText('已重新读取实际授权')
    await page.getByRole('button', { name: '保存权限', exact: true }).click()
    await expect(page.getByRole('button', { name: '保存权限', exact: true })).toBeDisabled()
    expect(mock.assignedPermissions.has(205)).toBe(true)
    expect(mock.assignedPermissions.has(1)).toBe(false)
    expect(mock.assignedPermissions.has(3)).toBe(true)
    expect(mock.calls.filter((c) => c.path === '/role_permission/batch' && c.method === 'POST')).toHaveLength(1)
    expect(mock.calls.filter((c) => c.path === '/role_permission/batch' && c.method === 'DELETE')).toHaveLength(2)
})
test('API 授权 8105 项完整读取；中间页失败禁用保存，重试恢复', async ({ page, mock }) => {
    mock.authorities.splice(
        0,
        mock.authorities.length,
        ...Array.from({ length: 8105 }, (_, i) => ({
            id: i + 1,
            name: '接口权限 ' + (i + 1),
            authority_uri: '/service/' + (i + 1),
            create_time: stamp,
            update_time: stamp,
        })),
    )
    mock.assigned.clear()
    for (let id = 1; id <= 8105; id++) mock.assigned.add(id)
    mock.statuses.set('GET /role_authority/1?page_number=2&page_size=100', {
        status: 503,
        remaining: 1,
    })
    await page.goto('/authority/assign?roleId=1')
    await expect(page.getByRole('heading', { name: '加载失败' })).toBeVisible()
    await expect(page.getByRole('button', { name: '保存权限', exact: true })).toBeDisabled()
    await page.getByRole('button', { name: '重新加载', exact: true }).click()
    await expect(page.getByText(/已选 8105 项 · 已授权 8105 项/)).toBeVisible()
    await expect(page.getByRole('checkbox')).toHaveCount(100)
    await page.getByRole('checkbox').nth(0).uncheck()
    await page.getByRole('button', { name: '保存权限', exact: true }).click()
    await expect(page.getByRole('button', { name: '保存权限', exact: true })).toBeDisabled()
    expect(mock.assigned.has(8105)).toBe(true)
    expect(mock.calls.find((c) => c.path === '/role_authority/batch')?.body.authority_ids).toEqual([1])
})
test('部分保存后实际状态再次读取失败，禁止盲目重试，核对后继续', async ({ page, mock }) => {
    await page.goto('/permissions/assign?roleId=1')
    await expect(page.getByRole('checkbox')).toHaveCount(4)
    await page.getByRole('checkbox').nth(0).uncheck()
    mock.failures.set('GET /role_permission/1', 1)
    await page.getByRole('button', { name: '保存权限', exact: true }).click()
    await expect(page.getByRole('button', { name: '保存权限', exact: true })).toBeDisabled()
    await page.getByRole('button', { name: '重新核对授权', exact: true }).click()
    await expect(page.getByRole('button', { name: '保存权限', exact: true })).toBeDisabled()
    expect(mock.calls.filter((c) => c.path === '/role_permission/batch' && c.method === 'DELETE')).toHaveLength(1)
})
test('客户端已创建但响应丢失：核对一次并完成关联，说明密钥未收到', async ({ page, mock }) => {
    mock.lostResponses.set('POST /oidc/clients', 1)
    await page.goto('/applications')
    await page.getByRole('button', { name: '注册应用', exact: true }).click()
    await page.getByLabel('应用名称', { exact: true }).fill('响应丢失应用')
    await page.getByLabel('授权回调地址', { exact: true }).fill('https://example.test/new-callback')
    await page.getByRole('dialog').getByRole('button', { name: '保存应用', exact: true }).click()
    await expect(
        page.getByRole('dialog').getByText('客户端创建结果已核对，但一次性密钥未收到。注册完成后请重新生成密钥。'),
    ).toBeVisible()
    await page.getByRole('button', { name: '已保存，完成', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    expect(mock.calls.filter((c) => c.path === '/oidc/clients' && c.method === 'POST')).toHaveLength(1)
    expect(mock.applications).toHaveLength(2)
})
test('创建结果无法确认时重试只核对，应用关联响应丢失也不重发', async ({ page, mock }) => {
    mock.lostBeforeCommit.set('POST /oidc/clients', 1)
    await page.goto('/applications')
    await page.getByRole('button', { name: '注册应用', exact: true }).click()
    await page.getByLabel('应用名称', { exact: true }).fill('未知结果应用')
    await page.getByLabel('授权回调地址', { exact: true }).fill('https://example.test/unknown')
    await page.getByRole('dialog').getByRole('button', { name: '保存应用', exact: true }).click()
    await expect(page.getByRole('dialog').getByRole('alert')).toContainText('创建结果未知')
    await page.getByRole('button', { name: '重试剩余步骤', exact: true }).click()
    await expect(page.getByRole('dialog').getByRole('alert')).toContainText('创建结果未知')
    expect(mock.calls.filter((c) => c.path === '/oidc/clients' && c.method === 'POST')).toHaveLength(1)
    await page.getByRole('dialog').getByRole('button', { name: '取消', exact: true }).click()
    await page.getByRole('alertdialog').getByRole('button', { name: '放弃修改', exact: true }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    mock.lostResponses.set('POST /application', 1)
    await page.getByRole('button', { name: '注册应用', exact: true }).click()
    await page.getByLabel('应用名称', { exact: true }).fill('关联响应丢失')
    await page.getByLabel('授权回调地址', { exact: true }).fill('https://example.test/link-lost')
    await page.getByRole('dialog').getByRole('button', { name: '保存应用', exact: true }).click()
    await expect(page.getByRole('button', { name: '已保存，完成', exact: true })).toBeVisible()
    expect(mock.calls.filter((c) => c.path === '/application' && c.method === 'POST')).toHaveLength(1)
})
test('删除触发对象后焦点回正文；仅键盘可完成编辑与菜单', async ({ page, browserName }) => {
    await page.goto('/authorized-apps')
    await page.getByRole('button', { name: '撤销', exact: true }).focus()
    await page.keyboard.press('Enter')
    await expect(page.getByRole('alertdialog').getByRole('button', { name: '取消', exact: true })).toBeFocused()
    await keyboardTab(page, browserName)
    await page.keyboard.press('Enter')
    await expect(page.getByRole('alertdialog')).toHaveCount(0)
    await expect(page.locator('#workspace-content')).toBeFocused()
    await page.locator('.workspace-sidebar').getByRole('link', { name: '角色管理', exact: true }).click()
    await page.getByRole('button', { name: '创建角色', exact: true }).focus()
    await page.keyboard.press('Enter')
    await expect(page.getByLabel('角色名称', { exact: true })).toBeFocused()
    await page.keyboard.type('键盘角色')
    await page.keyboard.press('Enter')
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await page.getByRole('button', { name: '用户菜单' }).focus()
    await page.keyboard.press('Enter')
    await page.keyboard.press('ArrowDown')
    await page.keyboard.press('Escape')
    await expect(page.getByRole('button', { name: '用户菜单' })).toBeFocused()
})
