// Mounts the window, then starts the app (app.ts finds its elements by id).
// The settings every window shares come from the core first: the settings module reads them when it loads.

import { invoke } from '@tauri-apps/api/core';

// in a browser tab the commands go to the daemon (the app's own window has Tauri's IPC)
if (!('__TAURI_INTERNALS__' in window)) await (await import('./browser')).installBrowserCore();
// the app's own window takes Finder drops natively, which stops the page's own drags: drive those by the mouse
else (await import('./dnd')).installPointerDnd();

try {
  const text = await invoke<string | null>('settings_get');
  if (text) localStorage.setItem('tlmbench.settings', text);
} catch {
  /* the browser's copy stands */
}

const { mount } = await import('svelte');
const { default: App } = await import('./App.svelte');
const { followOtherWindows } = await import('./settings.svelte');

mount(App, { target: document.body });
followOtherWindows();
await import('./app');
