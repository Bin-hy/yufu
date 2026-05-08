export interface Capability {
  name: string
  method: string
  path: string
  tags: string[]
  schema?: unknown | null
}

export interface DeviceVO {
  id: number
  cobotlet_alias: string
  device_id: string
  service_url: string
  capabilities: Capability[]
  online: boolean
  last_heartbeat: string
  registered_at: string
}
