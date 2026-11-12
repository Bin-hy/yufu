import { describe, it, expect, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import { defineComponent, ref, nextTick, effectScope } from 'vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
import UiSelect from '@/components/ui/select/select.vue'
import Pagination from '@/components/ui/pagination/pagination.vue'
import { useFormDraft } from '@/composables/use-form-draft'
import { createStepWorkflow } from '@/composables/use-step-workflow'
import { errorMessage } from '@/composables/use-async-mutation'
import { disposeTheme, initializeTheme, useTheme } from '@/composables/use-theme'
const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/', name: 'form', component: {} }],
})
describe('接口错误提示', () => {
    it('无法解析的 503 代码给出可重试的中文提示', () => {
        expect(
            errorMessage({
                response: { data: { code: 'deadline_exceeded', request_id: 'req-1' } },
            }),
        ).toBe('认证服务响应超时，请稍后重试')
        expect(
            errorMessage({
                response: { data: { code: 'dependency_unavailable' } },
            }),
        ).toBe('认证服务暂时不可用，请稍后重试')
    })
    it('优先使用服务端明确消息', () => {
        expect(
            errorMessage({ response: { data: { message: '账号或密码不正确' } } }),
        ).toBe('账号或密码不正确')
    })
})
describe('表单值与分页', () => {
    it('普通按钮默认为 button，显式 submit 保留', () => {
        expect(mount(UiButton).attributes('type')).toBe('button')
        expect(mount(UiButton, { props: { type: 'submit' } }).attributes('type')).toBe('submit')
    })
    it('密码输入可切换可见性，清除按钮发出空值', async () => {
        const password = mount(UiInput, { props: { modelValue: 'secret', type: 'password' } })
        expect(password.find('input').attributes('type')).toBe('password')
        await password.find('[aria-label="显示密码"]').trigger('click')
        expect(password.find('input').attributes('type')).toBe('text')
        await password.find('[aria-label="隐藏密码"]').trigger('click')
        expect(password.find('input').attributes('type')).toBe('password')
        const clearable = mount(UiInput, { props: { modelValue: 'hello', clearable: true } })
        await clearable.find('[aria-label="清除输入"]').trigger('click')
        expect(clearable.emitted('update:modelValue')?.[0]).toEqual([''])
    })
    it('按钮 loading 时禁用并暴露 busy 状态', () => {
        const wrapper = mount(UiButton, { props: { loading: true } })
        expect(wrapper.attributes('disabled')).toBeDefined()
        expect(wrapper.attributes('aria-busy')).toBe('true')
        expect(wrapper.find('.ui-button-spinner').exists()).toBe(true)
    })
    it('表单字段错误使用明确 id 并声明 aria-live 角色', () => {
        const wrapper = mount(UiFormField, {
            props: { id: 'account-email', label: '邮箱', error: '格式不正确' },
            slots: { default: '<input id="account-email" />' },
        })
        const error = wrapper.find('#account-email-error')
        expect(error.text()).toContain('格式不正确')
        expect(error.attributes('role')).toBe('alert')
        expect(wrapper.find('label').attributes('for')).toBe('account-email')
    })
    it('select 数值与 null 不泄漏为字符串', async () => {
        const wrapper = mount(UiSelect, {
            props: { modelValue: null },
            slots: { default: '<option value="null">全部</option><option value="2">二</option>' },
        })
        await wrapper.setValue('2')
        expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['2'])
        const numeric = mount(UiSelect, {
            props: { modelValue: 1, modelModifiers: { number: true } },
            slots: { default: '<option value="1">一</option><option value="2">二</option>' },
        })
        await numeric.setValue('2')
        expect(numeric.emitted('update:modelValue')?.[0]).toEqual([2])
    })
    it('分页仅发出一个页码事件，全部控件受 disabled 约束', async () => {
        const wrapper = mount(Pagination, { props: { page: 1, pageCount: 3, size: 10 } })
        await wrapper.find('[aria-label="下一页"]').trigger('click')
        expect(wrapper.emitted('page-change')).toEqual([[2]])
        expect(wrapper.emitted('change')).toBeUndefined()
        await wrapper.setProps({ disabled: true })
        expect(
            wrapper
                .findAll('button')
                .every((button) => button.attributes('disabled') !== undefined),
        ).toBe(true)
        expect(wrapper.find('select').attributes('disabled')).toBeDefined()
    })
    it('异步默认值只重建该字段基线，用户已输入内容仍为脏', async () => {
        const form = ref({ name: '', role_id: 0 })
        let draft!: ReturnType<typeof useFormDraft>
        const wrapper = mount(
            defineComponent({
                setup() {
                    draft = useFormDraft('form', () => form.value)
                    return () => null
                },
            }),
            { global: { plugins: [router] } },
        )
        form.value.name = 'draft'
        form.value.role_id = 1
        draft.rebase({ role_id: 1 })
        await nextTick()
        expect(draft.dirty.value).toBe(true)
        draft.markSaved()
        expect(draft.dirty.value).toBe(false)
        wrapper.unmount()
    })
})
it('应用分步创建/更新/删除第二步失败后不重复第一步', async () => {
    const first = vi.fn().mockResolvedValue(undefined),
        second = vi.fn().mockRejectedValueOnce(Error('failed')).mockResolvedValue(undefined),
        flow = createStepWorkflow([
            { id: 'first', label: 'first', run: first },
            { id: 'second', label: 'second', run: second },
        ])
    await expect(flow.run()).rejects.toThrow('failed')
    expect(flow.completed.value).toEqual(['first'])
    await flow.run()
    expect(first).toHaveBeenCalledTimes(1)
    expect(second).toHaveBeenCalledTimes(2)
})
it('主题可启动、恢复、切换；监听可释放且不会重复', async () => {
    disposeTheme()
    const add = vi.fn(),
        remove = vi.fn()
    vi.spyOn(window, 'matchMedia').mockReturnValue({
        matches: true,
        addEventListener: add,
        removeEventListener: remove,
    } as unknown as MediaQueryList)
    localStorage.setItem('yufu:theme', 'system')
    const scope = effectScope()
    scope.run(initializeTheme)
    initializeTheme()
    expect(add).toHaveBeenCalledTimes(1)
    expect(document.documentElement.classList.contains('dark')).toBe(true)
    useTheme().setTheme('light')
    await flushPromises()
    expect(document.documentElement.classList.contains('dark')).toBe(false)
    disposeTheme()
    expect(remove).toHaveBeenCalledTimes(1)
    scope.stop()
})
