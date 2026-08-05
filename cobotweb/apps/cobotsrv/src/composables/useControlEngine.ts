import { ref, type Ref } from 'vue'
import { submitTaskSync } from '../api'
import type { ControlPreset, ControlBinding } from '@cobotweb/shared'

export interface FeedbackEntry {
  key: string
  success: boolean
  error?: string
  durationMs: number
  timestamp: Date
}

export function useControlEngine(
  cobotletAlias: Ref<string>,
  deviceId: Ref<string>,
  preset: Ref<ControlPreset | null | undefined>,
) {
  const feedbackLog = ref<FeedbackEntry[]>([])
  const activeKeys = ref<Set<string>>(new Set())
  const timers = new Map<string, ReturnType<typeof setInterval>>()

  function getBinding(key: string): ControlBinding | undefined {
    return preset.value?.mapping[key]
  }

  async function sendCommand(key: string, params: Record<string, unknown> | undefined) {
    if (!params) return
    const binding = getBinding(key)
    if (!binding) return
    const start = performance.now()
    try {
      await submitTaskSync({
        cobotlet_alias: cobotletAlias.value,
        capability_name: binding.capability,
        device_id: deviceId.value || undefined,
        body: params,
        timeout_ms: 3000,
      })
      addFeedback(key, true, undefined, performance.now() - start)
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e)
      addFeedback(key, false, msg, performance.now() - start)
    }
  }

  function addFeedback(key: string, success: boolean, error: string | undefined, durationMs: number) {
    feedbackLog.value.unshift({ key, success, error, durationMs: Math.round(durationMs), timestamp: new Date() })
    if (feedbackLog.value.length > 50) feedbackLog.value.pop()
  }

  function press(key: string) {
    const binding = getBinding(key)
    if (!binding) return
    activeKeys.value.add(key)
    activeKeys.value = new Set(activeKeys.value)

    if (binding.mode === 'fire_once') {
      sendCommand(key, binding.params as Record<string, unknown> | undefined)
      activeKeys.value.delete(key)
      activeKeys.value = new Set(activeKeys.value)
      return
    }

    if (binding.mode === 'press_release') {
      sendCommand(key, binding.params as Record<string, unknown> | undefined)
      return
    }

    if (binding.mode === 'continuous') {
      sendCommand(key, binding.params as Record<string, unknown> | undefined)
      const interval = binding.interval_ms ?? 200
      const timer = setInterval(() => {
        sendCommand(key, binding.params as Record<string, unknown> | undefined)
      }, interval)
      timers.set(key, timer)
    }
  }

  function release(key: string) {
    const binding = getBinding(key)
    if (!binding) return
    activeKeys.value.delete(key)
    activeKeys.value = new Set(activeKeys.value)

    if (binding.mode === 'press_release' && binding.release_params) {
      sendCommand(key, binding.release_params as Record<string, unknown>)
    }

    if (binding.mode === 'continuous') {
      const timer = timers.get(key)
      if (timer) {
        clearInterval(timer)
        timers.delete(key)
      }
    }
  }

  function fire(key: string) {
    press(key)
  }

  function destroy() {
    for (const timer of timers.values()) {
      clearInterval(timer)
    }
    timers.clear()
    activeKeys.value.clear()
  }

  return { press, release, fire, destroy, feedbackLog, activeKeys }
}
