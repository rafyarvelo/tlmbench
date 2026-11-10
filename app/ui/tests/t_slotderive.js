// A derived channel in a tab with several files reads the other files' channels by the names the tab gives them
// (`"lat·2"`, quoted because of the dot): the recipe applies to the primary, the core is told the group, the new
// channel lands after the primary's own and the other file's channels move up, with the panes following them.
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = { [A]: {}, [B]: { channels: ['lat', 'lon'], units: ['deg', 'deg'], samples: 20000, rate: 100 } };

run(
  'slotderive',
  async ({ p, check, fake, until, text }) => {
    const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm');
    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel [data-addtab]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 2);
    // plot lat (channel 6) so its place can be followed
    await p.click('#explorer .row.chan[data-ch="6"] .box');

    // Derive… from a channel of each file
    await p.click('#explorer .row.chan[data-ch="0"] .nm');
    await p.click('#explorer .row.chan[data-ch="6"] .nm', { modifiers: ['ControlOrMeta'] });
    await p.click('#selBar [data-sel="derive"]');
    check('the dialog opens', await until(() => !!document.querySelector('#deriveDlg')));
    check('the expression names the other file\'s channel quoted', (await p.inputValue('#deriveDlg [data-r="expr"]')) === 'accel_x - "lat·2"', await p.inputValue('#deriveDlg [data-r="expr"]'));
    await p.fill('#deriveDlg [data-r="name"]', 'Offset');
    await until(() => !document.querySelector('#deriveDlg [data-ok]').disabled);
    await p.click('#deriveDlg [data-ok]');
    check('the core computes it on the primary, with the group', await until(() => window.__tlmFake.derivedApplied.length === 1 && window.__tlmFake.derivedApplied[0].file === 1 && window.__tlmFake.derivedApplied[0].group != null), await fake(() => window.__tlmFake.derivedApplied));
    check('the group is made again with the new primary', await until(() => window.__tlmFake.groupSets.length === 2));
    check('Offset follows the primary\'s channels and the other file\'s move up', await until(() => document.querySelector('#explorer .row.chan[data-ch="6"][data-derived]')) && !!(await p.$('#explorer .row.chan[data-ch="7"]')), await p.$$eval('#explorer .row.chan', (e) => e.map((x) => x.dataset.ch + ':' + x.querySelector('.nm').textContent)));
    check('the plotted lat followed its file (6 → 7), and the new channel is plotted too', (await app('panes'))[0].includes(7) && (await app('panes'))[0].includes(6), await app('panes'));
    check('slot 2 still starts after it', (await text('#explorer .row.slot[data-slot="2"] .nm')) === 'gps');
  },
  { files, launch: null },
);
