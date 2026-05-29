export interface JsonSchemaProperty {
  type: 'string' | 'number' | 'integer' | 'boolean' | 'array' | 'object'
  description?: string
  minimum?: number
  maximum?: number
  minLength?: number
  maxLength?: number
  enum?: unknown[]
  default?: unknown
  items?: JsonSchemaProperty
  properties?: Record<string, JsonSchemaProperty>
  required?: string[]
}

export interface JsonSchema {
  type: 'object'
  description?: string
  properties?: Record<string, JsonSchemaProperty>
  required?: string[]
}

export interface Capability {
  name: string
  method: string
  path: string
  description?: string | null
  tags: string[]
  input_schema?: JsonSchema | null
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
