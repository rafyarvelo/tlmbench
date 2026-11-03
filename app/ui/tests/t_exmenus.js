// Explorer's menus: the folder menu (open, expand and collapse all, remove the folder, close all files), the ⋯ menu
// (add every channel shown, a new chart or table, categories, files that can't be opened, swapping sides), the filter
// matching units and categories, and "Open only this".
import { run } from './lib.js';

const DIR = '/data/flights';
const files = { [`${DIR}/a/flight.tlm`]: {}, [`${DIR}/a/hover.csv`]: { channels: ['volts', 'amps'], units: ['V', 'A'] } };

run('exmenus', async ({ p, check, text, visible, fake, paneCount, until, settingsLS, wait }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const rows = () => p.$$eval('#explorer .tree .row', (r) => r.map((x) => x.querySelector('.nm').textContent));
  await fake(([d, u]) => ((window.__tlmFake.pickFolder = d), (window.__tlmFake.unopenable = [u])), [DIR, `${DIR}/a/notes.json`]);
  await p.click('#exFolder');
  await until(() => document.querySelectorAll('#explorer .row.file').length >= 2);
  check('files the app cannot open stay out of the list', !(await rows()).includes('notes.json'), await rows());

  await p.click('#exMore');
  check('⋯ says how many are hidden', /Show files that can't be opened \(1 hidden\)/.test(await text('.exmenu')), await text('.exmenu'));
  await p.click('.exmenu [data-ex="showall"]');
  check('and Show lists them, dimmed', (await rows()).includes('notes.json') && (await p.$$('#explorer .row.cant')).length === 1);
  await p.dblclick('#explorer .row.cant');
  check('they do not open', (await p.$$('#wsTabs .wstab')).length === 0);
  await p.click('#exMore');
  await p.click('.exmenu [data-ex="showall"]');

  // folder menu
  await p.click('#exFolderMenu');
  await p.click('.exmenu [data-ex="collapse"]');
  check('Collapse all folders (menu)', (await rows()).join() === 'flights,a' || (await rows()).length <= 3, await rows());
  await p.click('#exFolderMenu');
  await p.click('.exmenu [data-ex="expand"]');
  check('Expand all folders (menu)', (await rows()).length >= 4, await rows());

  // open one, then ⋯ adds
  await p.click('#explorer .row.file[data-path$="flight.tlm"]');
  check('a selected file offers Only this', await visible('#explorer .row.file.sel .tbtn[data-tip]'));
  await p.dblclick('#explorer .row.file[data-path$="flight.tlm"]');
  await until(() => document.querySelectorAll('#explorer .row.chan').length === 6);
  await p.fill('#explorer .exfilter input', 'rpm');
  const n = await paneCount();
  await p.click('#exMore');
  check('Add all N counts what the filter shows', /Add all 2 channels to pane/.test(await text('.exmenu')), await text('.exmenu'));
  await p.click('.exmenu [data-ex="new-table"]');
  check('Open them in a new table', (await paneCount()) === n + 1 && (await p.$$('#stack [data-table]')).length === 1);
  await p.fill('#explorer .exfilter input', 'deg');
  check('the filter matches units', (await p.$$('#explorer .row.chan')).length === 2, await rows());
  await p.fill('#explorer .exfilter input', 'accel');
  check('and categories', (await p.$$('#explorer .row.chan')).length === 2, await rows());
  await p.fill('#explorer .exfilter input', '');

  // categories off from ⋯
  await p.click('#exMore');
  await p.click('.exmenu [data-ex="cats"]');
  check('Categories off lists channels flat', (await p.$$('#explorer .row.cat')).length === 0 && (await settingsLS()).exCategories === false);
  await p.click('#exMore');
  await p.click('.exmenu [data-ex="cats"]');

  // Open only this
  await p.dblclick('#explorer .row.file[data-path$="hover.csv"]');
  await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2);
  await p.click('#explorer .row.file[data-path$="flight.tlm"]', { button: 'right' });
  await p.click('.filemenu [data-fm="only"]').catch(() => {});
  await wait(400);
  check('Open only this closes the other tabs', (await p.$$('#wsTabs .wstab')).length === 1 && (await text('#wsTabs .nm')) === 'flight.tlm', await text('#wsTabs'));
  await p.click('#exFolderMenu');
  await p.click('.exmenu [data-ex="remove"]');
  check('Remove the folder empties the tree but not the tabs', (await settingsLS()).folder === '' && (await p.$$('#wsTabs .wstab')).length === 1);
}, { files, launch: null });
