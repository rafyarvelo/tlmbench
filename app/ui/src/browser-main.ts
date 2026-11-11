// The entry of browser-only mode: the same UI as the app, with the core in the page (see browser-only.ts).
export {};
const { installBrowserOnly } = await import('./browser-only');
await installBrowserOnly();

const { mount } = await import('svelte');
const { default: App } = await import('./App.svelte');
mount(App, { target: document.body });
await import('./app');
