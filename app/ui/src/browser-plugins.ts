// Browser-only mode's decoder plugins (ledger D-brow-09): WASM plugins you add to this browser (kept in its private storage)
// and the open folder's `.tlmbench/plugins/` (they load once you trust the folder, as in the app). Each call runs in a Worker
// of its own (browser-plugin-worker.ts) with the app's limits: no files or network, 1 GiB of memory, 30 s for a call; a
// Worker that runs past it is ended. Native plugins can't run in a browser: they are listed as needing the app.

import type { PluginJob, PluginReply } from './browser-plugin-worker';
import type { PluginRow } from './plugins.svelte';

const STORE = 'tlmbench.plugins';
const DIR = 'tlmbench-plugins';
/** The app's decoder limits (crates/plugin/src/wasm.rs `Limits::DECODER`). */
const LIMITS = { cpuMs: 30_000, maxPages: (1 << 30) / 65536 };
const limits = () => ({ ...LIMITS, ...((globalThis as { __tlmPluginLimits?: Partial<typeof LIMITS> }).__tlmPluginLimits ?? {}) });
export const NATIVE = /\.(dylib|so|dll)$/i;

interface About {
  name: string;
  version: string;
  description?: string;
  files: string[];
}
/** A plugin this page can run: added to this browser, or from the open folder (`folder`). */
export interface WasmPlugin {
  id: string;
  file: string;
  about: About;
  folder: string | null;
  bytes: () => Promise<ArrayBuffer>;
}

const digest = async (b: ArrayBuffer) => [...new Uint8Array(await crypto.subtle.digest('SHA-256', b))].slice(0, 16).map((x) => x.toString(16).padStart(2, '0')).join('');

/** Runs one job in a Worker of its own, ended when a call runs past the CPU limit (each chunk restarts the clock). */
function run(job: Omit<PluginJob, 'maxPages'>, onProgress?: (rows: number) => void): Promise<PluginReply> {
  const lim = limits();
  return new Promise((done) => {
    const w = new Worker(new URL('./browser-plugin-worker.ts', import.meta.url), { type: 'module' });
    let timer = 0;
    const arm = () => {
      clearTimeout(timer);
      timer = window.setTimeout(() => {
        w.terminate();
        done({ kind: 'error', error: `it ran for more than ${Math.round(lim.cpuMs / 1000)} s and was stopped` });
      }, lim.cpuMs);
    };
    w.onmessage = (e: MessageEvent<PluginReply>) => {
      if (e.data.kind === 'progress') return (arm(), onProgress?.(e.data.rows));
      clearTimeout(timer);
      w.terminate();
      done(e.data);
    };
    w.onerror = (e) => {
      clearTimeout(timer);
      w.terminate();
      done({ kind: 'error', error: e.message || 'the plugin failed' });
    };
    arm();
    w.postMessage({ ...job, maxPages: lim.maxPages });
  });
}

// ---------- the plugins added to this browser ----------

interface Kept {
  id: string;
  file: string;
  about: About;
}
const kept = (): Kept[] => {
  try {
    return JSON.parse(localStorage.getItem(STORE) ?? '[]') as Kept[];
  } catch {
    return [];
  }
};
async function dir(): Promise<FileSystemDirectoryHandle> {
  return (await navigator.storage.getDirectory()).getDirectoryHandle(DIR, { create: true });
}
const own = (): WasmPlugin[] =>
  kept().map((k) => ({ ...k, folder: null, bytes: async () => (await (await (await dir()).getFileHandle(`${k.id}.wasm`)).getFile()).arrayBuffer() }));

/** Add plugin…: asks the plugin for its name and file patterns (in the sandbox), then keeps it in this browser. */
export async function addPlugin(f: File): Promise<string> {
  if (NATIVE.test(f.name)) throw new Error('Native plugins need the app: a browser tab runs WASM plugins only.');
  const bytes = await f.arrayBuffer();
  const id = await digest(bytes);
  const r = await run({ op: 'about', wasm: bytes, id });
  if (r.kind !== 'about') throw new Error(`${f.name} isn't a decoder plugin: ${r.kind === 'error' ? r.error : 'no answer'}`);
  const w = await (await (await dir()).getFileHandle(`${id}.wasm`, { create: true })).createWritable();
  await w.write(bytes);
  await w.close();
  localStorage.setItem(STORE, JSON.stringify([...kept().filter((k) => k.id !== id && k.file !== f.name), { id, file: f.name, about: r.about }]));
  return r.about.name;
}

export async function removePlugin(file: string): Promise<void> {
  const k = kept().find((x) => x.file === file);
  if (!k) return;
  localStorage.setItem(STORE, JSON.stringify(kept().filter((x) => x.file !== file)));
  await (await dir()).removeEntry(`${k.id}.wasm`).catch(() => {});
}

// ---------- the open folder's .tlmbench/plugins/ ----------

let shared: { folder: string; wasm: { file: string; bytes: ArrayBuffer }[]; native: string[] } = { folder: '', wasm: [], native: [] };
const sharedAbout = new Map<string, About | { error: string }>();
/** The open folder's plugins as described, for finding the files they name without waiting. */
let sharedKnown: About[] = [];

