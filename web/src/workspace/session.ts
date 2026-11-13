import { readonly, ref } from 'vue'
import { clearWorkspace } from '@/storage/workspace'
const epoch = ref(0)
const owner = ref<string | null>(null)
const resets = new Set<() => void>()
const reads = new Set<AbortController>()
let expire: (() => void) | null = null
export const sessionExpiredNotice = ref(false)
export const sessionEpoch = readonly(epoch)
export const sessionOwner = readonly(owner)
export function onSessionReset(callback: () => void) {
    resets.add(callback)
    return () => resets.delete(callback)
}
export function registerRead(controller: AbortController) {
    reads.add(controller)
    return () => reads.delete(controller)
}
export function resetSession() {
    sessionExpiredNotice.value = false
    clearWorkspace(owner.value)
    epoch.value++
    owner.value = null
    for (const controller of reads) controller.abort()
    reads.clear()
    for (const reset of resets) reset()
}
export function establishSession(id: string | number | null | undefined) {
    const next = id == null || id === '' ? null : String(id)
    if (owner.value !== null && owner.value !== next) resetSession()
    owner.value = next
    sessionExpiredNotice.value = false
}
export function setAuthExpiredHandler(handler: () => void) {
    expire = handler
}
export function expireSession() {
    const wasAuthenticated = owner.value !== null
    resetSession()
    sessionExpiredNotice.value = wasAuthenticated
    expire?.()
}
