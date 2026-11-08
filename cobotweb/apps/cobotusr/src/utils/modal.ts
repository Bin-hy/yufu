import { shallowRef, ref } from 'vue'
import { onSessionReset } from '@/workspace/session'
import { captureFocusOrigin } from '@/utils/focus'
export interface ModalOptions {
    title: string
    content: string
    confirmText?: string
    cancelText?: string
    destructive?: boolean
    confirmCallback?: () => unknown | Promise<unknown>
    cancleCallback?: () => unknown
}
interface Confirmation {
    options: ModalOptions
    resolve: (value: boolean) => void
    trigger: HTMLElement | null
}
export const currentConfirmation = shallowRef<Confirmation | null>(null)
export const confirmationBusy = ref(false)
const queue: Confirmation[] = []
export function showModal(options: ModalOptions): Promise<boolean> {
    return new Promise((resolve) => {
        queue.push({
            options,
            resolve,
            trigger: captureFocusOrigin(),
        })
        next()
    })
}
function next() {
    if (!currentConfirmation.value) currentConfirmation.value = queue.shift() ?? null
}
export function settleConfirmation(value: boolean) {
    const current = currentConfirmation.value
    if (!current) return
    currentConfirmation.value = null
    confirmationBusy.value = false
    current.resolve(value)
    queueMicrotask(() => {
        if (current.trigger?.isConnected) current.trigger.focus()
        next()
    })
}
export function cleanupConfirmations() {
    confirmationBusy.value = false
    currentConfirmation.value?.resolve(false)
    currentConfirmation.value = null
    for (const current of queue.splice(0)) current.resolve(false)
}
onSessionReset(cleanupConfirmations)
