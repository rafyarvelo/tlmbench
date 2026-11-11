// The core compiled to WASM (crates/web) behind one `call`, and the stand-in for Tauri's IPC that the viewer (an HTML export)
// and browser-only mode (files opened in the page) both install before anything calls `invoke`.

type Handler = (data: unknown) => void;
export type Core = (cmd: string, args?: unknown, bin?: Uint8Array) => unknown;

const enc = new TextEncoder();
const dec = new TextDecoder();

/** Loads the WASM core. JSON replies parse, binary ones come back as an ArrayBuffer, errors throw. */
export async function loadCore(url: string): Promise<Core> {
  const res = await fetch(url);
  const { instance } = await WebAssembly.instantiateStreaming(res, {});
  const x = instance.exports as unknown as {
    memory: WebAssembly.Memory;
    alloc(n: number): number;
    dealloc(p: number, n: number): void;
    call(c: number, cl: number, a: number, al: number, b: number, bl: number): number;
    reply_len(): number;
  };
  const put = (b: Uint8Array) => {
    const p = x.alloc(b.length);
    new Uint8Array(x.memory.buffer, p, b.length).set(b);
    return p;
  };
  return function call(cmd: string, args: unknown = null, bin: Uint8Array = new Uint8Array(0)): unknown {
    const c = enc.encode(cmd);
    const a = args === null ? new Uint8Array(0) : enc.encode(JSON.stringify(args));
    const [pc, pa, pb] = [put(c), put(a), put(bin)];
    const p = x.call(pc, c.length, pa, a.length, pb, bin.length);
    const out = new Uint8Array(x.memory.buffer, p, x.reply_len()).slice();
    x.dealloc(pc, c.length);
    x.dealloc(pa, a.length);
    x.dealloc(pb, bin.length);
    const body = out.subarray(1);
    if (out[0] === 2) throw new Error(dec.decode(body));
    return out[0] === 0 ? JSON.parse(dec.decode(body)) : body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength);
  };
}

export type Commands = Record<string, (a: Record<string, unknown>) => unknown | Promise<unknown>>;

/** The IPC the app's `invoke` goes through, answered by `commands`. Returns what the page needs to deliver events to the app. */
export function installInternals(commands: Commands, debug = false) {
  const callbacks = new Map<number, Handler>();
  const listeners = new Map<string, number[]>();
  let nextCb = 1;
  const none = () => null;
  const base: Commands = {
    'plugin:event|listen': (a) => {
      const list = listeners.get(a.event as string) ?? [];
      list.push(a.handler as number);
      listeners.set(a.event as string, list);
      return a.handler;
    },
    'plugin:event|unlisten': none,
    'plugin:window|set_title': (a) => ((document.title = String(a.value)), null),
  };
  const all = { ...base, ...commands };
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
    invoke: async (cmd: string, args?: Record<string, unknown>) => {
      if (debug) console.log('invoke', cmd, JSON.stringify(args ?? {}).slice(0, 200));
      // a real IPC reply comes on a task of its own: answering in microtasks alone would starve timers and fetches
      await new Promise<void>((r) => setTimeout(r, 0));
      const h = all[cmd];
      if (!h) throw new Error(`${cmd} is not available here`);
      return h(args ?? {});
    },
    transformCallback(cb: Handler, once = false) {
      const id = nextCb++;
      callbacks.set(id, (d) => {
        if (once) callbacks.delete(id);
        return cb(d);
      });
      return id;
    },
    unregisterCallback: (id: number) => callbacks.delete(id),
    runCallback: (id: number, d: unknown) => callbacks.get(id)?.(d),
    convertFileSrc: (p: string) => p,
  };
  (window as unknown as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  return {
    callbacks,
    /** Delivers an event to what the app listens with (a drop, for one). */
    emit(event: string, payload: unknown) {
      for (const h of listeners.get(event) ?? []) callbacks.get(h)?.({ event, id: h, payload });
    },
  };
}

const none = () => null;

/** Commands the app makes that have no meaning here (no folders, windows, settings in the core, check sets or recipes). */
export const stubs: Commands = {
  settings_get: none,
  settings_save: none,
  set_expr_env: none,
  hold: none,
  holders: () => [],
  usage: () => ({ used: 0, budget: 0 }),
  check_sets_list: () => [],
  recipes_list: () => [],
  plugins_list: () => [],
  shared_views: () => [],
  needs_descriptor: () => false,
  packet_parts: () => [],
  window_frame: none,
  window_place: none,
  window_at: none,
  window_take: none,
  group_state: none,
  missing_between: () => [],
  filter_check: none,
  browser_access: () => ({ on: false }),
  cache_info: () => ({ bytes: 0, files: 0, root: '' }),
};
