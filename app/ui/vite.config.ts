import { defineConfig } from 'vite';

// Tauri serves the build from ../ui/dist; in dev it loads the Vite server on 5173.
export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: 'es2022', outDir: 'dist', emptyOutDir: true },
});
