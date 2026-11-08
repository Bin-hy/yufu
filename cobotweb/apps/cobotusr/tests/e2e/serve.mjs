import { spawn } from 'node:child_process'
import process from 'node:process'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const outDir = await mkdtemp(join(tmpdir(), 'cobotusr-e2e-'))
const env = { ...process.env, VITE_BASE_URL: '/api/v1', VITE_SYSTEM_NAME: 'cobotusr-e2e' }
const build = spawn('pnpm', ['exec', 'vite', 'build', '--outDir', outDir], {
    env,
    stdio: 'inherit',
})
const code = await new Promise((resolve) => build.on('exit', resolve))
if (code !== 0) {
    await rm(outDir, { recursive: true, force: true })
    process.exit(code ?? 1)
}
const preview = spawn(
    'pnpm',
    [
        'exec',
        'vite',
        'preview',
        '--outDir',
        outDir,
        '--host',
        '127.0.0.1',
        '--port',
        '5188',
        '--strictPort',
    ],
    { env, stdio: 'inherit' },
)
async function stop() {
    preview.kill('SIGTERM')
    await rm(outDir, { recursive: true, force: true })
    process.exit()
}
process.on('SIGINT', stop)
process.on('SIGTERM', stop)
preview.on('exit', async (code) => {
    await rm(outDir, { recursive: true, force: true })
    process.exit(code ?? 0)
})
