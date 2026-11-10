// The + menu says what a layout needs ("needs file 2" when it names `lat·2`), and opening it asks which recording
// fills the empty slot (the slot picker) before it opens, on a tab that now has both files.
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = { [A]: {}, [B]: { channels: ['lat', 'lon'], units: ['deg', 'deg'], samples: 20000, rate: 100 } };
const settings = { workspaces: [{ schema: 1, name: 'Track', panes: [{ channels: ['accel_x', 'lat·2'] }] }, { schema: 1, name: 'Plain', panes: [{ channels: ['accel_x'] }] }] };

run(
  'slotpick',
  async ({ p, check, text, fake, until }) => {
    const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm');
    await p.click('#wsTabNew');
    check('a layout naming `lat·2` says it needs file 2', (await text('.addmenu [data-wsopen="Track"] .hint')) === 'needs file 2', await text('.addmenu [data-wsopen="Track"] .hint'));
    check('one that names only file 1 does not', !/needs/.test(await text('.addmenu [data-wsopen="Plain"] .hint')));
    await p.click('.addmenu [data-wsopen="Track"]');
    check('opening it asks for the file', !!(await until(() => !!document.querySelector('#slotDlg'))));
    check('the picker lists the folder\'s other files, with an empty choice', (await p.$$eval('#slotDlg [data-slotpick="2"] option', (o) => o.map((x) => x.textContent))).join() === 'Leave it empty,gps.tlm', await p.$$eval('#slotDlg [data-slotpick="2"] option', (o) => o.map((x) => x.textContent)));
    await p.selectOption('#slotDlg [data-slotpick="2"]', B);
    await p.click('#slotDlg [data-go]');
    check('the file joins the tab', await until(() => window.__tlmFake.groupSets.length === 1));
    check('and the layout opens with its channel from file 2', (await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'Track')) && JSON.stringify(await app('panes')) === '[[0,6]]', await app('panes'));

    // the tab has both files now: nothing is needed, and the layout shows as open
    await p.click('#wsTabNew');
    check('with both files in the tab the layout shows as open', (await text('.addmenu [data-wsopen="Track"] .hint')) === 'open in a tab');
    check('and another that names file 2 would need nothing', !(await p.$('.addmenu [data-needs]')));
    await p.keyboard.press('Escape');
  },
  { files, launch: null, settings },
);
