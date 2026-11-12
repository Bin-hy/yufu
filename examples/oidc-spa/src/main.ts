import { createApiClient } from './api.js'
import { browserConfig } from './config.js'
import { createSpaOidc, type SpaSession } from './oidc.js'

const config = browserConfig()
const oidc = createSpaOidc(config)
const api = createApiClient(config)

const root = document.querySelector<HTMLElement>('#app')
if (!root) throw new Error('#app is missing')
const app: HTMLElement = root

function render(session: SpaSession | undefined, message = ''): void {
  app.innerHTML = session
    ? `<h1>Signed in</h1>
       <p data-testid="display-name">${escapeHtml(session.identity.displayName)}</p>
       <p data-testid="local-subject">${escapeHtml(session.identity.subject)}</p>
     <p data-testid="issuer">${escapeHtml(session.identity.issuer)}</p>
     <p data-testid="client-id">${escapeHtml(session.clientId)}</p>
     <p data-testid="audience">${escapeHtml(session.identity.audience.join(', '))}</p>
     <p data-testid="sid">${escapeHtml(session.identity.sid ?? '—')}</p>
     <p data-testid="auth-time">${escapeHtml(String(session.identity.authTime ?? '—'))}</p>
     <p data-testid="state-validated">${session.stateVerified}</p>
     <p data-testid="nonce-validated">${session.nonceVerified}</p>
     <p data-testid="access-token-fingerprint">${escapeHtml(session.accessTokenFingerprint)}</p>
       <button id="operation" type="button">Run protected operation</button>
       <button id="logout-local" type="button">Sign out of this tab</button>
       <button id="logout-central" type="button">Sign out of the identity center</button>`
    : `<h1>Not signed in</h1>
       <button id="login" type="button">Sign in with Cobotrix</button>`
  if (message) app.insertAdjacentHTML('beforeend', `<p role="status">${escapeHtml(message)}</p>`)

  document.querySelector('#login')?.addEventListener('click', () => void oidc.signIn())
  document.querySelector('#operation')?.addEventListener('click', () => void runOperation())
  document.querySelector('#logout-local')?.addEventListener('click', async () => {
    await oidc.signOutLocal()
    render(undefined, 'Signed out of this tab.')
  })
  document.querySelector('#logout-central')?.addEventListener('click', () => {
    void oidc.signOutCentral()
  })
}

function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
}

async function runOperation(): Promise<void> {
  try {
    const token = await oidc.accessToken()
    const result = await api.runOperation(token, { action: 'example-write' })
    render(await oidc.currentSession(), result.ok ? 'Operation accepted.' : 'Operation rejected.')
  } catch (error) {
    render(
      await oidc.currentSession(),
      error instanceof Error ? error.message : 'Operation failed.',
    )
  }
}

async function start(): Promise<void> {
  const url = new URL(window.location.href)
  if (url.pathname === new URL(config.redirectUri).pathname) {
    const session = await oidc.completeSignIn(window.location.href)
    window.history.replaceState({}, '', '/')
    render(session)
    return
  }
  render(await oidc.currentSession())
}

void start()
