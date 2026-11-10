// A shared view carries all its files: slot 1 and the files added to the tab, each with its alignment (method, what it
// found, the knots). Opening it brings the files back on one timeline exactly as they were left, nothing measured
// again, and the time range.
import { run } from './lib.js';

const ROOT = '/rec';
const A = '/rec/flights/a.tlm';
const B = '/rec/flights/b.tlm';
const files = { [A]: {}, [B]: { channels: ['lat', 'lon'], units: ['deg', 'deg'], samples: 20000, rate: 100 } };

run(
  'sharedgroup',
  async ({ p, check, fake, until, text }) => {
    const front = () => p.$eval('#wsTabs .wstab[aria-selected=true]', (t) => t.dataset.tab);
    const tabMenu = async () => p.click(`#wsTabs .wstab[data-tab="${await front()}"] .tm`);
    const view = (f) => p.evaluate((f) => window.__tlmFake.views['/rec'][f], f);
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'a.tlm');
    // the second file joins, aligned by hand
    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 2);
    await p.click('#explorer .row.slot[data-slot="2"]');
    await p.selectOption('[data-falign="2"]', 'manual');
    await p.fill('[data-foff="2"]', '1.5');
    await p.press('[data-foff="2"]', 'Tab');
    await until(() => window.__tlmFake.aligns.at(-1)?.method.seconds === 1.5);

    await tabMenu();
    await p.click('.tabmenu [data-ws="share"]');
    await p.fill('#wsName', 'Pair');
    await p.press('#wsName', 'Enter');
    await until(() => window.__tlmFake.views['/rec']?.['pair.tlmview.json']);
    const v = await view('pair.tlmview.json');
    check('both files, by path in the folder', v.files.length === 2 && v.files[0].path === 'flights/a.tlm' && v.files[0].slot === 1 && v.files[1].path === 'flights/b.tlm' && v.files[1].slot === 2, v.files);
    const al = v.files[1].align;
    check('the second with its alignment: method, seconds, knots', al.method === 'manual' && al.seconds === 1.5 && al.knots.length === 1 && al.locked === false, al);
    check('the primary has none', v.files[0].align === undefined);

    // close everything, then open the shared view
    await p.click('#exFolder');
    await p.click('.exmenu [data-ex="closeall"]');
    await until(() => document.querySelectorAll('#wsTabs .wstab').length === 0);
    await fake(() => (window.__tlmFake.aligns.length = 0));
    await p.click('#wsTabNew');
    await until(() => document.querySelectorAll('.addmenu [data-shopen]').length >= 1);
    await p.click('.addmenu [data-shopen="pair.tlmview.json"]');
    check('opening it brings both files back on one timeline', await until(() => document.querySelectorAll('#explorer .row.slot').length === 2), await text('#explorer').catch(() => ''));
    await p.waitForTimeout(300);
    if (!(await p.$('[data-foff="2"]'))) await p.click('#explorer .row.slot[data-slot="2"]');
    await p.waitForSelector('[data-foff="2"]');
    check('with the alignment it was shared with, nothing measured again', (await p.inputValue('[data-falign="2"]')) === 'manual' && Math.abs(+(await p.inputValue('[data-foff="2"]')) - 1.5) < 0.01 && (await fake(() => window.__tlmFake.aligns.length)) === 0);
  },
  { files, launch: null, settings: { folder: ROOT, exExpanded: ['/rec/flights'] } },
);
