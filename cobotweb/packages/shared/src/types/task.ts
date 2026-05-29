export interface TaskSubmitRequest {
  cobotlet_alias: string
  capability_name: string
  task_id?: string
  device_id?: string
  headers?: Record<string, string>
  body?: unknown
  timeout_ms?: number
}

export interface TaskResult {
  id: string
  status: number
  headers: Record<string, string>
  body?: unknown | null
  error?: string | null
}

export type SubmitResponse = TaskResult | { task_id: string }
