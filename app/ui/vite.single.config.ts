import { readFileSync, readdirSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';
import { APP_VERSION } from './vite.version';

const out = fileURLToPath(new URL('../viewer/single/', import.meta.url));
const wasm = fileURLToPath(new URL('../../crates/web/target/wasm32-unknown-unknown/release/tlmbench_web.wasm', import.meta.url));

/** Packs files as the Rust side expects (see vite.viewer.config.ts). */
function pack(files: [string, Buffer][]): Buffer {
  return Buffer.concat(
    files.flatMap(([path, data]) => {
      const p = Buffer.from(path);
      const n = Buffer.alloc(4);
      n.writeUInt32LE(p.length);
      const m = Buffer.alloc(4);
      m.writeUInt32LE(data.length);
      return [n, p, m, data];
    }),
  );
}

// The viewer of a one-file export: the page, its script, its styles and fonts in one HTML file with a marker where the
// export's files go (`<!--TLM_FILES-->`), and the core beside it. Nothing is fetched, so the file opens from `file://`.
export default defineConfig({
  define: { __VIEWER__: 'true', __APP_VERSION__: APP_VERSION },
  plugins: [
    svelte(),
    {
      name: 'viewer-single',
      closeBundle() {
        let html = readFileSync(`${out}viewer.html`, 'utf8');
        const assets = (ext: string) => readdirSync(`${out}assets`).filter((f) => f.endsWith(ext));
        for (const css of assets('.css')) {
          const text = readFileSync(`${out}assets/${css}`, 'utf8');
          html = html.replace(new RegExp(`<link[^>]*href="[^"]*${css}"[^>]*>`), () => `<style>${text}</style>`);
        }
        for (const js of assets('.js')) {
          const text = readFileSync(`${out}assets/${js}`, 'utf8').replace(/<\/script/gi, '<\\/script');
          html = html.replace(new RegExp(`<script[^>]*src="[^"]*${js}"[^>]*></script>`), () => `<script type="module">${text}</script>`);
        }
        // the files go before the page's script, which reads them as it starts
        html = html.replace(/<body[^>]*>/, (b) => `${b}<!--TLM_FILES-->`).replace(/<link rel="modulepreload"[^>]*>/g, '');
        if (!existsSync(wasm)) console.warn('no tlmbench_web.wasm: build crates/web first (just viewer)');
        writeFileSync(`${out}../single.pack`, pack([['index.html', Buffer.from(html)], ['tlmbench_web.wasm', existsSync(wasm) ? readFileSync(wasm) : Buffer.alloc(0)]]));
        rmSync(out, { recursive: true, force: true });
      },
    },
  ],
  base: './',
  clearScreen: false,
  build: {
    target: 'es2022',
    outDir: '../viewer/single',
    emptyOutDir: true,
    assetsInlineLimit: 1 << 30,
    cssCodeSplit: false,
    modulePreload: false,
    rollupOptions: { input: 'viewer.html', output: { inlineDynamicImports: true } },
  },
});