/** The open folder's plugins (read through its handle), described in the sandbox once each. */
export async function setShared(folder: string, wasm: { file: string; bytes: ArrayBuffer }[], native: string[]): Promise<void> {
  shared = { folder, wasm, native };
  for (const p of wasm) {
    const id = await digest(p.bytes);
    if (sharedAbout.has(id)) continue;
    const r = await run({ op: 'about', wasm: p.bytes, id });
    sharedAbout.set(id, r.kind === 'about' ? r.about : { error: r.kind === 'error' ? r.error : 'no answer' });
  }
  const abouts = await Promise.all(wasm.map(async (p) => sharedAbout.get(await digest(p.bytes))));
  sharedKnown = abouts.filter((a): a is About => !!a && !('error' in a));
}

/** Every WASM plugin that may run: yours, then the folder's when it is trusted. */
async function runnable(trusted: string[]): Promise<WasmPlugin[]> {
  const out = own();
  const pluginsDir = `${shared.folder}/.tlmbench/plugins`;
  if (shared.folder && trusted.includes(pluginsDir)) {
    for (const p of shared.wasm) {
      const id = await digest(p.bytes);
      const a = sharedAbout.get(id);
      if (a && !('error' in a)) out.push({ id, file: p.file, about: a, folder: pluginsDir, bytes: async () => p.bytes });
    }
  }
  return out;
}

/** What Settings › Plugins lists, as the app's `plugins_list` gives it. */
export async function rows(trusted: string[]): Promise<PluginRow[]> {
  const out: PluginRow[] = own().map((p) => ({ file: p.file, path: `browser://plugins/${p.file}`, tier: 'wasm', kind: 'decoder', name: p.about.name, version: p.about.version, description: p.about.description ?? null, shared: false, folder: null, status: { state: 'loaded' } }));
  const pluginsDir = `${shared.folder}/.tlmbench/plugins`;
  for (const p of shared.wasm) {
    const a = sharedAbout.get(await digest(p.bytes));
    const ok = a && !('error' in a);
    out.push({
      file: p.file,
      path: `${pluginsDir}/${p.file}`,
      tier: 'wasm',
      kind: 'decoder',
      name: ok ? a.name : p.file,
      version: ok ? a.version : null,
      description: ok ? (a.description ?? null) : null,
      shared: true,
      folder: pluginsDir,
      status: !ok ? { state: 'failed', why: a && 'error' in a ? a.error : 'not read yet' } : trusted.includes(pluginsDir) ? { state: 'loaded' } : { state: 'untrusted' },
    });
  }
  // a native plugin can't run in a tab: listed, saying so
  for (const n of shared.native) out.push({ file: n, path: `${pluginsDir}/${n}`, tier: 'native', kind: 'decoder', name: n, version: null, description: null, shared: true, folder: pluginsDir, status: { state: 'failed', why: 'Native plugins need the app: a browser tab runs WASM plugins only.' } });
  return out;
}

const named = (p: WasmPlugin, name: string) => p.about.files.some((g) => matches(g, name));
function matches(pattern: string, name: string): boolean {
  const re = new RegExp(`^${pattern.split('*').map((s) => s.replace(/[.+?^${}()|[\]\\]/g, '\\$&')).join('.*')}$`);
  return re.test(name);
}

/** Whether a plugin's file patterns name this file (Explorer lists such a file as openable): yours, or the open folder's
 *  once it is trusted. */
export const namesFile = (name: string, trusted: string[]): boolean =>
  own().some((p) => named(p, name)) || (!!shared.folder && trusted.includes(`${shared.folder}/.tlmbench/plugins`) && sharedKnown.some((a) => a.files.some((g) => matches(g, name))));

/** The open folder's plugin whose patterns name a file, trusted or not (opening one that waits for trust says so). */
export const folderPluginNaming = (name: string): string | null => sharedKnown.find((a) => a.files.some((g) => matches(g, name)))?.name ?? null;

/** The first plugin that claims a file, as the app asks them: its patterns, then its own sniff of the first bytes. */
export async function claim(f: File, trusted: string[]): Promise<WasmPlugin | null> {
  const list = await runnable(trusted);
  if (!list.length) return null;
  const byName = list.find((p) => named(p, f.name));
  if (byName) return byName;
  const head = new Uint8Array(await f.slice(0, 4096).arrayBuffer());
  for (const p of list) {
    const r = await run({ op: 'sniff', wasm: await p.bytes(), id: p.id, head, name: f.name });
    if (r.kind === 'sniff' && r.yes) return p;
  }
  return null;
}

/** Decodes a file through a plugin: what the page's core loads (`import_columns`). */
export async function decode(p: WasmPlugin, f: File, onProgress?: (rows: number) => void): Promise<{ meta: Record<string, unknown>; bin: ArrayBuffer }> {
  const r = await run({ op: 'decode', wasm: await p.bytes(), id: p.id, file: f, name: f.name }, onProgress);
  if (r.kind === 'decoded') return { meta: r.meta, bin: r.bin };
  throw new Error(`${p.about.name}: ${r.kind === 'error' ? r.error : 'no answer'}`);
}
