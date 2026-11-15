// Browser-only mode's parser: a Worker that reads a file with the WASM core and sends back what the core keeps of it (its own
// `dump`, which the page's core `load`s in a moment), so a big CSV never freezes the tab while it parses.
import { loadCore } from './wasmcore';

interface Job {
  wasm: string;
  file: File;
  path: string;
}

self.onmessage = async (e: MessageEvent<Job>) => {
  try {
    const { wasm, file, path } = e.data;
    const core = await loadCore(wasm);
    const bytes = new Uint8Array(await file.arrayBuffer());
    core('import_file', { file: 0, path }, bytes);
    const dump = core('dump', { file: 0 }) as ArrayBuffer;
    (self as unknown as Worker).postMessage({ dump }, [dump]);
  } catch (err) {
    (self as unknown as Worker).postMessage({ error: String((err as Error)?.message ?? err) });
  }
};
