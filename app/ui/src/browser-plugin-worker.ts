// Browser-only mode's plugin sandbox (ledger D-brow-09; owner, Nov 16): a WASM decoder plugin, the same `.wasm` component
// the app runs on wasmtime, translated here by jco (fetched only when a plugin is used) and run in this Worker of its own.
// What it gets is what the app's sandbox gives: the file through `source.len` and `source.read` and nothing else (no
// folders, no environment or arguments, clocks fixed at zero, stdout and stderr thrown away). Its memory is capped by
// rewriting the core modules' memory limits before they compile; the page ends this Worker when a call runs past the CPU
// limit. One Worker does one job: `about` (describe), `sniff`, or `decode` (start, then next until it ends).

import * as cli from '@bytecodealliance/preview2-shim/cli';
import * as clocks from '@bytecodealliance/preview2-shim/clocks';
import * as filesystem from '@bytecodealliance/preview2-shim/filesystem';
import * as io from '@bytecodealliance/preview2-shim/io';
import * as random from '@bytecodealliance/preview2-shim/random';

/** What a descriptor's payload plugin decodes (the core's `payload_requests`, D-ingest-09): a packet type's packets, each
 *  `[header length u32][payload length u32][header][payload]`, or plugin fields' bytes, field after field. */
export type PayloadWork =
  | { kind: 'packets'; type: string; params: string; packets: number; bin: ArrayBuffer }
  | { kind: 'fields'; records: number; fields: { name: string; params: string; dtype: string; size: number }[]; bin: ArrayBuffer };

export interface PluginJob {
  op: 'about' | 'sniff' | 'decode' | 'payload';
  /** The plugin's bytes, and a digest of them that names its translation in the page's storage. */
  wasm: ArrayBuffer;
  id: string;
  /** WebAssembly pages (64 KB) a memory may grow to: the app's 1 GiB. */
  maxPages: number;
  file?: File;
  head?: Uint8Array;
  name?: string;
  payload?: PayloadWork;
}

export type PluginReply =
  | { kind: 'progress'; rows: number }
  | { kind: 'about'; about: { name: string; version: string; description?: string; files: string[] } }
  | { kind: 'sniff'; yes: boolean }
  | { kind: 'decoded'; meta: Record<string, unknown>; bin: ArrayBuffer }
  | { kind: 'values'; bin: ArrayBuffer }
  | { kind: 'error'; error: string };

/** FileReaderSync: Workers have it; the project's DOM types don't. */
declare const FileReaderSync: { new (): { readAsArrayBuffer(b: Blob): ArrayBuffer } } | undefined;

const post = (r: PluginReply, transfer: Transferable[] = []) => (self as unknown as Worker).postMessage(r, transfer);

/** Unsigned LEB128 at `at`: the value and the bytes it took. */
function leb(b: Uint8Array, at: number): [number, number] {
  let v = 0;
  let shift = 0;
  let k = at;
  for (;;) {
    const x = b[k++];
    v += (x & 0x7f) * 2 ** shift;
    if (!(x & 0x80)) return [v, k - at];
    shift += 7;
  }
}
const lebBytes = (v: number): number[] => {
  const out: number[] = [];
  do {
    let x = v % 128;
    v = Math.floor(v / 128);
    if (v) x |= 0x80;
    out.push(x);
  } while (v);
  return out;
};

/** A core module with every memory it defines given a maximum of `pages` (or its own, if smaller): growing past it fails
 *  inside the plugin, as wasmtime's memory limit does in the app. */
export function capMemory(wasm: Uint8Array, pages: number): Uint8Array {
  const out: number[] = [...wasm.subarray(0, 8)];
  let at = 8;
  while (at < wasm.length) {
    const id = wasm[at];
    const [size, n] = leb(wasm, at + 1);
    const start = at + 1 + n;
    const body = wasm.subarray(start, start + size);
    if (id !== 5) out.push(id, ...lebBytes(size), ...body);
    else {
      const [count, c] = leb(body, 0);
      let p = c;
      const mem: number[] = [...lebBytes(count)];
      for (let i = 0; i < count; i++) {
        const flags = body[p++];
        if (flags & 0x04) throw new Error('the plugin uses 64-bit memory, which a browser tab can’t limit');
        const [min, a] = leb(body, p);
        p += a;
        let max = pages;
        if (flags & 0x01) {
          const [m, b] = leb(body, p);
          p += b;
          max = Math.min(m, pages);
        }
        if (min > pages) throw new Error(`the plugin starts with ${Math.round((min * 65536) / 2 ** 20)} MB of memory, over the limit`);
        mem.push((flags & 0x02) | 0x01, ...lebBytes(min), ...lebBytes(max));
      }
      out.push(5, ...lebBytes(mem.length), ...mem);
    }
    at = start + size;
  }
  return new Uint8Array(out);
}

