import { createHttpClient } from './axios'

// The center owns HttpOnly cookies. Each tab restores current identity from
// the primary session endpoint; it does not rotate a shared refresh token.
export const identityRequest = createHttpClient({ authMode: 'center' })
