import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';
import { APP_VERSION } from './vite.version';

// Tauri serves the build from ../ui/dist; in dev it loads the Vite server on 5173.
export default defineConfig({
  define: { __VIEWER__: 'false', __APP_VERSION__: APP_VERSION },
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: 'es2022', outDir: 'dist', emptyOutDir: true },
});
