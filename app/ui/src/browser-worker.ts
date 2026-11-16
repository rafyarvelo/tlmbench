// Browser-only mode's parser: a Worker that reads a file with the WASM core and sends back what the core keeps of it (its own
// `dump`, which the page's core `load`s in a moment), so a big file never freezes the tab while it parses. A Parquet file goes
// to the Parquet module, which the page fetches only now (it is larger than the core and most tabs never open Parquet).
import { isParquet, parquetDump } from './parquet-wasm';
import { loadCore } from './wasmcore';

interface Job {
  wasm: string;
  parquetWasm: string;
  file: File;
  path: string;
  /** A binary file read through a descriptor the page keeps, and the packet type to read (or none). */
  described?: { descriptor: unknown; part: string | null };
}

self.onmessage = async (e: MessageEvent<Job>) => {
  try {
    const { wasm, parquetWasm, file, path, described } = e.data;
    const bytes = new Uint8Array(await file.arrayBuffer());
    let dump: ArrayBuffer;
    if (described) {
      const core = await loadCore(wasm);
      core('import_described', { file: 0, path, descriptor: described.descriptor, part: described.part }, bytes);
      dump = core('dump', { file: 0 }) as ArrayBuffer;
    } else if (isParquet(bytes)) dump = await parquetDump(parquetWasm, bytes, path);
    else {
      const core = await loadCore(wasm);
      core('import_file', { file: 0, path }, bytes);
      dump = core('dump', { file: 0 }) as ArrayBuffer;
    }
    (self as unknown as Worker).postMessage({ dump }, [dump]);
  } catch (err) {
    (self as unknown as Worker).postMessage({ error: String((err as Error)?.message ?? err) });
  }
};
