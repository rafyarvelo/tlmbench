// Layouts and tabs: save as, open from New tab, delete with undo, open a shared view, close a tab with undo.
const T = require('./lib');
T.run('layouts', async ({ p, check, tabs, paneCount, text, wait }) => {
  // save the current tab as a layout
  await p.evaluate(() => document.querySelector('#wsTabs .wstab[aria-selected="true"] [data-tabm]').click()); await wait(250);
  await p.click('#wsSaveAs'); await wait(150);
  await p.fill('#wsName', 'Layout test'); await p.click('#wsNameOk'); await wait(400);
  check('tab takes the layout name', (await tabs()).some(t => /Layout test\*/.test(t)), await tabs());

  // New tab menu lists it; opening it makes a tab
  await p.click('#wsTabNew'); await wait(250);
  check('New tab lists your layout', /Layout test/.test(await text('#addMenu')));
  await p.evaluate(() => document.querySelector('#addMenu [data-new]').click()); await wait(300);
  await p.fill('#wsName', 'Empty one'); await p.click('#wsNameOk'); await wait(800);
  check('Empty layout… makes a new tab', (await tabs()).length === 2 && /Empty one\*/.test((await tabs())[1]), await tabs());
  await p.evaluate(() => [...document.querySelectorAll('#wsTabs .wstab')][0].click()); await wait(800);
  check('switching tabs keeps each layout', (await paneCount()) > 0);

  // delete it from the New tab menu, then undo
  await p.click('#wsTabNew'); await wait(250);
  await p.evaluate(() => { const row = [...document.querySelectorAll('#addMenu [data-open]')].find(b => /Layout test/.test(b.textContent)); const del = row.parentElement.querySelector('[data-lydel]') || document.querySelector('#addMenu [data-lydel]'); del.click(); }); await wait(400);
  check('deleted', !/Layout test/.test(await text('#addMenu') || ''));
  await p.keyboard.press('Escape'); await wait(150);
  await p.evaluate(() => { const b = [...document.querySelectorAll('#wsNote button')].find(b => /undo/i.test(b.textContent)); b && b.click(); }); await wait(400);
  await p.click('#wsTabNew'); await wait(250);
  check('undo brings it back', /Layout test/.test(await text('#addMenu')));
  await p.keyboard.press('Escape');

  // a shared view from .tlmbench opens in a new tab
  await p.evaluate(() => { const f = document.querySelector('#fileTree [data-fold=".tlmbench"]'); if (f && f.getAttribute('aria-expanded') === 'false') f.click(); }); await wait(300);
  const n = (await tabs()).length;
  await p.evaluate(() => document.querySelector('[data-shared]').click()); await wait(1500);
  check('shared view opens a tab', (await tabs()).length === n + 1, await tabs());
  check('it has panes', (await paneCount()) > 0);

  // close a tab, undo
  await p.evaluate(() => document.querySelector('#wsTabs .wstab[aria-selected="true"] [data-tabx]').click()); await wait(500);
  check('tab closed', (await tabs()).length === n, await tabs());
});
