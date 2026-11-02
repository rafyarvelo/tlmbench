// Tabs: a tab per open file, each keeping its panes, view and cursors while another is in front; ×, middle-click
// and the tab menu close tabs (and their files); tabs reorder by dragging; the tabs come back after a reload.
import { run } from './lib.js';

const A = '/data/a.tlm';
const B = '/data/b.csv';
const C = '/data/c.tlm';
const files = { [A]: {}, [B]: { channels: ['volts', 'amps'], units: ['V', 'A'] }, [C]: { channels: ['x', 'y', 'z'] } };

run('tabs', async ({ p, check, fake, paneCount, plotAt, until, settingsLS, wait }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const names = () => p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.querySelector('.nm').textContent + (x.getAttribute('aria-selected') === 'true' ? '*' : '')));
  const tab = (path) => `#wsTabs .wstab[data-tip="${path}"]`;
  for (const f of [B, C]) {
    await fake((f) => window.__tlmFake.drop([f]), f);
    await until((f) => document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip === f, f);
  }
  check('a tab per file, the last in front', (await names()).join() === 'a.tlm,b.csv,c.tlm*', await names());
  check('every file stays open', (await fake(() => window.__tlmFake.open.size)) === 3);

  // each tab keeps its own panes, view and cursors
  await p.click(tab(A));
  await until(() => window.__tlmApp.panes().length === 3);
  const c = await plotAt(0);
  await p.mouse.move(c.x, c.y);
  await p.mouse.wheel(0, -400);
  await wait(300);
  const zoomed = await app('view');
  await p.keyboard.press('c');
  const ca = await plotAt(0, 0.4);
  await p.mouse.click(ca.x, ca.y);
  await p.click('#chans .chan[data-ch="5"] .box');
  const panesA = JSON.stringify(await app('panes'));
  await p.click(tab(B));
  check('another tab, its own panes and view', await until(() => JSON.stringify(window.__tlmApp.panes()) === '[[0],[1]]') && (await app('view')).t0 !== zoomed.t0, await app('panes'));
  check('…and no cursors', (await app('cursors')).A === null);
  await p.click(tab(A));
  check('back: the panes as left', await until((s) => JSON.stringify(window.__tlmApp.panes()) === s, panesA));
  check('the view as left', JSON.stringify(await app('view')) === JSON.stringify(zoomed), [await app('view'), zoomed]);
  check('the cursor as left', (await app('cursors')).A !== null);
  check('every pane drawn after switching', await until(() => window.__tlmApp.drawn().every((d) => d && d.length)));

  // reorder by dragging; the order is saved
  await p.locator(tab(C)).dragTo(p.locator(tab(A)));
  check('drag reorders tabs', (await names()).join() === 'c.tlm,a.tlm*,b.csv', await names());

  // middle-click and × close tabs and their files; the neighbour comes to the front
  await p.click(tab(B), { button: 'middle' });
  check('middle-click closes a tab and its file', (await names()).join() === 'c.tlm,a.tlm*' && (await until(() => window.__tlmFake.open.size === 2)), await names());
  await p.hover(tab(A));
  await p.click(`${tab(A)} .tx`);
  check('× closes the tab in front; the next one comes forward', await until(() => document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip === '/data/c.tlm'), await names());

  // the tab menu
  for (const f of [A, B]) {
    await fake((f) => window.__tlmFake.drop([f]), f);
    await until((f) => document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip === f, f);
  }
  await p.click(tab(A), { button: 'right' });
  check('the tab menu', /Close tab.*Close other tabs.*Close all tabs/s.test(await p.textContent('.tabmenu')));
  await p.locator('.tabmenu .mi', { hasText: /^Close other tabs$/ }).click();
  check('Close other tabs', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 1) && (await names()).join() === 'a.tlm*', await names());

  // the tabs come back after a reload, the same one in front
  await fake((f) => window.__tlmFake.drop([f]), C);
  await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2);
  await p.click(tab(A));
  check('saved', JSON.stringify((await settingsLS()).session) === JSON.stringify({ files: [A, C], active: A }), (await settingsLS()).session);
  await p.reload();
  check('the tabs come back', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2) && (await names()).join() === 'a.tlm*,c.tlm', await names());
  await p.click(tab(A), { button: 'right' });
  await p.locator('.tabmenu .mi', { hasText: /^Close all tabs$/ }).click();
  check('Close all tabs empties the window', await until(() => !document.querySelector('#wsTabs .wstab')) && (await paneCount()) === 0);
}, { files, launch: A });
