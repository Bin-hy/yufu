import { request } from '../http'
import type { DeviceVO, TaskResult } from '@cobotweb/shared'

// ── Devices ──

export function listDevices() {
  return request<DeviceVO[]>({ url: '/devices', method: 'GET' })
}

export function listDevicesByAlias(alias: string) {
  return request<DeviceVO[]>({ url: `/devices/${alias}`, method: 'GET' })
}

export function getDevice(alias: string, deviceId: string) {
  return request<DeviceVO>({ url: `/devices/${alias}/${deviceId}`, method: 'GET' })
}

// ── Tasks ──

export interface SubmitTaskPayload {
  cobotlet_alias: string
  capability_name: string
  task_id?: string
  device_id?: string
  headers?: Record<string, string>
  body?: unknown
  timeout_ms?: number
}

/** 同步提交（wait=true），直接返回结果 */
export function submitTaskSync(payload: SubmitTaskPayload) {
  return request<TaskResult>({
    url: '/tasks',
    method: 'POST',
    params: { wait: true },
    data: payload,
  })
}

/** 异步提交（wait=false），返回 task_id */
export function submitTaskAsync(payload: SubmitTaskPayload) {
  return request<{ task_id: string }>({
    url: '/tasks',
    method: 'POST',
    params: { wait: false },
    data: payload,
  })
}

export function getTaskResult(taskId: string) {
  return request<TaskResult>({ url: `/tasks/${taskId}`, method: 'GET' })
}
