import { request } from './http'
import type { Capability } from '@cobotweb/shared'

export interface LocalDevice {
  id: string
  service_url: string
  capabilities: Capability[]
  last_heartbeat: string
  registered_at: string
  state: string
}

export interface LocalTaskResult {
  id: string
  status: number
  headers: Record<string, string>
  body?: unknown | null
  error?: string | null
}

export interface CobotletInfo {
  alias: string
}

export function getHello() {
  return request<CobotletInfo>({ url: '/hello', method: 'GET' })
}

export function listDevices() {
  return request<LocalDevice[]>({ url: '/devices', method: 'GET' })
}

export function deleteDevice(id: string) {
  return request<void>({ url: `/devices/${encodeURIComponent(id)}`, method: 'DELETE' })
}

export function dispatchTask(payload: {
  id: string
  capability_name: string
  device_id?: string
  body?: unknown
  timeout_ms?: number
}) {
  return request<LocalTaskResult>({ url: '/tasks', method: 'POST', data: payload })
}
