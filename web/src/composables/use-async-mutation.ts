import { ref, shallowRef } from 'vue'
import { sessionEpoch } from '@/workspace/session'
import { normalizeFailure, formatFailure, type BusinessFailure } from '@/http/errors'
import { showModal, type ModalOptions } from '@/utils/modal'

export function errorMessage(error: unknown): string {
    return formatFailure(normalizeFailure(error))
}

export function useAsyncMutation() {
    const pending = ref(false)
    const error = ref<string | null>(null)
    const failure = shallowRef<BusinessFailure | null>(null)
    let lastError: unknown
    function clearError() {
        error.value = null
        failure.value = null
        lastError = undefined
    }
    async function execute<T>(loader: () => Promise<T>): Promise<T> {
        if (pending.value) throw new DOMException('请求正在执行', 'AbortError')
        if (failure.value?.outcomeUncertain) throw lastError
        const epoch = sessionEpoch.value
        pending.value = true
        clearError()
        try {
            const result = await loader()
            if (epoch !== sessionEpoch.value) throw new DOMException('会话已改变', 'AbortError')
            return result
        } catch (e) {
            if (epoch === sessionEpoch.value) {
                const config = (e as { config?: { url?: string; method?: string } } | null)?.config
                const result = normalizeFailure(e, config?.method || /(?:^|\/)login$/.test(config?.url ?? '') ? {} : { operation: 'write' })
                if (!result.cancelled) {
                    lastError = e
                    failure.value = result
                    error.value = formatFailure(result)
                }
            }
            throw e
        } finally {
            pending.value = false
        }
    }
    async function run<T>(loader: () => Promise<T>, success?: (data: T) => void | Promise<void>): Promise<boolean> {
        if (pending.value || failure.value?.outcomeUncertain) return false
        let result: T
        try { result = await execute(loader) } catch { return false }
        const epoch = sessionEpoch.value
        pending.value = true
        try { await success?.(result) } catch (e) {
            if (epoch === sessionEpoch.value) {
                const followUp = normalizeFailure(e, { operation: 'read' })
                if (!followUp.cancelled) {
                    failure.value = { ...followUp, title: '操作已完成，后续处理失败', outcomeUncertain: false, recovery: 'reload' }
                    error.value = '操作已完成，但后续处理失败。请重新加载核对结果，勿重复提交。' + formatFailure(followUp)
                }
            }
        } finally { pending.value = false }
        return true
    }
    async function confirm(options: Omit<ModalOptions, 'confirmCallback'>, loader: () => Promise<unknown>): Promise<boolean> {
        if (pending.value || failure.value?.outcomeUncertain) return false
        return showModal({ ...options, confirmCallback: () => execute(loader) })
    }
    return { pending, error, failure, clearError, run, confirm }
}
