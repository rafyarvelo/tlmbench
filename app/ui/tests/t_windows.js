// Several windows: the settings every window shares (changes saved to the core, another window's applied at once),
// what each window keeps for itself (sidebar widths and panels), and the core holds per window.
import { run } from './lib.js';

const A = '/data/flight.tlm';
const B = '/data/bench run.csv';

run('windows', async ({ p, check, settingsLS, until, wait, fake }) => {
  const calls = () => p.evaluate(() => window.__tlmFake.calls.filter((c) => c === 'settings_save').length);
  const theme = () => p.evaluate(() => document.documentElement.dataset.theme ?? null);

  // a shared setting is sent to the core
  const before = await calls();
  await p.evaluate(() => window.__tlmSettings?.set?.('theme', 'light'));
  const hasHook = await p.evaluate(() => !!window.__tlmSettings);
  check('the settings hook is there', hasHook);
  await wait(100);
  check('a shared setting is saved to the core', (await calls()) > before, await calls());
  const saved = await p.evaluate(() => JSON.parse(window.__tlmFake.sharedSettings));
  check('what the core holds has the setting and no sidebar state', saved.theme === 'light' && !('leftWidth' in saved) && !('panelsClosed' in saved), Object.keys(saved).length);

  // a sidebar width is this window's own: no save to the core
  const n = await calls();
  await p.evaluate(() => window.__tlmSettings.set('leftWidth', 420));
  await wait(100);
  check('a sidebar width is not sent', (await calls()) === n);
  check('it is kept for this window', (await settingsLS()).leftWidth === 420);

  // another window changes a shared setting and a sidebar width
  await p.evaluate(() => window.__tlmFake.otherWindowSaved({ ...JSON.parse(window.__tlmFake.sharedSettings), theme: 'dark', leftWidth: 555 }));
  await wait(100);
  check('another window’s setting applies here at once', (await theme()) === 'dark', await theme());
  check('but not its sidebar width', (await p.evaluate(() => window.__tlmSettings.get('leftWidth'))) === 420);
  check('applying it does not send it back', (await calls()) === n);

  // Move to new window: the tab goes to a window of its own, opening the same file, and closes here
  await fake((a) => (window.__tlmFake.pick = a), A);
  await p.keyboard.press('ControlOrMeta+o');
  await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm');
  await fake((b) => (window.__tlmFake.pick = b), B);
  await p.keyboard.press('ControlOrMeta+o');
  await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'bench run.csv');
  await p.click('#wsTabs .wstab[aria-selected=true]', { button: 'right' });
  check('the tab menu offers Move to new window', await until(() => !!document.querySelector('[data-ws=newwin]')));
  await p.click('[data-ws=newwin]');
  check('a window opens', await until(() => window.__tlmFake.windows.length === 1));
  const w = await fake(() => window.__tlmFake.windows[0]);
  check('with that file open at its view', w.handoff.session.files.join() === B && w.handoff.session.active === B, JSON.stringify(w.handoff));
  check('and the tab is gone here, the other stays', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].map((n) => n.textContent).join() === 'flight.tlm'));
  check('the window is remembered with the first one’s place', (await settingsLS()).windows?.main?.frame?.w === 1200);
}, { files: { [A]: {}, [B]: {} } });
