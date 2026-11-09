import { ref } from 'vue'
import { usePageContext } from '@/workspace/page-context'
import { useAsyncQuery } from './use-async-query'
import type { Page } from '@/pages/account/api/type'
export function useCursorPage<T, C extends string | number = string>(key: string, loader: (after: C | undefined, signal: AbortSignal) => Promise<Page<T, C>>) {
    const context = usePageContext()
    const after = ref<C>()
    const state = useAsyncQuery({ key: () => `${key}:${after.value ?? ''}`, active: context.active, loader: signal => loader(after.value as C | undefined, signal) })
    return { ...state, next: () => { if (state.data.value?.next != null) after.value = state.data.value.next }, first: () => { after.value = undefined; state.invalidate() } }
}