/** The translation of a plugin: jco's files (the glue as text, the core modules), kept in the page's storage by digest. */
async function translated(job: PluginJob): Promise<Map<string, Uint8Array>> {
  let dir: FileSystemDirectoryHandle | null = null;
  try {
    dir = await (await (await navigator.storage.getDirectory()).getDirectoryHandle('tlmbench-plugins', { create: true })).getDirectoryHandle(`jco-${job.id}`, { create: true });
    const files = new Map<string, Uint8Array>();
    for await (const [n, h] of (dir as unknown as { entries(): AsyncIterable<[string, FileSystemHandle]> }).entries()) {
      if (h.kind === 'file') files.set(n, new Uint8Array(await (await (h as FileSystemFileHandle).getFile()).arrayBuffer()));
    }
    if (files.has('plugin.js')) return files;
  } catch {
    dir = null; // no storage: translated each time
  }
  const { generate, $init } = (await import('@bytecodealliance/jco-transpile/component')) as unknown as {
    generate(c: Uint8Array, o: Record<string, unknown>): { files: [string, Uint8Array][] };
    $init: Promise<void>;
  };
  await $init;
  const out = generate(new Uint8Array(job.wasm), { name: 'plugin', instantiation: { tag: 'async' }, noTypescript: true, noNodejsCompat: true });
  const files = new Map(out.files);
  if (dir) {
    for (const [n, b] of files) {
      try {
        const w = await (await dir.getFileHandle(n, { create: true })).createWritable();
        await w.write(b as Uint8Array<ArrayBuffer>);
        await w.close();
      } catch {
        /* kept next time, or not */
      }
    }
  }
  return files;
}

interface Decoder {
  describe(): { name: string; version: string; description?: string; files: string[] };
  sniff(head: Uint8Array, name: string): boolean;
  start(): { channels: { name: string; unit?: string; dtype: string }[]; timed: boolean };
  next(max: number): { rows: number; time: BigInt64Array | bigint[]; columns: Uint8Array[] } | undefined;
}

/** A payload plugin's exports (the WIT's `payload` world, as jco names them). */
interface Payload {
  describe(): { name: string; version: string; description?: string; files: string[] };
  packetSchema(typeName: string, params: string): { channels: { name: string; unit?: string; dtype: string }[]; timed: boolean };
  decodePackets(typeName: string, params: string, packets: { header: Uint8Array; payload: Uint8Array }[]): { rows: number; time: BigInt64Array | bigint[]; columns: Uint8Array[] };
  decodeField(field: string, params: string, dtype: string, size: number, count: number, data: Uint8Array): Uint8Array;
}

/** The plugin, instantiated with only what the sandbox gives it. */
async function instantiate(job: PluginJob): Promise<Decoder> {
  const files = await translated(job);
  const glue = new TextDecoder().decode(files.get('plugin.js'));
  const url = URL.createObjectURL(new Blob([glue], { type: 'text/javascript' }));
  const mod = (await import(/* @vite-ignore */ url)) as { instantiate(get: (n: string) => Promise<WebAssembly.Module>, imports: Record<string, unknown>): Promise<Decoder> };
  URL.revokeObjectURL(url);
  const file = job.file;
  const reader = typeof FileReaderSync === 'function' ? new FileReaderSync!() : null;
  const source = {
    len: () => BigInt(file?.size ?? 0),
    // a synchronous read: the WIT's `read` returns the bytes, and a Worker can read a File that way
    read: (offset: bigint, len: number) => {
      if (!file || !reader) return new Uint8Array();
      const o = Number(offset);
      return new Uint8Array(reader.readAsArrayBuffer(file.slice(o, Math.min(file.size, o + len))));
    },
  };
  const zero = { seconds: 0n, nanoseconds: 0 };
  // stdout and stderr go nowhere; no environment, arguments or working folder
  const sink = { write() {}, flush() {}, blockingFlush() {}, drop() {} };
  const sys = cli.createCli({ stdout: sink, stderr: sink, environment: {}, arguments: [], initialCwd: '/' });
  const imports: Record<string, unknown> = {
    'tlmbench:plugin/source': source,
    'tlmbench:plugin/types': {},
    // nothing from the machine: no environment, arguments or working folder, and clocks that say zero
    'wasi:cli/environment': { getEnvironment: () => [], getArguments: () => [], initialCwd: () => undefined },
    'wasi:cli/exit': sys.exit,
    'wasi:cli/stderr': sys.stderr,
    'wasi:cli/stdin': sys.stdin,
    'wasi:cli/stdout': sys.stdout,
    'wasi:cli/terminal-input': sys.terminalInput,
    'wasi:cli/terminal-output': sys.terminalOutput,
    'wasi:cli/terminal-stderr': sys.terminalStderr,
    'wasi:cli/terminal-stdin': sys.terminalStdin,
    'wasi:cli/terminal-stdout': sys.terminalStdout,
    'wasi:clocks/monotonic-clock': { ...clocks.monotonicClock, now: () => 0n, resolution: () => 1n },
    'wasi:clocks/wall-clock': { now: () => zero, resolution: () => ({ seconds: 0n, nanoseconds: 1 }) },
    'wasi:filesystem/preopens': { getDirectories: () => [] },
    'wasi:filesystem/types': filesystem.types,
    'wasi:io/error': io.error,
    'wasi:io/poll': io.poll,
    'wasi:io/streams': io.streams,
    'wasi:random/random': random.random,
    'wasi:random/insecure': random.insecure,
    'wasi:random/insecure-seed': random.insecureSeed,
  };
  return mod.instantiate((n) => {
    const b = files.get(n);
    if (!b) throw new Error(`the translation has no ${n}`);
    return WebAssembly.compile(capMemory(b, job.maxPages) as Uint8Array<ArrayBuffer>);
  }, imports);
}

