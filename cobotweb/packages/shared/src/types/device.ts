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
  state: string
  last_heartbeat: string
  registered_at: string
  control_preset?: ControlPreset | null
}

// ── Control Preset ──

export type ControlTemplate = 'dpad' | 'dual_stick' | 'button_matrix'
export type InteractionMode = 'press_release' | 'continuous' | 'fire_once'

export interface ControlBinding {
  capability: string
  mode: InteractionMode
  interval_ms?: number
  params?: Record<string, unknown>
  release_params?: Record<string, unknown>
}

export interface ControlPreset {
  template: ControlTemplate
  mapping: Record<string, ControlBinding>
}
