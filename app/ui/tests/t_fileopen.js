// What a file opens with (Settings → When a file opens, Files opened for the first time, Forget layouts), Clicking a
// row (Act at once), and the Quiet interface.
import { run } from './lib.js';

const files = { '/data/a.tlm': {}, '/data/b.tlm': {}, '/data/c.tlm': {}, '/data/d.tlm': {} };

run('fileopen', async ({ p, check, fake, until, paneCount, visible, text, settingsLS, wait }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const set = async (key, v) => {
    await p.click('#b-settings');
    await p.click('#setPanel [data-sv="all"]');
    await p.selectOption(`[data-set="${key}"]`, v);
    await p.click('[data-rail="explorer"]');
  };
  const openFile = async (path) => {
    await fake((d) => (window.__tlmFake.pick = d), path);
    await p.click('#b-more');
    await p.click('.moremenu .mi:has-text("Open recording")');
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === path.split('/').pop());
    await wait(300);
  };

  check('a new file opens with starter charts', (await paneCount()) === 3);
  await set('fileOpenNew', 'table');
  await openFile('/data/b.tlm');
  check('a file opened for the first time follows "first time": one table', (await paneCount()) === 1 && (await p.$$('#stack [data-table]')).length === 1);
  await set('fileOpenNew', 'values');
  await openFile('/data/c.tlm');
  check('one values pane', (await paneCount()) === 1 && (await p.$$('#stack [data-tiles]')).length === 1);
  await set('fileOpenNew', 'none');
  await openFile('/data/d.tlm');
  check('nothing plotted', (await paneCount()) === 0);

  // a file left with panes comes back with them (Remember), unless "When a file opens" says otherwise
  await p.click('#wsTabs .wstab[data-tip="/data/a.tlm"]');
  await wait(300);
  check('the first file keeps its own panes (Remember)', (await paneCount()) === 3, await paneCount());
  await set('fileOpen', 'starterboth');
  await p.hover('#wsTabs .wstab[data-tip="/data/a.tlm"]');
  await p.click('#wsTabs .wstab[data-tip="/data/a.tlm"] .tx');
  await wait(300);
  await openFile('/data/a.tlm');
  check('a starter choice overrides what was remembered', (await paneCount()) === 3 && (await p.$$('#stack [data-table]')).length === 3, await paneCount());
  await set('fileOpen', 'remember');
  await p.click('#b-settings');
  await p.click('#setForget');
  check('Forget layouts clears them', Object.keys((await settingsLS()).layouts).length === 0);
  await p.click('[data-rail="explorer"]');

  // Clicking a row: Act at once
  await set('clickMode', 'act');
  const before = JSON.stringify(await app('panes'));
  await p.click('#explorer .row.chan[data-ch="0"] .nm');
  check('one click plots a channel', JSON.stringify(await app('panes')) !== before, await app('panes'));
  await fake((d) => (window.__tlmFake.pickFolder = d), '/data');
  await p.click('#exFolder');
  await until(() => document.querySelectorAll('#explorer .row.file').length >= 4);
  await p.click('#explorer .row.file[data-path$="b.tlm"]');
  check('one click opens a file', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'b.tlm'));

  // Quiet interface hides the "pane N" badges
  await p.click('#explorer .row.chan[data-ch="1"] .box').catch(() => {});
  await set('clickMode', 'select');
  const where = () => p.evaluate(() => [...document.querySelectorAll('#explorer [data-where]')].filter((e) => e.offsetParent).length);
  await p.click('#stack [data-pane="0"] .phead').catch(() => {});
  const shown = await where();
  await p.click('#b-settings');
  await p.click('#setPanel [data-set="quietUI"]');
  await p.click('[data-rail="explorer"]');
  check('Quiet interface hides the badges', (await where()) === 0 && shown >= 0, [shown, await where()]);
}, { files, launch: '/data/a.tlm' });
