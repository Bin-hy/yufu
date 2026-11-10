import { spawn, type ChildProcess } from 'node:child_process'
import { createSign } from 'node:crypto'
import { createReadStream, existsSync, readFileSync, statSync } from 'node:fs'
import {
  request as httpRequest,
  type IncomingMessage,
  type ServerResponse,
} from 'node:http'
import { createServer, type Server } from 'node:https'
import { extname, join, normalize, resolve, sep } from 'node:path'

import type { Page } from '@playwright/test'

/** Everything the Rust fixture decides and the Node side only consumes. */
export interface SsoConfig {
  repoRoot: string
  opOrigin: string
  aOrigin: string
  bOrigin: string
  opUpstream: string
  aUpstream: string
  apiUpstream: string
  cert: string
  key: string
  ca: string
  spaDist: string
  webPort: number
  apiPort: number
  username: string
  password: string
  subject: string
  webClientId: string
  webClientSecret: string
  spaClientId: string
  apiAudience: string
  readerClientId: string
  readerClientSecret: string
  sessionSecret: string
  signingKey: string
}

function required(name: string): string {
  const value = process.env[name]?.trim()
  if (!value) throw new Error(name + ' is required')
  return value
}

function requiredPort(name: string): number {
  const value = Number(required(name))
  if (!Number.isInteger(value) || value < 1 || value > 65535) {
    throw new Error(name + ' must be an integer port')
  }
  return value
}

export function config(): SsoConfig {
  return {
    repoRoot: required('COBOT_SSO_REPO_ROOT'),
    opOrigin: required('COBOT_SSO_OP_ORIGIN'),
    aOrigin: required('COBOT_SSO_A_ORIGIN'),
    bOrigin: required('COBOT_SSO_B_ORIGIN'),
    opUpstream: required('COBOT_SSO_OP_UPSTREAM'),
    aUpstream: required('COBOT_SSO_A_UPSTREAM'),
    apiUpstream: required('COBOT_SSO_API_UPSTREAM'),
    cert: required('COBOT_SSO_CERT'),
    key: required('COBOT_SSO_KEY'),
    ca: required('COBOT_SSO_CA'),
    spaDist: required('COBOT_SSO_SPA_DIST'),
    webPort: requiredPort('COBOT_SSO_WEB_PORT'),
    apiPort: requiredPort('COBOT_SSO_API_PORT'),
    username: required('COBOT_SSO_USERNAME'),
    password: required('COBOT_SSO_PASSWORD'),
    subject: required('COBOT_SSO_SUBJECT'),
    webClientId: required('COBOT_SSO_WEB_CLIENT_ID'),
    webClientSecret: required('COBOT_SSO_WEB_CLIENT_SECRET'),
    spaClientId: required('COBOT_SSO_SPA_CLIENT_ID'),
    apiAudience: required('COBOT_SSO_API_AUDIENCE'),
    readerClientId: required('COBOT_SSO_READER_CLIENT_ID'),
    readerClientSecret: required('COBOT_SSO_READER_CLIENT_SECRET'),
    sessionSecret: required('COBOT_SSO_SESSION_SECRET'),
    signingKey: required('COBOT_SSO_SIGNING_KEY'),
  }
}

function proxy(upstream: URL, label = 'proxy') {
  return (request: IncomingMessage, response: ServerResponse): void => {
    if (process.env.COBOT_SSO_TRACE) {
      process.stderr.write(`[${label}] ${request.method} ${request.url}\n`)
    }
    const target = new URL(request.url ?? '/', upstream)
    const headers = { ...request.headers, host: upstream.host }
    const forwarded = httpRequest(
      {
        hostname: target.hostname,
        port: target.port,
        path: target.pathname + target.search,
        method: request.method,
        headers,
      },
      (upstreamResponse) => {
        if (process.env.COBOT_SSO_TRACE) {
          process.stderr.write(
            `[${label}] ${request.method} ${request.url} -> ${upstreamResponse.statusCode}\n`,
          )
        }
        response.writeHead(upstreamResponse.statusCode ?? 502, upstreamResponse.headers)
        upstreamResponse.pipe(response)
      },
    )
    forwarded.on('error', () => {
      if (!response.headersSent) response.writeHead(502)
      response.end()
    })
    request.pipe(forwarded)
  }
}

const CONTENT_TYPES: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
}

function spaServer(dist: string, api: URL) {
  const apiProxy = proxy(api)
  const root = resolve(dist)
  return (request: IncomingMessage, response: ServerResponse): void => {
    const path = request.url ?? '/'
    if (path.startsWith('/api/') || path.startsWith('/oidc/')) {
      apiProxy(request, response)
      return
    }
    const pathname = decodeURIComponent(new URL(path, 'https://spa.invalid').pathname)
    const candidate = normalize(join(root, pathname))
    if (candidate !== root && !candidate.startsWith(root + sep)) {
      response.writeHead(403)
      response.end()
      return
    }
    const file =
      existsSync(candidate) && statSync(candidate).isFile()
        ? candidate
        : join(root, 'index.html')
    response.writeHead(200, {
      'content-type': CONTENT_TYPES[extname(file)] ?? 'application/octet-stream',
      'cache-control': 'no-store',
    })
    createReadStream(file).pipe(response)
  }
}

