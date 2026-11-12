import { computed, onScopeDispose, ref, toValue, type MaybeRefOrGetter } from 'vue'
import { usePageContext } from '@/workspace/page-context'
import { registerDraft } from '@/workspace/draft-registry'
let serial = 0
export function useFormDraft<T>(
    title: string,
    value: () => T,
    options: {
        enabled?: MaybeRefOrGetter<boolean>
        submitting?: MaybeRefOrGetter<boolean>
        discard?: () => void
    } = {},
) {
    const context = usePageContext()
    const baseline = ref(JSON.stringify(value()))
    const initialized = ref(true)
    const dirty = computed(
        () =>
            initialized.value &&
            (options.enabled === undefined || toValue(options.enabled)) &&
            JSON.stringify(value()) !== baseline.value,
    )
    const markSaved = () => {
        baseline.value = JSON.stringify(value())
        initialized.value = true
    }
    const dispose = registerDraft({
        id: context.key + ':form:' + ++serial,
        tabKey: context.key,
        title,
        dirty: () => dirty.value,
        submitting: () => options.submitting !== undefined && toValue(options.submitting),
        discard: () => {
            options.discard?.()
            markSaved()
        },
    })
    onScopeDispose(dispose)
    return {
        dirty,
        markSaved,
        setBaseline: (next: T) => {
            baseline.value = JSON.stringify(next)
            initialized.value = true
        },
        rebase: (patch: Partial<T>) => {
            baseline.value = JSON.stringify({ ...JSON.parse(baseline.value), ...patch })
        },
        suspend: () => {
            initialized.value = false
        },
        async confirmDiscard() {
            if (options.submitting !== undefined && toValue(options.submitting)) return false
            if (!dirty.value) return true
            const { showModal } = await import('@/utils/modal')
            return showModal({
                title: '放弃未保存的修改？',
                content: title + '尚未保存，关闭后输入将丢失。',
                confirmText: '放弃修改',
                cancelText: '继续编辑',
                destructive: true,
            })
        },
    }
}
