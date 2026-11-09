export interface AuthorizationFlow {
    flow_id: string
    application_name: string
    scopes: string
    interaction: 'login' | 'consent' | 'ready'
    expires_at: number
    username: string | null
    csrf_token: string
}
