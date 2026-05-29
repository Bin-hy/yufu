export interface ScopeInfo {
  name: string
  description: string | null
}

export interface ClientInfoResponse {
  client_name: string
  client_id: string
  scopes: ScopeInfo[]
  already_consented: boolean
}

export interface ConsentRequest {
  client_id: string
  redirect_uri: string
  response_type: string
  code_challenge?: string
  code_challenge_method?: string
  scope?: string
  state?: string
  nonce?: string
  approved: boolean
}

export interface ConsentResponse {
  redirect_url: string
}
