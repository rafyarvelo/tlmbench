// The viewer's entry: the same UI as the app, over an export folder (see viewer.ts).
export {};
const { installViewerCore, explainLoadFailure } = await import('./viewer');
try {
  await installViewerCore();
} catch (e) {
  // say why instead of leaving an empty page (the usual cause: a folder export opened from file://)
  document.body.textContent = explainLoadFailure(e);
  document.body.style.cssText = 'font:13px/1.5 system-ui,sans-serif;max-width:60ch;margin:12vh auto;padding:0 16px';
  throw e;
}
document.body.classList.add('viewer');

const { mount } = await import('svelte');
const { default: App } = await import('./App.svelte');
mount(App, { target: document.body });
await import('./app');
