import { copyFileSync, existsSync, renameSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

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
      },
    },
  ],
  base: './',
  clearScreen: false,
  build: { target: 'es2022', outDir: '../viewer/dist', emptyOutDir: true, rollupOptions: { input: 'viewer.html' } },
});