const SIZES: Record<string, number> = { i8: 1, u8: 1, i16: 2, u16: 2, i32: 4, u32: 4, f32: 4, i64: 8, u64: 8, f64: 8 };

/** A little-endian column of `dtype` as f64. */
function asF64(b: Uint8Array, dtype: string, rows: number, out: Float64Array, at: number): void {
  const v = new DataView(b.buffer, b.byteOffset, b.byteLength);
  const size = SIZES[dtype];
  for (let i = 0; i < rows; i++) {
    const o = i * size;
    out[at + i] =
      dtype === 'f64' ? v.getFloat64(o, true)
      : dtype === 'f32' ? v.getFloat32(o, true)
      : dtype === 'i8' ? v.getInt8(o)
      : dtype === 'u8' ? v.getUint8(o)
      : dtype === 'i16' ? v.getInt16(o, true)
      : dtype === 'u16' ? v.getUint16(o, true)
      : dtype === 'i32' ? v.getInt32(o, true)
      : dtype === 'u32' ? v.getUint32(o, true)
      : dtype === 'i64' ? Number(v.getBigInt64(o, true))
      : Number(v.getBigUint64(o, true));
  }
}

const message = (e: unknown): string => {
  const p = e as { payload?: unknown; message?: string };
  return typeof p?.payload === 'string' ? p.payload : String(p?.message ?? e);
};

/** Packets a call takes at most, as the app's batches. */
const BATCH_PACKETS = 4096;

/** A payload plugin's work (D-ingest-09), as the app runs it: a packet type's rows (as a decoder plugin's columns, for
 *  `import_columns`) or each plugin field's values (f64, field after field, for `import_described`). */
function payload(p: Payload, w: PayloadWork): PluginReply {
  const bytes = new Uint8Array(w.bin);
  const view = new DataView(w.bin);
  if (w.kind === 'fields') {
    const out = new Float64Array(w.fields.length * w.records);
    let at = 0;
    w.fields.forEach((f, k) => {
      const n = f.size * w.records;
      const got = p.decodeField(f.name, f.params, f.dtype, f.size, w.records, bytes.subarray(at, at + n));
      at += n;
      if (got.length !== w.records * SIZES[f.dtype]) throw new Error(`field ${f.name}: ${got.length} bytes for ${w.records} values of ${f.dtype}`);
      asF64(got, f.dtype, w.records, out, k * w.records);
      post({ kind: 'progress', rows: (k + 1) * w.records });
    });
    return { kind: 'values', bin: out.buffer };
  }
  const schema = p.packetSchema(w.type, w.params);
  const seen = new Set<string>();
  for (const c of schema.channels) {
    if (!c.name || seen.has(c.name)) throw new Error(`channel names must be unique and not empty ("${c.name}")`);
    if (!SIZES[c.dtype]) throw new Error(`channel ${c.name}: no type ${c.dtype}`);
    seen.add(c.name);
  }
  const times: bigint[] = [];
  const cols: number[][] = schema.channels.map(() => []);
  let rows = 0;
  let at = 0;
  for (let done = 0; done < w.packets; ) {
    const batch: { header: Uint8Array; payload: Uint8Array }[] = [];
    while (done < w.packets && batch.length < BATCH_PACKETS) {
      const h = view.getUint32(at, true);
      const n = view.getUint32(at + 4, true);
      batch.push({ header: bytes.slice(at + 8, at + 8 + h), payload: bytes.slice(at + 8 + h, at + 8 + h + n) });
      at += 8 + h + n;
      done++;
    }
    const chunk = p.decodePackets(w.type, w.params, batch);
    if (schema.timed && chunk.time.length !== chunk.rows) throw new Error(`a batch of ${chunk.rows} rows has ${chunk.time.length} times`);
    if (chunk.columns.length !== schema.channels.length) throw new Error(`a batch has ${chunk.columns.length} columns for ${schema.channels.length} channels`);
    chunk.columns.forEach((b, k) => {
      const c = schema.channels[k];
      if (b.length !== chunk.rows * SIZES[c.dtype]) throw new Error(`channel ${c.name}: a column of ${b.length} bytes for ${chunk.rows} rows of ${c.dtype}`);
      const f = new Float64Array(chunk.rows);
      asF64(b, c.dtype, chunk.rows, f, 0);
      for (const x of f) cols[k].push(x);
    });
    if (schema.timed) for (const t of chunk.time) times.push(BigInt(t));
    rows += chunk.rows;
    post({ kind: 'progress', rows });
  }
  const bin = new Uint8Array((schema.timed ? rows * 8 : 0) + rows * 8 * schema.channels.length);
  const out = new DataView(bin.buffer);
  let o = 0;
  if (schema.timed) for (const t of times) (out.setBigInt64(o, t, true), (o += 8));
  for (const c of cols) for (const x of c) (out.setFloat64(o, x, true), (o += 8));
  const meta = {
    samples: rows,
    time: schema.timed ? { kind: 'column', name: 'time' } : { kind: 'rate', t0: '0', hz: 1 },
    index: !schema.timed,
    channels: schema.channels.map((c) => ({ name: c.name, unit: c.unit ?? null })),
    format: 'Descriptor',
    part: w.type,
  };
  return { kind: 'decoded', meta, bin: bin.buffer };
}

