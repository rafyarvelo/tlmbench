// The memory cap of browser-only mode's plugin sandbox (browser-plugin-worker.ts): a plugin's core modules are rewritten
// before they compile so that all their memories together can grow to the app's limit and no further, as wasmtime's
// limiter counts them in the app (crates/plugin/src/wasm.rs; R6 hostile-plugin review, Nov 23).

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

/** What a plugin's memories may still grow to, in WebAssembly pages (64 KB), shared by all its core modules. */
export interface PageBudget {
  left: number;
}

/**
 * A core module with every memory it defines given a maximum (its own, if smaller) out of `budget`, which it then
 * takes from: growing past it fails inside the plugin, as wasmtime's memory limit does in the app. A plugin's memories
 * together get the budget once (a component may define several, each of which could otherwise grow to the limit).
 */
export function capMemory(wasm: Uint8Array, budget: PageBudget): Uint8Array {
  // sections are copied as they are (a code section is megabytes: never spread into arguments, the stack overflows)
  const parts: Uint8Array[] = [wasm.subarray(0, 8)];
  let at = 8;
  while (at < wasm.length) {
    const id = wasm[at];
    const [size, n] = leb(wasm, at + 1);
    const start = at + 1 + n;
    const body = wasm.subarray(start, start + size);
    if (id !== 5) parts.push(wasm.subarray(at, start + size));
    else {
      const [count, c] = leb(body, 0);
      let p = c;
      const mem: number[] = [...lebBytes(count)];
      for (let i = 0; i < count; i++) {
        const flags = body[p++];
        if (flags & 0x04) throw new Error('the plugin uses 64-bit memory, which a browser tab can’t limit');
        if (flags & ~0x03) throw new Error('the plugin uses a kind of memory a browser tab can’t limit');
        const [min, a] = leb(body, p);
        p += a;
        let max = budget.left;
        if (flags & 0x01) {
          const [m, b] = leb(body, p);
          p += b;
          max = Math.min(m, budget.left);
        }
        if (min > max) throw new Error(`the plugin starts with ${Math.round((min * 65536) / 2 ** 20)} MB of memory, over the limit`);
        budget.left -= max;
        mem.push((flags & 0x02) | 0x01, ...lebBytes(min), ...lebBytes(max));
      }
      parts.push(new Uint8Array([5, ...lebBytes(mem.length), ...mem]));
    }
    at = start + size;
  }
  const out = new Uint8Array(parts.reduce((k, p) => k + p.length, 0));
  let k = 0;
  for (const p of parts) (out.set(p, k), (k += p.length));
  return out;
}
