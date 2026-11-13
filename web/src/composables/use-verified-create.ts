import { ref } from 'vue'
import { sessionEpoch } from '@/workspace/session'
import { normalizeFailure, UncertainSubmissionError } from '@/http/errors'

export function createVerifiedCreate<T>(write: () => Promise<T>, find: () => Promise<T | null>) {
    const uncertain = ref(false)
    let uncertainCause: unknown
    async function run(): Promise<T> {
        const epoch = sessionEpoch.value
        if (!uncertain.value) {
            try {
                return await write()
            } catch (error) {
                const failure = error as { response?: unknown; code?: string }
                const config = (error as { config?: { method?: string } })?.config
                if (!normalizeFailure(error, config?.method ? {} : { operation: 'write' }).outcomeUncertain
                    || (!failure.response && !['ECONNABORTED', 'ETIMEDOUT', 'ERR_NETWORK'].includes(failure.code ?? '')))
                    throw error
                uncertainCause = error
                uncertain.value = true
            }
        }
        if (epoch !== sessionEpoch.value) throw Error('会话已改变')
        let result: T | null = null
        try {
            result = await find()
        } catch {
            /* 保持结果未知，不重发创建 */
        }
        if (epoch !== sessionEpoch.value) throw Error('会话已改变')
        if (result !== null) {
            uncertain.value = false
            return result
        }
        throw new UncertainSubmissionError(uncertainCause)
    }
    return { uncertain, run }
}
