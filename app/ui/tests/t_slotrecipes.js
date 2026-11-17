// Recipes and adjustments in a tab of several files (D-derive-03, S5.6-07, S5.6-12 adjustments): an automatic recipe
// ("Any file that has these channels") made while two files share a tab gives each file its own copy (`Twice`,
// `Twice·2`), each from its own channels; a recipe that reads the other file on purpose stays one channel, and names
// that file's channel by its short name (`"gps accel_x"`) as well as by slot (`"accel_x·2"`); a scale reopens every
// file of the tab, not only the primary.
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = { [A]: {}, [B]: { samples: 20000, rate: 100 } };

run(
  'slotrecipes',
  async ({ p, check, fake, until }) => {
    const rows = () => p.$$eval('#explorer .row.chan', (e) => e.map((x) => x.querySelector('.nm').textContent + (x.hasAttribute('data-derived') ? 'ƒ' : '')));
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm');
    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 2);

    // an automatic recipe from the primary's channel
    await p.click('#explorer .row.chan[data-ch="0"] .nm');
    await p.click('#selBar [data-sel="derive"]');
    await until(() => !!document.querySelector('#deriveDlg'));
    await p.fill('#deriveDlg [data-r="expr"]', 'accel_x * 2');
    await p.fill('#deriveDlg [data-r="name"]', 'Twice');
    await p.check('#deriveDlg input[name="rsc"][value="auto"]');
    await until(() => !document.querySelector('#deriveDlg [data-ok]').disabled);
    await p.click('#deriveDlg [data-ok]');
    check('each file gets its own copy', await until(() => [...document.querySelectorAll('#explorer .row.chan[data-derived] .nm')].map((x) => x.textContent).join() === 'Twice,Twice'), await rows());
    const applied = await fake(() => window.__tlmFake.derivedApplied);
    check('the other file is applied alone, then the primary with the group', applied.length >= 2 && applied.at(-2).group == null && applied.at(-1).group != null, applied);
    const names = await fake(() => [...window.__tlmFake.open.values()].filter((o) => o.group).at(-1).r.chans.map((c) => c.name));
    check('the tab names them Twice and Twice·2', names.includes('Twice') && names.includes('Twice·2'), names);

    // a recipe that reads the other file by its short name: one channel, on the primary
    await p.click('#explorer .row.chan[data-ch="0"] .nm');
    await p.click('#selBar [data-sel="derive"]');
    await until(() => !!document.querySelector('#deriveDlg'));
    await p.fill('#deriveDlg [data-r="expr"]', 'accel_x - "gps accel_x"');
    await p.fill('#deriveDlg [data-r="name"]', 'Gap');
    check('the dialog takes the short name', await until(() => !document.querySelector('#deriveDlg [data-ok]').disabled), await p.textContent('#deriveDlg'));
    const asked = await fake(() => window.__tlmFake.checks.at(-1));
    check('the check was the expression', asked === 'accel_x - "gps accel_x"', asked);
    await p.click('#deriveDlg [data-ok]');
    check('Gap is one channel, on the primary', await until(() => [...document.querySelectorAll('#explorer .row.chan[data-derived] .nm')].map((x) => x.textContent).join() === 'Twice,Gap,Twice'), await rows());
    const last = await fake(() => window.__tlmFake.derivedApplied.at(-1));
    check('the core is told each file\'s short name', last.prefixes?.join() === 'flight,gps', last);

    // a scale reopens both files
    await fake(() => (window.__tlmFake.adjustFiles.length = 0));
    await p.evaluate(() => window.__tlmSettings.set('chdisp', { accel_x: { scale: 2 } }));
    check('a scale reopens every file of the tab', await until(() => new Set(window.__tlmFake.adjustFiles).size === 2), await fake(() => window.__tlmFake.adjustFiles));
    check('no note refuses it', !/close the others/.test(await p.textContent('body')));
    check('the channels are all still there', await until(() => document.querySelectorAll('#explorer .row.chan[data-derived]').length === 3), await rows());
  },
  { files, launch: null },
);
