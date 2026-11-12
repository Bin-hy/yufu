import { spawn } from 'node:child_process'
import { release } from 'node:os'
import process from 'node:process'

// COBOT_SSO_PW_ARGS lets a debugging run narrow the suite, for example
// "--project=chromium -g logout". Normal runs leave it empty.
const extra = (process.env.COBOT_SSO_PW_ARGS ?? '').split(/\s+/).filter(Boolean)
const externalWebkit =
  process.platform === 'darwin' && Number(release().split('.')[0]) < 24
const projects = extra.filter((argument) => argument.startsWith('--project='))
const wantsWebkit =
  externalWebkit && (projects.length === 0 || projects.includes('--project=webkit'))
const playwrightArgs = externalWebkit
  ? extra.filter((argument) => argument !== '--project=webkit')
  : extra
const runsPlaywright =
  !externalWebkit || projects.length === 0 || playwrightArgs.some((argument) => argument.startsWith('--project='))

function run(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, args, { stdio: 'inherit', env: process.env })
    child.once('error', reject)
    child.once('exit', (code, signal) => {
      if (signal) process.kill(process.pid, signal)
      else resolve(code ?? 1)
    })
  })
}

if (runsPlaywright) {
  const code = await run([
    'node_modules/@playwright/test/cli.js',
    'test',
    '-c',
    'playwright.enterprise-sso.config.ts',
    ...playwrightArgs,
  ])
  if (code !== 0) process.exit(code)
}
if (wantsWebkit) {
  process.exit(await run(['tests/e2e/enterprise-sso-webkit.mjs']))
}
process.exit(0)
