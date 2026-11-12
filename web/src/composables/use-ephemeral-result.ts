import { shallowRef, onDeactivated, onScopeDispose, type ShallowRef } from 'vue'
import { onSessionReset } from '@/workspace/session'
export function useEphemeralResult<T>(): { value: ShallowRef<T | null>; clear: () => void } {
    const value = shallowRef<T | null>(null)
    const clear = () => { value.value = null }
    onDeactivated(clear)
    onScopeDispose(onSessionReset(clear))
    onScopeDispose(clear)
    return { value, clear }
}
export function useOperationKey() {
    let previous = '', key = crypto.randomUUID()
    return (input: unknown) => {
        const current = JSON.stringify(input)
        if (current !== previous) { previous = current; key = crypto.randomUUID() }
        return key
    }
}
