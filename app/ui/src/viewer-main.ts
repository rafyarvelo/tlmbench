// The viewer's entry: the same UI as the app, over an export folder (see viewer.ts).
export {};const { installViewerCore } = await import('./viewer');
await installViewerCore();

const { mount } = await import('svelte');
const { default: App } = await import('./App.svelte');
mount(App, { target: document.body });
await import('./app');
