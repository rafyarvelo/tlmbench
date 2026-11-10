// The UI in a browser tab: the app's commands go to the daemon over a WebSocket instead of Tauri's IPC (design: Daemon and
// clients). Installed before anything calls `invoke`, only when the page isn't inside the app.
//
// Frames from the daemon (binary): [kind u8][id u32 LE][payload]: 0 a JSON reply, 1 a raw reply (an ArrayBuffer, as
// Tauri gives it), 2 an error, 3 and 4 a JSON or raw channel message (payload: [index u32 LE] then the message), 5 an
// event. The daemon names this window first, in a text frame.

type Handler = (data: unknown) => void;

export async function installBrowserCore(): Promise<void> {
  const q = new URLSearchParams(location.search);
  // the token comes with the page's address once; it stays for reloads of the tab
  const token = q.get('token') ?? sessionStorage.getItem('tlmbench.token') ?? '';
  sessionStorage.setItem('tlmbench.token', token);
  const wanted = q.get('window') ?? sessionStorage.getItem('tlmbench.window');
  const ws = new WebSocket(`ws://${location.host}/ws?token=${encodeURIComponent(token)}${wanted ? `&window=${encodeURIComponent(wanted)}` : ''}`);
  ws.binaryType = 'arraybuffer';

  const callbacks = new Map<number, Handler>();
  const pending = new Map<number, { ok: (v: unknown) => void; err: (e: unknown) => void }>();
  const listeners = new Map<string, number[]>();
  let nextCb = 1;
  let nextId = 1;

  const label = await new Promise<string>((ok, err) => {
    ws.onmessage = (e) => typeof e.data === 'string' && ok(JSON.parse(e.data).hello.label);
    ws.onerror = () => err(new Error('The TelemetryBench daemon refused the connection. Open the address it printed, with its token.'));
  });
  // a reload keeps the window (and the files it holds)
  sessionStorage.setItem('tlmbench.window', label);

  const text = new TextDecoder();
  ws.onmessage = (e) => {
    if (typeof e.data === 'string') return;
    const f = new DataView(e.data as ArrayBuffer);
    const kind = f.getUint8(0);
    const id = f.getUint32(1, true);
    const body = (e.data as ArrayBuffer).slice(5);
    if (kind === 0 || kind === 1 || kind === 2) {
      const p = pending.get(id);
      pending.delete(id);
      if (!p) return;
      if (kind === 0) p.ok(JSON.parse(text.decode(body)));
      else if (kind === 1) p.ok(body);
      else p.err(JSON.parse(text.decode(body)));
    } else if (kind === 3 || kind === 4) {
      const index = new DataView(body).getUint32(0, true);
      const rest = body.slice(4);
      callbacks.get(id)?.({ message: kind === 3 ? JSON.parse(text.decode(rest)) : rest, index });
    } else if (kind === 5) {
      const { event, payload } = JSON.parse(text.decode(body));
      for (const h of listeners.get(event) ?? []) callbacks.get(h)?.({ event, id: h, payload });
    }
  };

  const call = (cmd: string, args: unknown) =>
    new Promise<unknown>((ok, err) => {
      const id = nextId++;
      pending.set(id, { ok, err });
      // bytes go as numbers: the JSON has no other way to carry them
      ws.send(JSON.stringify({ id, cmd, args: args ?? {} }, (_k, v) => (v instanceof Uint8Array ? Array.from(v) : v instanceof ArrayBuffer ? Array.from(new Uint8Array(v)) : v)));
    });

  const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
    'plugin:event|listen': (a) => {
      const list = listeners.get(a.event as string) ?? [];
      list.push(a.handler as number);
      listeners.set(a.event as string, list);
      return a.handler;
    },
    'plugin:event|unlisten': () => null,
    // no file dialogs in a browser: the path is on the machine the daemon runs on
    'plugin:dialog|open': (a) => {
      const o = (a.options ?? {}) as { directory?: boolean };
      return prompt(o.directory ? 'Folder on the machine running TelemetryBench' : 'Recording on the machine running TelemetryBench') || null;
    },
    'plugin:dialog|save': () => prompt('Save to this path on the machine running TelemetryBench') || null,
    'plugin:dialog|message': (a) => (confirm(String(a.message)) ? 'Stop' : 'No'),
    'plugin:window|set_title': (a) => {
      document.title = String(a.value);
      return null;
    },
    // a window of the app is a tab here: the daemon makes the window's state, the new tab attaches to it
    window_new: async (a) => {
      const name = (await call('window_new', a)) as string;
      window.open(`${location.pathname}?token=${encodeURIComponent(token)}&window=${encodeURIComponent(name)}`, '_blank');
      return name;
    },
    window_frame: () => null,
    window_place: () => null,
    window_at: () => null,
  };

  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label }, currentWebview: { windowLabel: label, label } },
    invoke: (cmd: string, args?: Record<string, unknown>) => (commands[cmd] ? Promise.resolve(commands[cmd](args ?? {})) : call(cmd, args)),
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
}
