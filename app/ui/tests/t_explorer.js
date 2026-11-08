// Explorer: Open folder lists its recordings (folder → file → channel), select-first rows (a click selects and
// shows Open; double-click or Enter opens), the checkbox plots a channel in the selected pane, the filter (text and
// regex) over files and channels, Expand all / Collapse all, Close all, and the folder coming back after a reload.
import { run } from './lib.js';

const DIR = '/data/flights';
const files = {
  [`${DIR}/2026-10-12/flight.tlm`]: {},
  [`${DIR}/2026-10-12/hover.csv`]: { channels: ['volts', 'amps'], units: ['V', 'A'] },
  [`${DIR}/bench/esc_bench.csv`]: { channels: ['rpm', 'temp'], units: ['rpm', '°C'] },
};

run('explorer', async ({ p, check, text, visible, fake, paneCount, until, settingsLS, wait }) => {
  const rows = () => p.$$eval('#explorer .tree .row', (r) => r.map((x) => x.querySelector('.nm').textContent));
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  check('nothing open: how to start', /Open a folder/.test(await text('#explorer')));

  await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
  await p.click('#exFolder');
  check('Open folder lists it, its folders expanded', await until(() => document.querySelectorAll('#explorer .row.file').length === 3), await rows());
  check('folders then files, by name', (await rows()).join() === '2026-10-12,flight.tlm,hover.csv,bench,esc_bench.csv', await rows());
  check('no sizes beside files', !/MB|KB/.test(await text('#explorer .row.file')));

  // select first: a click selects and offers Open; double-click opens
  await p.click('#explorer .row.file[data-path$="hover.csv"]');
  check('a click selects the file', (await p.getAttribute('#explorer .row.file[data-path$="hover.csv"]', 'aria-selected')) === 'true' && (await paneCount()) === 0);
  check('…and shows Open beside it', await visible('#explorer .row.file.sel .act'));
  await p.click('#explorer .row.file.sel .act');
  check('Open opens it', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'hover.csv'));
  check('its channels under it', (await p.$$eval('#explorer .row.chan .nm', (r) => r.map((x) => x.textContent))).join() === 'volts,amps');
  await p.dblclick('#explorer .row.file[data-path$="esc_bench.csv"]');
  check('double-click opens another', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'esc_bench.csv'));
  await p.focus('#explorer .row.file[data-path$="flight.tlm"]');
  await p.keyboard.press('Enter');
  check('Enter opens one', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm') && (await paneCount()) === 3);

  // the checkbox plots a channel in the selected pane; a click on the row only selects it
  await p.click('#explorer .row.chan[data-ch="5"] .nm');
  check('a click selects a channel row', (await p.getAttribute('#explorer .row.chan[data-ch="5"]', 'aria-selected')) === 'true' && !(await app('panes'))[0].includes(5));
  await p.click('#explorer .row.chan[data-ch="5"] .box');
  check('the checkbox plots it in the selected pane', (await app('panes'))[0].includes(5) && (await p.getAttribute('#explorer .row.chan[data-ch="5"] .box', 'aria-checked')) === 'true');
  await p.keyboard.press('Enter');
  check('Enter on a channel row toggles it', !(await app('panes'))[0].includes(5));

  // filter: names, channels, regex
  await p.fill('#explorer input[type=search]', 'esc');
  check('filter by file name', (await rows()).join() === 'bench,esc_bench.csv', await rows());
  await p.fill('#explorer input[type=search]', 'motor');
  check('filter by a channel of the open file', (await rows()).join() === '2026-10-12,flight.tlm,motor,motor_rpm,motor_rpm_target', await rows());
  await p.click('#explorer .rx');
  await p.fill('#explorer input[type=search]', '^accel_[xy]$');
  check('regex', (await p.$$eval('#explorer .row.chan .nm', (r) => r.map((x) => x.textContent))).join() === 'accel_x,accel_y');
  await p.fill('#explorer input[type=search]', '([');
  check('a bad regex is marked', (await p.getAttribute('#explorer input[type=search]', 'aria-invalid')) === 'true');
  await p.fill('#explorer input[type=search]', '');

  // collapse and expand all
  await p.click('#treeCollapse');
  check('Collapse all: the folders with open files stay open, the channels fold', (await rows()).join() === '2026-10-12,flight.tlm,hover.csv,bench,esc_bench.csv', await rows());
  await p.click('#explorer .row.dir[data-path$="bench"]');
  check('a folder row toggles', !(await rows()).includes('esc_bench.csv'), await rows());
  await p.click('#treeExpand');
  check('Expand all', (await rows()).length === 14, await rows());

  // Close all, and the folder after a reload
  await p.click('#closeAll');
  check('Close all empties the panes', (await paneCount()) === 0 && (await visible('#empty')) && (await p.locator('#wsTabs .wstab').count()) === 0);
  check('the folder is saved', (await settingsLS()).folder === DIR);
  await p.reload();
  await wait(800);
  check('it is listed again after a reload', await until(() => document.querySelectorAll('#explorer .row.file').length === 3));
}, { files, launch: null });
