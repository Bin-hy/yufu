import { request } from '../http'
import type { DeviceVO, JsonSchema, PageResult, TaskResult } from '@cobotweb/shared'

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

// ── MCP management ──

export interface ToolProvider {
  cobotlet_alias: string
  device_id: string
  state: string
}

export interface ToolDescriptor {
  name: string
  description?: string | null
  method: string
  path: string
  tags: string[]
  input_schema?: JsonSchema | null
  providers: ToolProvider[]
}

export interface McpInfo {
  protocol_version: string
  audience: string
  call_timeout_ms: number
  online_tools: number
  online_devices: number
  calls_today: number
  success_today: number
}

export interface McpSession {
  session_id: string
  subject: string
  connected_at: string
  last_seen: string
  transport: 'http' | 'sse'
}

export interface McpCallLog {
  id: number
  created_at: string
  subject: string
  method: string
  tool_name?: string | null
  /** Error message if failed, null/undefined if success */
  error_msg?: string | null
  duration_ms: number
  request_id?: string | null
  input_params?: object | null
  status_code?: number | null
  device_id?: string | null
  cobotlet_alias?: string | null
  source?: string | null
}

export interface McpLogQuery {
  page_number?: number
  page_size?: number
  subject?: string
  tool?: string
  result?: 'ok' | 'error'
  device_id?: string
  source?: string
}

/** 工具目录（= 设备能力聚合），走 mcp:admin 保护的聚合视图（与 GET /tools 同形，
 * 但不要求 device:read，纯 MCP 管理员也可用） */
export function listTools() {
  return request<{ tools: ToolDescriptor[] }>({ url: '/mcp/admin/tools', method: 'GET' }).then(
    (r) => r.tools,
  )
}

export function getMcpInfo() {
  return request<McpInfo>({ url: '/mcp/admin/info', method: 'GET' })
}

export function listMcpSessions() {
  return request<McpSession[]>({ url: '/mcp/admin/sessions', method: 'GET' })
}

export function listMcpLogs(params: McpLogQuery = {}) {
  return request<PageResult<McpCallLog>>({ url: '/mcp/admin/logs', method: 'GET', params })
}
