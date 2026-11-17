// Quiet interface reaches chips, file numbers and grips (D-ui-10, as the prototype's quiet rules): a chip's empty value
// readout takes no room until you point at a plot; a file's slot number in the folder listing shows on hover or
// selection; a table's resize grip shows when hovered. With the setting off, all three show as before.
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = { [A]: {}, [B]: { samples: 20000, rate: 100 } };

run(
  'quiet',
  async ({ p, check, fake, until }) => {
    const shown = (sel) => p.evaluate((sel) => [...document.querySelectorAll(sel)].map((e) => getComputedStyle(e).display !== 'none' && getComputedStyle(e).visibility !== 'hidden'), sel);
    const grip = () => p.evaluate(() => {
      const g = document.querySelector('.tgrip');
      return g ? getComputedStyle(g, '::before').opacity : null;
    });
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.evaluate(() => window.__tlmSettings.set('fileOpenNew', 'table'));
    await p.click('#exFolder');
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm');
    await p.click(`#explorer .row.file[data-path="${B}"]`);
    await p.click('#explorer .row.file.sel', { button: 'right' });
    await p.click('.filemenu [data-fm="add"]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length === 2);
    await p.evaluate(() => window.__tlmApp.setPanes([{ channels: [0, 1] }]));
    await until(() => document.querySelectorAll('#stack .chip').length === 2);
    await p.mouse.move(5, 5);

    // off: everything shows
    check('quiet is off by default', !(await p.evaluate(() => window.__tlmSettings.get('quietUI'))));
    check('off: chip readouts keep their room', (await shown('#stack .chip .val')).every(Boolean), await shown('#stack .chip .val'));
    check('off: the folder listing shows file numbers', (await shown('#explorer .row.file [data-fslot]')).some(Boolean), await shown('#explorer .row.file [data-fslot]'));

    await p.evaluate(() => window.__tlmSettings.set('quietUI', true));
    await p.waitForTimeout(100);
    check('quiet: an empty chip readout takes no room', (await shown('#stack .chip .val')).every((x) => !x), await shown('#stack .chip .val'));
    // a value shows while the pointer is on a plot
    const t = await p.evaluate(() => window.__tlmApp.view().t0);
    await p.evaluate((t) => window.__tlmApp.hoverAt(String(BigInt(t) + 5000000000n)), t);
    check('quiet: a chip readout shows while you point at a plot', await until(() => [...document.querySelectorAll('#stack .chip .val')].some((e) => e.textContent && getComputedStyle(e).display !== 'none')));
    await p.evaluate(() => window.__tlmApp.hoverAt(null));

    const rowB = `#explorer .row.file[data-path="${B}"]`;
    await p.click(`#explorer .row.file[data-path="${A}"] .nm`).catch(() => {});
    await p.mouse.move(5, 5);
    const numB = () => p.evaluate((sel) => { const e = document.querySelector(sel + ' [data-fslot]'); return e ? getComputedStyle(e).visibility : null; }, rowB);
    check('quiet: a file number waits in the folder listing', (await numB()) === 'hidden', await numB());
    await p.hover(rowB);
    check('quiet: hovering the file shows it', (await numB()) === 'visible', await numB());

    // grips: a table pane
    await p.evaluate(() => window.__tlmApp.setPanes([{ channels: [0, 1], mode: 'table' }]));
    const g = await until(() => !!document.querySelector('.tgrip'), 3000);
    if (g) {
      await p.mouse.move(5, 5);
      check('quiet: a table grip waits', (await grip()) === '0', await grip());
      await p.hover('.tgrip');
      check('quiet: hovering it shows it', (await grip()) === '1', await grip());
      await p.evaluate(() => window.__tlmSettings.set('quietUI', false));
      await p.mouse.move(5, 5);
      check('off: the grip shows', (await grip()) === '1', await grip());
    } else check('a table pane has a grip', false);
  },
  { files, launch: null },
);