self.onmessage = async (e: MessageEvent<PluginJob>) => {
  const job = e.data;
  try {
    const p = await instantiate(job);
    if (job.op === 'about') return post({ kind: 'about', about: p.describe() });
    if (job.op === 'payload') {
      const r = payload(p as unknown as Payload, job.payload!);
      return post(r, r.kind === 'values' || r.kind === 'decoded' ? [r.bin] : []);
    }
    if (job.op === 'sniff') return post({ kind: 'sniff', yes: !!p.sniff(job.head ?? new Uint8Array(), job.name ?? '') });
    const schema = p.start();
    const seen = new Set<string>();
    for (const c of schema.channels) {
      if (!c.name || seen.has(c.name)) throw new Error(`channel names must be unique and not empty ("${c.name}")`);
      if (!SIZES[c.dtype]) throw new Error(`channel ${c.name}: no type ${c.dtype}`);
      seen.add(c.name);
    }
    const times: bigint[][] = [];
    const cols: Float64Array[][] = schema.channels.map(() => []);
    let rows = 0;
    for (;;) {
      const chunk = p.next(65_536);
      if (!chunk) break;
      if (schema.timed && chunk.time.length !== chunk.rows) throw new Error(`a chunk of ${chunk.rows} rows has ${chunk.time.length} times`);
      if (chunk.columns.length !== schema.channels.length) throw new Error(`a chunk has ${chunk.columns.length} columns for ${schema.channels.length} channels`);
      chunk.columns.forEach((b, k) => {
        const c = schema.channels[k];
        if (b.length !== chunk.rows * SIZES[c.dtype]) throw new Error(`channel ${c.name}: a column of ${b.length} bytes for ${chunk.rows} rows of ${c.dtype}`);
        const f = new Float64Array(chunk.rows);
        asF64(b, c.dtype, chunk.rows, f, 0);
        cols[k].push(f);
      });
      if (schema.timed) times.push([...chunk.time].map(BigInt));
      rows += chunk.rows;
      // each call has its own CPU budget: the page restarts its clock on this
      post({ kind: 'progress', rows });
    }
    const bin = new Uint8Array((schema.timed ? rows * 8 : 0) + rows * 8 * schema.channels.length);
    const view = new DataView(bin.buffer);
    let o = 0;
    if (schema.timed) for (const t of times.flat()) (view.setBigInt64(o, t, true), (o += 8));
    for (const parts of cols) for (const f of parts) for (const x of f) (view.setFloat64(o, x, true), (o += 8));
    const about = p.describe();
    const meta = {
      samples: rows,
      time: schema.timed ? { kind: 'column', name: 'time' } : { kind: 'rate', t0: '0', hz: 1 },
      index: !schema.timed,
      channels: schema.channels.map((c) => ({ name: c.name, unit: c.unit ?? null })),
      format: `${about.name} (plugin)`,
    };
    post({ kind: 'decoded', meta, bin: bin.buffer }, [bin.buffer]);
  } catch (err) {
    post({ kind: 'error', error: message(err) });
  }
};
