// The Parquet module of browser-only mode (crates/web-parquet): loaded only when a Parquet file is opened, by the Worker, or by
// the page where there is no Worker.
import { loadCore } from './wasmcore';

/** A Parquet file's channels in the core's dump form. */
export async function parquetDump(parquetWasm: string, bytes: Uint8Array, path: string): Promise<ArrayBuffer> {
  const core = await loadCore(parquetWasm);
  return core('parquet_dump', { path }, bytes) as ArrayBuffer;
}

/** The first bytes say Parquet (`PAR1`), whatever the file is called. */
export const isParquet = (b: Uint8Array) => b.length > 4 && b[0] === 0x50 && b[1] === 0x41 && b[2] === 0x52 && b[3] === 0x31;
