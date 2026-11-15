// A tab with several files (owner, Nov 15; ledger D-ui-47, D-ui-48, D-ui-49): a pane's ⋯ menu has Duplicate for file N (the
// pane's channels swapped for the same-named ones of that file, those it lacks left out and counted); a channel chip's
// right-click has From file N (switch the file it reads) and Add from every file; limits belong to a channel's own name, so
// accel_x and accel_x·2 share them and the editor says so.
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = {
  [A]: {},
  [B]: { channels: ['lat', 'lon', 'accel_x'], units: ['deg', 'deg', 'g'], samples: 20000, rate: 100 },
};

run(
  'filescopy',
  async ({ p, check, fake, until, text, settingsLS }) => {
    const panes = () => p.evaluate(() => window.__tlmApp.panes());
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelectorAll('#wsTabs .wstab').length === 1);
    check('one file: no Duplicate for…', await (async () => {
      await p.hover('#stack [data-pane="0"] .phead');
      await p.click('#stack [data-pane="0"] [data-pmenu]');
      const t = await p.$$eval('.menu .mi', (e) => e.map((x) => x.textContent ?? ''));
      await p.keyboard.press('Escape');
      return !t.some((x) => /Duplicate for/.test(x));
    })());

    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 2);
    // accel_x is channel 0 of flight and 8 of gps (`accel_x·2`); the first pane holds flight's accel_x and accel_y
    const base = JSON.stringify(await panes());

    // D-ui-47: Duplicate for file 2
    await p.hover('#stack [data-pane="0"] .phead');
    await p.click('#stack [data-pane="0"] [data-pmenu]');
    const items = await p.$$eval('.menu .mi', (e) => e.map((x) => x.textContent ?? ''));
    check('the pane menu offers Duplicate for file 2, saying a channel is left out', items.some((t) => /Duplicate for file 2 · gps.*1 left out/.test(t)), items);
    await p.click('.menu [data-pm="dupfor2"]');
    check('the copy has file 2\'s accel_x and leaves accel_y out, below the pane', await until((n) => window.__tlmApp.panes().length === n + 1, JSON.parse(base).length) && JSON.stringify((await panes())[1]) === '[8]', await panes());

    // D-ui-48: the chip's right-click
    await p.click('#stack [data-pane="0"] .chip[data-ch="0"]', { button: 'right' });
    const cm = await p.$$eval('.chanmenu .mi', (e) => e.map((x) => x.textContent ?? ''));
    check('a chip that another file has offers From file 2 and Add from every file', cm.some((t) => /From file 2/.test(t)) && cm.some((t) => /Add from every file/.test(t)) && !cm.some((t) => /From file 1/.test(t)), cm);
    await p.click('.chanmenu [data-cm="every"]');
    check('Add from every file puts file 2\'s accel_x in the pane', await until(() => (window.__tlmApp.panes()[0] ?? []).includes(8)), await panes());
    await p.click('#stack [data-pane="0"] .chip[data-ch="8"]', { button: 'right' });
    await p.click('.chanmenu [data-cm="from1"]');
    check('From file 1 on that chip takes it out where flight\'s is already there', await until(() => !(window.__tlmApp.panes()[0] ?? []).includes(8)), await panes());
    await p.click('#stack [data-pane="1"] .chip[data-ch="8"]', { button: 'right' });
    await p.click('.chanmenu [data-cm="from1"]');
    check('and switches the file the chip reads where it isn\'t', await until(() => JSON.stringify(window.__tlmApp.panes()[1]) === '[0]'), await panes());

    // D-ui-49: limits by name
    await p.click('#stack [data-pane="0"] .chip[data-ch="0"]');
    await p.click('[data-tab="limits"]');
    await until(() => !!document.querySelector('[data-lim="ah"]'));
    check('the editor says the limits apply to every file with the channel', /Applies to every file with “accel_x”/.test(await text('[data-limscope]')), await text('[data-limscope]'));
    await p.fill('[data-lim="ah"]', '1.5');
    await p.keyboard.press('Tab');
    await until(async () => true);
    const lims = (await settingsLS()).limits ?? {};
    check('the limit is saved under the channel\'s own name', lims.accel_x?.ah === 1.5 && !('accel_x·2' in lims), lims);
    await p.click('[data-close]');
    // file 2's copy shows the same limit
    await p.click('#stack [data-pane="0"] .chip[data-ch="0"]', { button: 'right' });
    await p.click('.chanmenu [data-cm="every"]');
    await until(() => (window.__tlmApp.panes()[0] ?? []).includes(8));
    await p.click('#stack [data-pane="0"] .chip[data-ch="8"]');
    await p.click('[data-tab="limits"]');
    check('file 2\'s accel_x·2 shows the same limit', await until(() => document.querySelector('[data-lim="ah"]')?.value === '1.5'), await p.$eval('[data-lim="ah"]', (e) => e.value));
  },
  { files, launch: null },
);
