import { afterEach, expect, it, vi } from 'vitest'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import Apps from '@/pages/account/views/apps.vue'
const mocked = vi.hoisted(() => ({ getApplications: vi.fn() }))
vi.mock('@/pages/account/api', () => mocked)
let wrapper: VueWrapper | undefined
async function render() {
    const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/account/apps', name: 'apps', component: Apps }] })
    await router.push('/account/apps')
    wrapper = mount(Apps, { global: { plugins: [router] } })
    await flushPromises()
    return wrapper
}
afterEach(() => { wrapper?.unmount(); mocked.getApplications.mockReset() })
it('默认请求生产环境，展示全部来源，通过中心同源入口启动', async () => {
    mocked.getApplications.mockResolvedValue({ items: [{ environment_id: 'env-one', application_id: 1, name: 'Workbench', environment: 'production', client_id: 'client', launchable: true, launch_description: '业务工作台', sources: [{ kind: 'direct', source_id: '1', label: '直接分配' }, { kind: 'group', source_id: 'group-one', label: '研发组' }] }], next: null })
    wrapper = await render()
    expect(mocked.getApplications).toHaveBeenCalledWith(undefined, expect.any(AbortSignal), 'production', undefined)
    expect(wrapper.text()).toContain('直接分配')
    expect(wrapper.text()).toContain('用户组：研发组')
    expect(wrapper.get('a').attributes('href')).toBe('/api/v1/account/apps/env-one/launch')
    await wrapper.get('select').setValue('development')
    await flushPromises()
    expect(mocked.getApplications).toHaveBeenLastCalledWith(undefined, expect.any(AbortSignal), 'development', undefined)
})
it('没有生效启动配置时不生成跳转链接', async () => {
    mocked.getApplications.mockResolvedValue({ items: [{ environment_id: 'env-one', application_id: 1, name: 'Workbench', environment: 'production', client_id: 'client', launchable: false, sources: [] }], next: null })
    wrapper = await render()
    expect(wrapper.find('a').exists()).toBe(false)
    expect(wrapper.text()).toContain('尚未配置启动入口')
})
