// 应用列表项
export interface ClientVO {
  id: number
  client_id: string
  name: string
  redirect_uris: string
  allowed_scopes: string
  grant_types: string
  public_client: boolean
  create_time: string
  update_time: string
}

// 注册应用请求
export interface ClientCreateReq {
  name: string
  homepage_url?: string
  description?: string
  redirect_uri: string
  redirect_uris?: string[]
  allowed_scopes?: string[]
  grant_types?: string[]
  public_client?: boolean
}

// 注册应用响应（含一次性 secret）
export interface ClientCreateResp extends ClientVO {
  client_secret: string | null
}

// 重新生成 secret 响应
export interface SecretResp {
  client_secret: string
}

// 更新应用请求
export interface ClientUpdateReq {
  name?: string
  redirect_uris?: string[]
  allowed_scopes?: string[]
  grant_types?: string[]
  public_client?: boolean
}

// Scope
export interface OAuthScopeVO {
  id: number
  name: string
  description: string | null
  create_time: string
  update_time: string
}

export interface ScopeCreateReq {
  name: string
  description?: string
}
