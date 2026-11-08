import { ref, computed } from 'vue'
import { useAsyncMutation, errorMessage } from './use-async-mutation'
import { registerRead, sessionEpoch } from '@/workspace/session'
export interface AssignmentAdapter {
    read: (signal: AbortSignal) => Promise<number[]>
    add: (ids: number[]) => Promise<unknown>
    remove: (ids: number[]) => Promise<unknown>
}
export function usePermissionAssignment(adapter: AssignmentAdapter) {
    const actual = ref<number[]>([]),
        desired = ref<number[]>([]),
        ready = ref(false),
        mutation = useAsyncMutation()
    const sorted = (ids: number[]) => [...new Set(ids)].sort((a, b) => a - b)
    const dirty = computed(
        () => JSON.stringify(sorted(actual.value)) !== JSON.stringify(sorted(desired.value)),
    )
    function initialize(ids: number[], preserve = false) {
        actual.value = sorted(ids)
        if (!preserve) desired.value = sorted(ids)
        ready.value = true
    }
    async function reconcile() {
        const controller = new AbortController(),
            unregister = registerRead(controller)
        const epoch = sessionEpoch.value
        try {
            const ids = await adapter.read(controller.signal)
            if (epoch === sessionEpoch.value) {
                initialize(ids, true)
                mutation.error.value = null
            }
        } catch (error) {
            if (epoch === sessionEpoch.value) {
                ready.value = false
                mutation.error.value = errorMessage(error)
            }
        } finally {
            unregister()
        }
    }
    async function save() {
        if (!ready.value || !dirty.value || mutation.pending.value) return false
        const intent = sorted(desired.value),
            origin = new Set(actual.value),
            epoch = sessionEpoch.value
        const added = intent.filter((id) => !origin.has(id)),
            removed = actual.value.filter((id) => !intent.includes(id))
        ready.value = false
        return mutation.run(
            async () => {
                let failure: string | null = null
                const controller = new AbortController(),
                    unregister = registerRead(controller)
                try {
                    try {
                        if (added.length) await adapter.add(added)
                        if (epoch !== sessionEpoch.value) throw Error('会话已改变')
                        if (removed.length) await adapter.remove(removed)
                    } catch (error) {
                        failure = errorMessage(error)
                    }
                    if (epoch !== sessionEpoch.value) throw Error('会话已改变')
                    const ids = await adapter.read(controller.signal)
                    return { ids, failure }
                } finally {
                    unregister()
                }
            },
            (result) => {
                initialize(result.ids, true)
                if (result.failure)
                    mutation.error.value = result.failure + '；已重新读取实际授权，请重试剩余变更。'
                else if (dirty.value)
                    mutation.error.value = '部分权限未保存，已核对实际授权，请重试剩余变更。'
            },
        )
    }
    return { actual, desired, ready, dirty, ...mutation, initialize, reconcile, save }
}