/**
 * Three distinct loopback hosts give the browser three distinct cookie jars.
 * macOS only aliases 127.0.0.1, so the other two sites use `localhost` and
 * `[::1]` instead of 127.0.0.2/127.0.0.3.
 */
function bindHost(origin: string): string | undefined {
  const host = new URL(origin).hostname
  if (host === 'localhost') return undefined
  if (host.startsWith('[') && host.endsWith(']')) return host.slice(1, -1)
  return host
}

function listen(server: Server, port: number, host: string | undefined): Promise<void> {
  return new Promise((resolveListen, reject) => {
    server.once('error', reject)
    const ready = () => resolveListen()
    if (host === undefined) server.listen(port, ready)
    else server.listen(port, host, ready)
  })
}

function close(server: Server): Promise<void> {
  return new Promise((resolveClose) => server.close(() => resolveClose()))
}

interface ManagedProcess {
  stop(): Promise<void>
}

function startProcess(
  name: string,
  cwd: string,
  command: string,
  args: string[],
  env: NodeJS.ProcessEnv,
): ManagedProcess {
  const child: ChildProcess = spawn(command, args, {
    cwd,
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  child.stdout?.on('data', (chunk: Buffer) => process.stdout.write(`[${name}] ${chunk}`))
  child.stderr?.on('data', (chunk: Buffer) => process.stderr.write(`[${name}] ${chunk}`))
  const exited = new Promise<void>((resolveExit) => child.once('exit', () => resolveExit()))
  let stopped = false
  return {
    async stop() {
      if (stopped) return
      stopped = true
      child.kill('SIGTERM')
      await new Promise<void>((resolveForce) => {
        const timer = setTimeout(() => {
          child.kill('SIGKILL')
          resolveForce()
        }, 5_000)
        void exited.then(() => {
          clearTimeout(timer)
          resolveForce()
        })
      })
    },
  }
}

function webEnvironment(cfg: SsoConfig): NodeJS.ProcessEnv {
  return {
    ...process.env,
    PORT: String(cfg.webPort),
    ISSUER: cfg.opOrigin,
    CLIENT_ID: cfg.webClientId,
    CLIENT_SECRET: cfg.webClientSecret,
    REDIRECT_URI: cfg.aOrigin + '/callback',
    POST_LOGOUT_REDIRECT_URI: cfg.aOrigin + '/',
    SESSION_SECRET: cfg.sessionSecret,
    READER_CLIENT_ID: cfg.readerClientId,
    READER_CLIENT_SECRET: cfg.readerClientSecret,
    ALLOW_INSECURE_LOOPBACK: 'true',
    NODE_EXTRA_CA_CERTS: cfg.ca,
  }
}

function apiEnvironment(cfg: SsoConfig): NodeJS.ProcessEnv {
  return {
    ...process.env,
    PORT: String(cfg.apiPort),
    ISSUER: cfg.opOrigin,
    API_AUDIENCE: cfg.apiAudience,
    SPA_CLIENT_ID: cfg.spaClientId,
    READER_CLIENT_ID: cfg.readerClientId,
    READER_CLIENT_SECRET: cfg.readerClientSecret,
    ALLOWED_ORIGIN: cfg.bOrigin,
    ALLOW_INSECURE_LOOPBACK: 'true',
    NODE_EXTRA_CA_CERTS: cfg.ca,
  }
}

function spaBuildEnvironment(cfg: SsoConfig): NodeJS.ProcessEnv {
  return {
    ...process.env,
    VITE_ISSUER: cfg.opOrigin,
    VITE_CLIENT_ID: cfg.spaClientId,
    VITE_REDIRECT_URI: cfg.bOrigin + '/callback',
    VITE_POST_LOGOUT_REDIRECT_URI: cfg.bOrigin + '/',
    VITE_API_BASE_URL: cfg.bOrigin,
    VITE_SCOPE: 'openid profile email',
  }
}

async function buildSpa(cfg: SsoConfig): Promise<void> {
  const cwd = join(cfg.repoRoot, 'examples/oidc-spa')
  const child = spawn(
    join(cwd, 'node_modules/.bin/vite'),
    ['build', '--outDir', cfg.spaDist, '--emptyOutDir', '--logLevel', 'warn'],
    { cwd, env: spaBuildEnvironment(cfg), stdio: ['ignore', 'inherit', 'inherit'] },
  )
  const code = await new Promise<number | null>((resolveExit) =>
    child.once('exit', resolveExit),
  )
  if (code !== 0) throw new Error(`SPA build failed with exit code ${code}`)
}

async function waitForHttp(url: string, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs
  let last = ''
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url)
      if (response.ok) return
      last = `status ${response.status}`
    } catch (error) {
      last = error instanceof Error ? error.message : String(error)
    }
    await new Promise((resolveWait) => setTimeout(resolveWait, 200))
  }
  throw new Error(`${url} did not become ready: ${last}`)
}

