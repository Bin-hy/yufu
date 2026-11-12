import {
    ref,
    shallowRef,
    watch,
    onScopeDispose,
    toValue,
    type MaybeRefOrGetter,
    type Ref,
    type ShallowRef,
} from 'vue'
import { sessionEpoch, registerRead } from '@/workspace/session'
import { errorMessage } from './use-async-mutation'
export interface AsyncQuery<T> {
    data: ShallowRef<T | null>
    status: Ref<'idle' | 'loading' | 'ready' | 'error'>
    refreshing: Ref<boolean>
    error: Ref<string | null>
    stale: Ref<boolean>
    refresh: () => Promise<void>
    invalidate: () => void
}
export function useAsyncQuery<T>(options: {
    key: MaybeRefOrGetter<string>
    active?: MaybeRefOrGetter<boolean>
    loader: (signal: AbortSignal) => Promise<T>
}): AsyncQuery<T> {
    const data = shallowRef<T | null>(null)
    const status = ref<'idle' | 'loading' | 'ready' | 'error'>('idle')
    const refreshing = ref(false)
    const error = ref<string | null>(null)
    const stale = ref(true)
    let sequence = 0,
        loadedKey = ''
    let controller: AbortController | null = null
    let unregister: (() => void) | null = null
    const active = () => options.active === undefined || toValue(options.active)
    function cancel() {
        sequence++
        controller?.abort()
        unregister?.()
        controller = null
        unregister = null
    }
    async function refresh() {
        if (!active()) {
            stale.value = true
            return
        }
        cancel()
        const id = sequence,
            epoch = sessionEpoch.value,
            key = toValue(options.key)
        controller = new AbortController()
        const signal = controller.signal
        unregister = registerRead(controller)
        const preserving = loadedKey === key && data.value !== null
        if (!preserving) {
            data.value = null
            status.value = 'loading'
        }
        refreshing.value = preserving
        error.value = null
        const valid = () => id === sequence && epoch === sessionEpoch.value && !signal.aborted
        try {
            const result = await options.loader(signal)
            if (!valid()) return
            data.value = result
            loadedKey = key
            status.value = 'ready'
            stale.value = false
        } catch (e) {
            if (!valid()) return
            error.value = errorMessage(e)
            status.value = preserving ? 'ready' : 'error'
            stale.value = true
        } finally {
            if (valid()) {
                refreshing.value = false
                unregister?.()
                unregister = null
                controller = null
            }
        }
    }
    watch(
        () => [toValue(options.key), active(), sessionEpoch.value] as const,
        ([key, enabled, epoch], old) => {
            if (old && old[2] !== epoch) {
                data.value = null
                loadedKey = ''
                stale.value = true
            }
            if (!enabled) {
                if (controller) stale.value = true
                cancel()
                if (status.value === 'loading') {
                    status.value = 'idle'
                    stale.value = true
                }
                refreshing.value = false
                return
            }
            if (!old || key !== loadedKey || stale.value) void refresh()
        },
        { immediate: true },
    )
    onScopeDispose(cancel)
    return {
        data,
        status,
        refreshing,
        error,
        stale,
        refresh,
        invalidate: () => {
            stale.value = true
            if (active()) void refresh()
        },
    }
}
