// Explorer follows its folder (design UI, D-ui-02; sheet "Ideas", v1): opening a folder asks the core to watch it; a
// folder-changed event lists it again, so a new file shows without asking; the folder menu's Refresh lists it by hand
// (a network share may send no events).
import { run } from './lib.js';

const DIR = '/rec';
run('watch', async ({ p, check, fake, until }) => {
  const files = () => p.$$eval('#explorer .row.file', (r) => r.map((x) => x.dataset.path));
  await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
  await p.click('#exFolder');
  check('the folder is listed', await until(() => document.querySelectorAll('#explorer .row.file').length === 1), await files());
  check('and the core watches it', await until((d) => window.__tlmFake.watched === d, DIR));

  await fake((d) => { window.__tlmFake.files[`${d}/new run.csv`] = {}; window.__tlmFake.folderChanged(); }, DIR);
  check('a file that appears shows without asking', await until(() => document.querySelectorAll('#explorer .row.file').length === 2), await files());

  await fake((d) => (window.__tlmFake.files[`${d}/quiet.tlm`] = {}), DIR);
  await p.click('#exFolder');
  check('the folder menu has Refresh', await until(() => !!document.querySelector('.exmenu [data-ex="refresh"]')));
  check('its note is a space apart and muted (sheet, Nov 16)', await p.evaluate(() => { const i = document.querySelector('.exmenu [data-ex="refresh"] .nm'); const n = i?.querySelector('small'); return /^Refresh New files/.test(i?.textContent ?? '') && n && getComputedStyle(n).fontSize === '11px'; }));
  await p.click('.exmenu [data-ex="refresh"]');
  check('Refresh lists the folder again (a share that sent nothing)', await until(() => document.querySelectorAll('#explorer .row.file').length === 3), await files());
}, { files: { '/rec/a.tlm': {} }, launch: null });