/**
 * Starts the two relying-party processes, builds the SPA with the fixture
 * origins and terminates TLS on three distinct loopback addresses so the
 * browser sees three separate sites. The Rust test owns the real OP.
 */
export default async function globalSetup(): Promise<() => Promise<void>> {
  const cfg = config()
  const certificate = readFileSync(cfg.cert)
  const key = readFileSync(cfg.key)
  const servers: Server[] = []
  const processes: ManagedProcess[] = []

  const stopAll = async (): Promise<void> => {
    await Promise.all(processes.map((process) => process.stop()))
    await Promise.all(servers.map(close))
  }

  try {
    await buildSpa(cfg)

    const spaRoot = join(cfg.repoRoot, 'examples/oidc-spa')
    processes.push(
      startProcess(
        'oidc-web',
        join(cfg.repoRoot, 'examples/oidc-web'),
        join(cfg.repoRoot, 'examples/oidc-web/node_modules/.bin/tsx'),
        ['src/server.ts'],
        webEnvironment(cfg),
      ),
    )
    processes.push(
      startProcess(
        'oidc-spa-api',
        spaRoot,
        join(spaRoot, 'node_modules/.bin/tsx'),
        ['server/server.ts'],
        apiEnvironment(cfg),
      ),
    )

    await waitForHttp(cfg.aUpstream + '/', 30_000)
    await waitForHttp(cfg.apiUpstream + '/healthz', 30_000)

    const op = new URL(cfg.opOrigin)
    const a = new URL(cfg.aOrigin)
    const b = new URL(cfg.bOrigin)
    servers.push(
      createServer({ cert: certificate, key }, proxy(new URL(cfg.opUpstream), 'op')),
      createServer({ cert: certificate, key }, proxy(new URL(cfg.aUpstream), 'web')),
      createServer({ cert: certificate, key }, spaServer(cfg.spaDist, new URL(cfg.apiUpstream))),
    )
    await Promise.all([
      listen(servers[0]!, Number(op.port), bindHost(cfg.opOrigin)),
      listen(servers[1]!, Number(a.port), bindHost(cfg.aOrigin)),
      listen(servers[2]!, Number(b.port), bindHost(cfg.bOrigin)),
    ])
  } catch (error) {
    await stopAll()
    throw error
  }

  return stopAll
}

/**
 * The consent screen only appears while the application has no current grant,
 * so waiting for it unconditionally breaks every later scenario in the same
 * fixture. The provider may also redirect straight back to the client when the
 * grant already exists, so poll for either the consent UI or the signed-in
 * client view instead of waiting for a provider navigation that may never
 * commit.
 */
export async function awaitConsentIfShown(page: Page, cfg: SsoConfig): Promise<void> {
  const consent = page.getByRole('button', { name: '同意授权' })
  const signedIn = page.getByTestId('display-name')
  const deadline = Date.now() + 20_000
  let approved = false
  while (Date.now() < deadline) {
    if (!approved && page.url().startsWith(cfg.opOrigin) && (await consent.isVisible())) {
      // Firefox can keep waiting for the load event of the redirect chain that
      // the consent submission starts. Dispatch the click in the page and let
      // the loop below observe the signed-in view instead.
      try {
        await page.evaluate(() => {
          const button = Array.from(document.querySelectorAll('button')).find(
            (candidate) => candidate.textContent?.trim() === '同意授权',
          )
          button?.click()
        })
        approved = true
      } catch {
        // A navigation can replace the document between the checks above.
      }
    }
    if (!page.url().startsWith(cfg.opOrigin) && (await signedIn.isVisible())) {
      return
    }
    await page.waitForTimeout(100)
  }
  throw new Error(`SSO did not reach consent or a signed-in application; current URL: ${page.url()}`)
}

export async function loginAtCenter(page: Page, cfg: SsoConfig): Promise<void> {
  await page.getByLabel('用户名', { exact: true }).fill(cfg.username)
  await page.getByLabel('密码', { exact: true }).fill(cfg.password)
  await page.getByRole('button', { name: '登录', exact: true }).click()
  await awaitConsentIfShown(page, cfg)
}

function base64url(value: Buffer | string): string {
  return Buffer.from(value).toString('base64url')
}

/** Signs a token with the fixture OP key; used only for negative-path checks. */
export function signToken(payload: Record<string, unknown>, typ: string): string {
  const key = readFileSync(config().signingKey, 'utf8')
  const header = base64url(JSON.stringify({ alg: 'RS256', typ, kid: 'cobotrix-rsa-1' }))
  const body = base64url(JSON.stringify(payload))
  const signature = createSign('RSA-SHA256').update(header + '.' + body).sign(key)
  return header + '.' + body + '.' + base64url(signature)
}

