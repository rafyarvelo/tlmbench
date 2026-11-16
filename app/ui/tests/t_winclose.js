// A window other than the first (one a tab was dragged out to): its close button closes it, and it is forgotten, so the
// next start doesn't bring it back; the first window's saved place stays (sheet C3: the window stayed open, and was
// forgotten anyway, so quitting afterwards lost it).
import { run } from './lib.js';

run(
  'winclose',
  async ({ p, check, fake, until, settingsLS }) => {
    check('this page is the second window', await p.evaluate(() => window.__TAURI_INTERNALS__.metadata.currentWindow.label === 'win-2'));
    check('it shows the tab it was handed', await until(() => /b\.csv/.test(document.querySelector('#wsTabs')?.textContent ?? '')), await p.evaluate(() => document.querySelector('#wsTabs')?.textContent));
    check('and has saved itself with its session', await until(() => JSON.parse(localStorage.getItem('tlmbench.settings') ?? '{}').windows?.['win-2']?.session?.files?.[0] === '/data/b.csv'), (await settingsLS()).windows);
    await fake(() => window.__tlmFake.closeRequested());
    check('the close button closes it', await until(() => (window.__tlmFake.destroyed ?? []).includes('win-2')), await fake(() => window.__tlmFake.destroyed));
    const w = (await settingsLS()).windows;
    check('it is forgotten, the first window is kept', !('win-2' in w) && 'main' in w, w);
  },
  {
    files: { '/data/b.csv': {} },
    launch: null,
    settings: { windows: { main: { folder: '/data', frame: { x: 0, y: 0, w: 900, h: 600 } } } },
    setup: (fake) => {
      window.__TAURI_INTERNALS__.metadata.currentWindow.label = 'win-2';
      window.__TAURI_INTERNALS__.metadata.currentWebview = { windowLabel: 'win-2', label: 'win-2' };
      fake.handoff = { session: { files: ['/data/b.csv'], active: '/data/b.csv' }, folder: '/data' };
    },
  },
);
