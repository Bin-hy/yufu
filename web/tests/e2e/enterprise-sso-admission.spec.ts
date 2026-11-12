import { expect, test } from '@playwright/test'

import { config, signToken } from './enterprise-sso-fixture'

test('the protected API rejects a foreign audience and an ID token replay', async ({
  request,
}) => {
  const cfg = config()
  const now = Math.floor(Date.now() / 1000)
  const claims = {
    iss: cfg.opOrigin,
    sub: cfg.subject,
    iat: now,
    exp: now + 300,
    session_id: 'fixture-credential-session',
    application: cfg.spaClientId,
  }

  const foreign = signToken(
    { ...claims, aud: 'urn:app:another-application' },
    'at+jwt',
  )
  const foreignResponse = await request.post(cfg.bOrigin + '/api/operation', {
    headers: { authorization: 'Bearer ' + foreign },
  })
  expect(foreignResponse.status()).toBe(401)
  await expect(foreignResponse.json()).resolves.toEqual({ error: 'invalid_token' })

  const idToken = signToken({ ...claims, aud: cfg.spaClientId }, 'JWT')
  const replayResponse = await request.post(cfg.bOrigin + '/api/operation', {
    headers: { authorization: 'Bearer ' + idToken },
  })
  expect(replayResponse.status()).toBe(401)
  await expect(replayResponse.json()).resolves.toEqual({ error: 'invalid_token' })
})

test('the protected API rejects an unregistered browser origin', async ({ request }) => {
  const cfg = config()
  const response = await request.post(cfg.bOrigin + '/api/operation', {
    headers: { origin: 'https://evil.example' },
  })

  expect(response.status()).toBe(403)
  await expect(response.json()).resolves.toEqual({ error: 'origin_rejected' })
})
