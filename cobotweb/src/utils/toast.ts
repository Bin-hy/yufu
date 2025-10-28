export type ToastType = 'success' | 'error' | 'info' | 'warning'

export interface ToastPayload {
  type?: ToastType
  message: string
  durationMs?: number
}

export function emitToast(payload: ToastPayload) {
  const detail = {
    type: payload.type ?? 'info',
    message: payload.message,
    durationMs: payload.durationMs ?? 3000
  }
  window.dispatchEvent(new CustomEvent('app:toast', { detail }))
}