// Several files in one tab: Add to tab puts a second file on the timeline as slot 2 (its channels `name·2`, read as
// `gps · lat`), the Alignment section (methods, offset, nudge, a preview to keep or undo, lock), Make primary,
// Channel prefix and closing a file. The fake core stands in for the core's Group and time maps.
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = {
  [A]: {},
  [B]: { channels: ['lat', 'lon', 'accel_x'], units: ['deg', 'deg', 'g'], samples: 20000, rate: 100 },
  [`${DIR}/bench.tlm`]: { channels: ['volts', 'amps'], units: ['V', 'A'], samples: 5000, rate: 50 },
  [`${DIR}/scope.tlm`]: { channels: ['ch1'], units: ['V'], samples: 5000, rate: 50, relative: true },
};

run(
  'slots',
  async ({ p, check, text, visible, fake, until, plotAt }) => {
    const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
    const calls = (k) => fake((k) => window.__tlmFake[k], k);
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    check('flight opens in its tab', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm'));
    check('one file: no slot rows', (await p.$$('#explorer .row.slot')).length === 0);

    // Add to this tab
    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
    check('Add to tab: two slots', await until(() => document.querySelectorAll('#explorer .row.slot').length === 2));
    check('the core was asked to group both', JSON.stringify(await calls('groupSets')) === '[[1,2]]', await calls('groupSets'));
    check('the Open list names both files', (await p.$$eval('#openFiles .ofile .ofnm', (e) => e.map((x) => x.textContent))).join() === 'flight.tlm,gps.tlm' && /Open · 2/.test(await p.$eval('.exoh', (e) => e.textContent)), await p.$$eval('#openFiles .ofile .ofnm', (e) => e.map((x) => x.textContent)));
    check('slot 1 is the primary', (await text('#explorer .row.slot[data-slot="1"] [data-primary]')) === 'primary');
    check('files logged on one clock: Same clock', /same clock/.test(await text('#explorer .row.slot[data-slot="2"] [data-asum]')));
    check('slot 2 channels sit under it', (await p.$$eval('#explorer .row.chan .nm', (r) => r.map((x) => x.textContent))).slice(-3).join() === 'lat,lon,accel_x');
    check('still one tab', (await p.$$('#wsTabs .wstab')).length === 1);

    // a channel of slot 2 plots, named by its file
    await p.click('#explorer .row.chan[data-ch="6"] .box');
    check('slot 2 channel reads `gps · lat`', await p.evaluate(() => document.body.innerText.includes('gps · lat')));
    check('and is drawn', await until(() => window.__tlmApp.drawn().some((d) => d && d.includes(6))));

    // Alignment: Offset, nudge, a method that fits with a preview
    await p.click('#explorer .row.slot[data-slot="2"]');
    check('the Alignment section opens', await visible('.align[data-align="2"]'));
    await p.selectOption('[data-falign="2"]', 'manual');
    await p.fill('[data-foff="2"]', '2.5');
    await p.press('[data-foff="2"]', 'Tab');
    const lastAlign = async () => (await calls('aligns')).at(-1);
    check('Offset asks for those seconds', await until(() => window.__tlmFake.aligns.at(-1)?.method.seconds === 2.5), await lastAlign());
    check('the offset reads back', await until(() => /\+2\.5/.test(document.querySelector('[data-offset]')?.textContent ?? '')), await text('[data-offset]'));
    const before = (await calls('groupMaps')).at(-1).knots;
    const n0 = (await calls('groupMaps')).length;
    await p.click('[data-nudge="+"]');
    await until((n) => window.__tlmFake.groupMaps.length > n, n0);
    const after = (await calls('groupMaps')).at(-1).knots;
    check('nudge moves the map by the step (10 ms)', BigInt(after[0][1]) - BigInt(before[0][1]) === 10_000_000n, [before, after]);

    const c = await plotAt(0);
    await p.mouse.move(c.x, c.y);
    check('the pointer readout gives both clocks over the shifted file\'s channel', await until(() => /gps own clock/.test(document.getElementById('hoverTxt')?.textContent ?? '')), await text('#hoverTxt'));
    await p.mouse.move(5, 5);
    await fake(() => (window.__tlmFake.alignResults.xcorr = { knots: [['0', '-3330000000']], residuals: [['1000000000', 0.4], ['5000000000', -0.3], ['9000000000', 0.2]], quality: 0.97, note: 'Matched on accel_x: −3.33 s, r = 0.97', channel: 'accel_x' }));
    await p.selectOption('[data-falign="2"]', 'xcorr');
    check('Cross-correlation names the channel, offset and r', await until(() => /Matched on accel_x: −3\.33 s, r = 0\.97/.test(document.querySelector('.align .anote')?.textContent ?? '')));
    check('it shows the fit', /97%/.test(await text('[data-quality]')) && (await p.$$('.fit circle')).length === 3);
    check('and a result to keep or undo', await visible('[data-preview]'));
    await p.click('[data-undo]');
    check('Undo puts the map back', (await until(() => !document.querySelector('[data-preview]'))) && JSON.stringify((await calls('groupMaps')).at(-1).knots) === JSON.stringify(after), (await calls('groupMaps')).at(-1));
    await p.selectOption('[data-falign="2"]', 'xcorr');
    await until(() => document.querySelector('[data-keep]'));
    await p.click('[data-keep]');
    check('Keep leaves it applied', !(await visible('[data-preview]')) && /−3\.33/.test(await text('[data-offset]')), await text('[data-offset]'));

    // a method that can't match says why
    await fake(() => (window.__tlmFake.alignResults.timesource = new Error('Both files need a channel named gps_tow')));
    await p.selectOption('[data-falign="2"]', 'timesrc');
    check('a method that fails says why, the map stays', (await until(() => /Both files need a channel named gps_tow/.test(document.querySelector('.align .anote')?.textContent ?? ''))) && /−3\.33/.test(await text('[data-offset]')));

    // lock
    await p.check('[data-lock="2"]');
    check('Lock disables the controls', (await p.isDisabled('[data-falign="2"]')) && (await p.isDisabled('[data-nudge="+"]')));
    await p.uncheck('[data-lock="2"]');

    // Channel prefix
    await p.fill('[data-prefix="2"]', 'GPS');
    await p.press('[data-prefix="2"]', 'Tab');
    check('Channel prefix renames the channels', (await until(() => document.body.innerText.includes('GPS · lat'))) && (await text('#explorer .row.slot[data-slot="2"] .nm')) === 'GPS');
    check('the prefix is saved', (await p.evaluate(() => JSON.parse(localStorage.getItem('tlmbench.settings') || '{}').filePrefix))?.[B] === 'GPS');

    // Make primary: the files swap slots and the plotted channel follows its file
    await p.hover('#explorer .row.slot[data-slot="2"]');
    await p.click('#explorer .row.slot[data-slot="2"] [data-makeprimary]');
    check('Make primary regroups with the files swapped', (await until(() => window.__tlmFake.groupSets.length === 2)) && JSON.stringify((await calls('groupSets')).at(-1)) === '[2,1]', await calls('groupSets'));
    check('gps is slot 1 now', await until(() => document.querySelector('#explorer .row.slot[data-slot="1"] .nm')?.textContent === 'GPS'));
    check('the pane keeps its channel (lat is channel 0 now)', (await app('panes')).flat().includes(0), await app('panes'));
    check('the tab is named for its primary', (await text('#wsTabs [aria-selected=true] .nm')) === 'gps.tlm');

    // close a file of the tab
    await p.hover('#explorer .row.slot[data-slot="2"]');
    await p.click('#explorer .row.slot[data-slot="2"] [data-closeslot]');
    check('Close this file leaves one', await until(() => document.querySelectorAll('#explorer .row.slot').length === 0));
    check('and the channels of that file leave the panes', !(await app('panes')).flat().some((c) => c >= 3), await app('panes'));

    // Compare runs: two picked files, starts aligned, a pane per channel both have
    const sel = async (a, b) => {
      await p.click(`#explorer .row.file[data-path="${a}"]`);
      await p.click(`#explorer .row.file[data-path="${b}"]`, { modifiers: ['ControlOrMeta'] });
    };
    await sel(A, B);
    check('two picked files offer Compare and Open together', (await visible('[data-fsel="compare"]')) && (await visible('[data-fsel="together"]')));
    await p.click('[data-fsel="compare"]');
    check('the compare tab is named for both', await until(() => /flight vs gps/i.test(document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent ?? '')), await text('#wsTabs [aria-selected=true] .nm'));
    check('starts are aligned', await until(() => window.__tlmFake.aligns.at(-1)?.method.method === 'starts'));
    check('one pane pairs the shared channel of both files', (await until(() => window.__tlmApp.panes().length === 1)) && JSON.stringify(await app('panes')) === '[[0,8]]', await app('panes'));

    // Open together: the first is the primary, the others follow on its timeline
    const C = `${DIR}/bench.tlm`;
    await sel(C, `${DIR}/scope.tlm`);
    await p.click('[data-fsel="together"]');
    check('Open together: one tab, two slots', await until(() => /^bench/.test(document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent ?? '') && document.querySelectorAll('#explorer .row.slot').length === 2));
    check('separate runs line up at their starts', /starts aligned/.test(await text('#explorer .row.slot[data-slot="2"] [data-asum]')) || /same clock/.test(await text('#explorer .row.slot[data-slot="2"] [data-asum]')));
  },
  { files, launch: null },
);
