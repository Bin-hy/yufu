import { describe, it, expect, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import { defineComponent, ref, nextTick, effectScope } from 'vue'
import UiButton from '@/components/ui/button/button.vue'
import UiSelect from '@/components/ui/select/select.vue'
import Pagination from '@/components/ui/pagination/pagination.vue'
import { useFormDraft } from '@/composables/use-form-draft'
import { usePermissionAssignment } from '@/composables/use-permission-assignment'
import { createStepWorkflow } from '@/composables/use-step-workflow'
import { disposeTheme, initializeTheme, useTheme } from '@/composables/use-theme'
const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/', name: 'form', component: {} }],
})
describe('表单值与分页', () => {
    it('普通按钮默认为 button，显式 submit 保留', () => {
        expect(mount(UiButton).attributes('type')).toBe('button')
        expect(mount(UiButton, { props: { type: 'submit' } }).attributes('type')).toBe('submit')
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
describe('部分成功的业务写入', () => {
    it('权限新增成功、删除失败后核对实际状态，只重试剩余删除', async () => {
        const actual = new Set([1, 2]),
            add = vi.fn(async (ids: number[]) => ids.forEach((id) => actual.add(id))),
            remove = vi
                .fn()
                .mockRejectedValueOnce(Error('delete failed'))
                .mockImplementation(async (ids: number[]) => ids.forEach((id) => actual.delete(id)))
        const assignment = usePermissionAssignment({ read: async () => [...actual], add, remove })
        assignment.initialize([1, 2])
        assignment.desired.value = [2, 3]
        await assignment.save()
        expect(assignment.actual.value).toEqual([1, 2, 3])
        expect(assignment.error.value).toContain('delete failed')
        await assignment.save()
        expect(add).toHaveBeenCalledTimes(1)
        expect(remove).toHaveBeenCalledTimes(2)
        expect(assignment.actual.value).toEqual([2, 3])
        expect(assignment.dirty.value).toBe(false)
    })
    it('授权读取失败时禁止差异写入，恢复读取后可以继续', async () => {
        const add = vi.fn(),
            read = vi.fn().mockRejectedValueOnce(Error('read failed')).mockResolvedValue([1])
        const assignment = usePermissionAssignment({ read, add, remove: vi.fn() })
        assignment.initialize([1])
        assignment.desired.value = [2]
        await assignment.reconcile()
        expect(assignment.ready.value).toBe(false)
        await assignment.save()
        expect(add).not.toHaveBeenCalled()
        await assignment.reconcile()
        expect(assignment.ready.value).toBe(true)
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
    localStorage.setItem('cobotusr:theme', 'system')
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
