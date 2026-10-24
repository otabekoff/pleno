// Runs Pleno programs off the main thread, so a long run never freezes the
// page. The WebAssembly module is built by scripts/build-playground.sh.
import init, { run_playground } from '../wasm/pleno.js'

const ready = init()

self.onmessage = async (event: MessageEvent<{ id: number; source: string }>) => {
  const { id, source } = event.data
  try {
    await ready
    const result = JSON.parse(run_playground(source))
    self.postMessage({ id, ...result })
  } catch (error) {
    // A crash poisons the module; the page replaces this worker.
    self.postMessage({
      id,
      ok: false,
      output: '',
      error: `Pleno stopped unexpectedly: ${error}`,
      crashed: true,
    })
  }
}
