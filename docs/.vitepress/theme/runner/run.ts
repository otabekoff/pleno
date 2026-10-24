// Runs Pleno code in a Web Worker and returns its output. The worker is
// created on first use; a run that takes too long, or crashes, gets a fresh
// worker so the next run starts clean.

export interface RunResult {
  ok: boolean
  output: string
  error: string
  /** Milliseconds, including loading the WebAssembly module the first time. */
  elapsed: number
}

const TIMEOUT_MS = 10_000

let worker: Worker | null = null
let nextId = 0

function getWorker(): Worker {
  worker ??= new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' })
  return worker
}

function resetWorker() {
  worker?.terminate()
  worker = null
}

export function runPleno(source: string): Promise<RunResult> {
  const id = ++nextId
  const started = performance.now()
  const current = getWorker()

  return new Promise((resolve) => {
    const finish = (result: Omit<RunResult, 'elapsed'>, reset = false) => {
      clearTimeout(timer)
      current.removeEventListener('message', onMessage)
      if (reset) resetWorker()
      resolve({ ...result, elapsed: Math.round(performance.now() - started) })
    }

    const timer = setTimeout(() => {
      finish(
        { ok: false, output: '', error: `Stopped after ${TIMEOUT_MS / 1000} seconds.` },
        true,
      )
    }, TIMEOUT_MS)

    function onMessage(event: MessageEvent) {
      if (event.data?.id !== id) return
      const { ok, output, error, crashed } = event.data
      finish({ ok, output, error }, Boolean(crashed))
    }

    current.addEventListener('message', onMessage)
    current.postMessage({ id, source })
  })
}

/** Code shared through a playground link: `#code=<base64url of UTF-8>`. */
export function encodeCode(code: string): string {
  const bytes = new TextEncoder().encode(code)
  let binary = ''
  bytes.forEach((byte) => (binary += String.fromCharCode(byte)))
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

export function decodeCode(encoded: string): string | null {
  try {
    const base64 = encoded.replace(/-/g, '+').replace(/_/g, '/')
    const binary = atob(base64)
    return new TextDecoder().decode(Uint8Array.from(binary, (c) => c.charCodeAt(0)))
  } catch {
    return null
  }
}
