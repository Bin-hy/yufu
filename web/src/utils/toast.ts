export type ToastType = 'success' | 'error' | 'info' | 'warning'

export interface ToastPayload {
  type?: ToastType
  message: string
  durationMs?: number
  dedupeKey?: string
}

import { useToastStore } from '@/pinia/stores/toast'
import { pinia } from '@/pinia'

export function emitToast(payload: ToastPayload) {
  const store = useToastStore(pinia)
  store.push(payload.type ?? 'info', payload.message, payload.durationMs, payload.dedupeKey)
}