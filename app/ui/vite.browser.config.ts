import { copyFileSync, existsSync, renameSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

const out = fileURLToPath(new URL('../browser/dist/', import.meta.url));
const wasm = fileURLToPath(new URL('../../crates/web/target/wasm32-unknown-unknown/release/tlmbench_web.wasm', import.meta.url));

// Browser-only mode as a static site: relative paths, so it can be served from any folder of a static host.
export default defineConfig({
  plugins: [
    svelte(),
    {
      name: 'browser-layout',
      closeBundle() {
        renameSync(`${out}browser.html`, `${out}index.html`);
        if (existsSync(wasm)) copyFileSync(wasm, `${out}tlmbench_web.wasm`);
        else console.warn('no tlmbench_web.wasm: build crates/web first (just viewer)');
      },
    },
  ],
  base: './',
  clearScreen: false,
  build: { target: 'es2022', outDir: '../browser/dist', emptyOutDir: true, rollupOptions: { input: 'browser.html' } },
});
