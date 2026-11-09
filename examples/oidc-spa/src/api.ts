import type { SpaConfig } from './config.js'

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly reason: string,
  ) {
    super(`api request failed: ${reason}`)
    this.name = 'ApiError'
  }
}

export interface OperationResult {
  ok: boolean
  operation?: string
}

export interface ApiClient {
  runOperation(accessToken: string, payload?: Record<string, unknown>): Promise<OperationResult>
}

/** The access token is sent only to the companion API, never to the issuer. */
export function createApiClient(
  config: SpaConfig,
  options: { fetch?: typeof fetch } = {},
): ApiClient {
  const http = options.fetch ?? fetch

  return {
    async runOperation(accessToken, payload = {}) {
      const response = await http(`${config.apiBaseUrl}/api/operation`, {
        method: 'POST',
        headers: {
          authorization: `Bearer ${accessToken}`,
          'content-type': 'application/json',
        },
        body: JSON.stringify(payload),
      })
      const body = (await response.json().catch(() => ({}))) as Record<string, unknown>
      if (!response.ok) {
        throw new ApiError(
          response.status,
          typeof body.error === 'string' ? body.error : 'request_failed',
        )
      }
      return {
        ok: body.ok === true,
        operation: typeof body.operation === 'string' ? body.operation : undefined,
      }
    },
  }
}
