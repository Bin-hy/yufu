import type { PageResult } from '@/types/pagination'
import type { ApplicationVO } from '@/types/models'

export type ApplicationPage = PageResult<ApplicationVO>

export interface ApplicationInsertReq {
  app_key: string
  name: string
  description?: string | null
  oidc_client_id?: number | null
}

export interface ApplicationPatchReq {
  id: number
  name?: string | null
  description?: string | null
  oidc_client_id?: number | null
}

export interface ApplicationDeleteReq {
  id: number
}
