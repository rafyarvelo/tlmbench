import { copyFileSync, existsSync, readdirSync, readFileSync, renameSync, statSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

/** Every file under `dir` as a pack the Rust side embeds: per file a u32 LE path length, the path, a u32 LE data length, the data. */
function pack(dir: string): Buffer {
  const parts: Buffer[] = [];
  const walk = (d: string, rel: string) => {
    for (const name of readdirSync(d).sort()) {
      const p = d + name;
      if (statSync(p).isDirectory()) walk(`${p}/`, `${rel}${name}/`);
      else {
        const path = Buffer.from(rel + name);
        const data = readFileSync(p);
        const n = Buffer.alloc(4);
        n.writeUInt32LE(path.length);
        const m = Buffer.alloc(4);
        m.writeUInt32LE(data.length);
        parts.push(n, path, m, data);
      }
    }
  };
  walk(dir, '');
  return Buffer.concat(parts);
}

const out = fileURLToPath(new URL('../viewer/dist/', import.meta.url));
const wasm = fileURLToPath(new URL('../../crates/web/target/wasm32-unknown-unknown/release/tlmbench_web.wasm', import.meta.url));

// The viewer a static export ships: relative paths, so the folder opens from any place on a static host or share.
// The page is index.html, and the core (crates/web, built by `just viewer`) sits beside it.
export default defineConfig({
  plugins: [
    svelte(),
    {
      name: 'viewer-layout',
      closeBundle() {
        renameSync(`${out}viewer.html`, `${out}index.html`);
        if (existsSync(wasm)) copyFileSync(wasm, `${out}tlmbench_web.wasm`);
        else console.warn('no tlmbench_web.wasm: build crates/web first (just viewer)');
        writeFileSync(`${out}../viewer.pack`, pack(out));
      },
    },
  ],
  base: './',
  clearScreen: false,
  build: { target: 'es2022', outDir: '../viewer/dist', emptyOutDir: true, rollupOptions: { input: 'viewer.html' } },
});
