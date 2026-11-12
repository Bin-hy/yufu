import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { nextTick } from 'vue'
import EntityPicker from '@/components/ui/entity-picker/entity-picker.vue'
import type { PickerOption } from '@/components/ui/entity-picker/types'

const options: PickerOption[] = [
    { value: 18, label: '李娜', secondary: '@li.na · #18' },
    { value: 25, label: '王强', secondary: '@wang.qiang · #25' },
]
let wrappers: VueWrapper[] = []

beforeEach(() => vi.useFakeTimers())
afterEach(() => {
    for (const wrapper of wrappers) wrapper.unmount()
    wrappers = []
    vi.useRealTimers()
    document.body.innerHTML = ''
})

async function render() {
    const load = vi.fn(async () => options)
    const wrapper = mount(EntityPicker, {
        attachTo: document.body,
        props: { load, modelValue: null, placeholder: '搜索账户' },
    })
    wrappers.push(wrapper)
    return { wrapper, load }
}

describe('可搜索对象选择器', () => {
    it('防抖搜索后展示可读名称、用户名与唯一 ID，并发出选择', async () => {
        const { wrapper, load } = await render()
        const input = wrapper.get('input')
        await input.trigger('focus')
        await input.setValue('李')
        await vi.advanceTimersByTimeAsync(100)
        expect(load).toHaveBeenCalledTimes(1)
        await vi.advanceTimersByTimeAsync(150)
        await flushPromises()
        expect(load).toHaveBeenCalledTimes(2)
        expect(load).toHaveBeenLastCalledWith('李', expect.any(AbortSignal))
        const option = document.querySelector<HTMLButtonElement>('.entity-picker-option')
        expect(option?.textContent).toContain('李娜')
        expect(option?.textContent).toContain('@li.na')
        expect(option?.textContent).toContain('#18')
        option?.click()
        await nextTick()
        expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([18])
        expect(wrapper.emitted('select')?.[0]?.[0]).toMatchObject({ value: 18, label: '李娜' })
    })

    it('已选择对象显示可更换与清除操作，清除发出 null', async () => {
        const load = vi.fn(async () => options)
        const wrapper = mount(EntityPicker, {
            attachTo: document.body,
            props: {
                load,
                modelValue: 18,
                selectedLabel: '李娜',
                selectedSecondary: '@li.na · #18',
            },
        })
        wrappers.push(wrapper)
        expect(wrapper.text()).toContain('李娜')
        expect(wrapper.text()).toContain('@li.na · #18')
        await wrapper.get('[aria-label="清除选择"]').trigger('click')
        expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([null])
    })

    it('键盘上下键与回车可以选择当前项', async () => {
        const { wrapper } = await render()
        const input = wrapper.get('input')
        await input.trigger('focus')
        await vi.advanceTimersByTimeAsync(250)
        await flushPromises()
        await input.trigger('keydown', { key: 'ArrowDown' })
        await input.trigger('keydown', { key: 'Enter' })
        expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([25])
    })
})
