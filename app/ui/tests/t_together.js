// Open together draws every file (owner, Nov 15; ledger D-ui-46): the first layout has a pane per channel the files share, each
// holding every file's copy, then each file's starter charts for the channels only it has. A tab that was already open keeps
// the panes it has.
import { run } from './lib.js';

const DIR = '/data/runs';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/gps.tlm`;
const files = {
  [A]: {},
  [B]: { channels: ['lat', 'lon', 'accel_x'], units: ['deg', 'deg', 'g'], samples: 20000, rate: 100 },
  [`${DIR}/bench.tlm`]: { channels: ['volts', 'amps'], units: ['V', 'A'], samples: 5000, rate: 50 },
};

run(
  'together',
  async ({ p, check, fake, until, text }) => {
    const panes = () => p.evaluate(() => window.__tlmApp.panes());
    await fake((d) => (window.__tlmFake.pickFolder = d), DIR);
    await p.click('#exFolder');
    await until(() => document.querySelectorAll('#explorer .row.file').length >= 3);
    const sel = async (a, b) => {
      await p.click(`#explorer .row.file[data-path="${a}"]`);
      await p.click(`#explorer .row.file[data-path="${b}"]`, { modifiers: ['ControlOrMeta'] });
    };

    // two files that share a channel: a pane for it with both traces, then each file's own channels
    await sel(A, B);
    await p.click('[data-fsel="together"]');
    check('Open together: one tab, two slots', await until(() => document.querySelectorAll('#explorer .row.slot').length === 2));
    const tab = () => p.evaluate(() => { const t = document.querySelector('#wsTabs .wstab[aria-selected=true]'); return { name: t?.querySelector('.nm')?.textContent, tip: t?.dataset.tip }; });
    check('the tab is named for both files, its tooltip lists each (owner, Nov 16)', await until(() => document.querySelector('#wsTabs .wstab[aria-selected=true] .nm')?.textContent === 'flight + gps') && (await tab()).tip === `${A}\n${B}`, await tab());
    check('the shared channel first, each file\'s copy in the one pane; then each file\'s own channels as starter charts', await until(() => window.__tlmApp.panes().length === 6) && JSON.stringify(await panes()) === '[[0,8],[1,2],[3,4],[5],[6],[7]]', await panes());

    // a table over both files' copies: the slower file holds its last sample, dimmed where unchanged (S4.2-09)
    await p.click('#stack [data-pane="0"] [data-vmenu]');
    await p.click('.menu [data-vm="table"]');
    check('the other file\'s cells that repeat the one above are dimmed', await until(() => document.querySelectorAll('#stack .trw > span.held').length > 0), await p.$$eval('#stack .tth', (h) => h.map((x) => x.dataset.col)));
    check('the first file\'s cells are not', !(await p.$('#stack .trw > span.held:nth-child(3)')));
    check('and the status line says how they show', /held between their samples, dimmed when unchanged/.test(await text('#stack [data-tstat]')), await text('#stack [data-tstat]'));
    await p.click('#stack [data-pane="0"] [data-vmenu]');
    await p.click('.menu [data-vm="chart"]');

    // files with nothing in common: each file's starter charts
    await p.click(`#explorer .row.file[data-path="${DIR}/bench.tlm"]`);
    await p.click(`#explorer .row.file[data-path="${B}"]`, { modifiers: ['ControlOrMeta'] });
    await p.click('[data-fsel="together"]');
    check('a tab opened with the first file draws both: bench\'s two charts, then gps\'s three', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((n) => /^bench/.test(n.textContent ?? ''))) && (await until(() => window.__tlmApp.panes().length === 5)) && JSON.stringify(await panes()) === '[[0],[1],[2],[3],[4]]', await panes());

    // a tab that was open already keeps its own panes
    await p.click(`#wsTabs .wstab:has-text("flight")`);
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent?.startsWith('flight'));
    const before = JSON.stringify(await panes());
    await sel(A, `${DIR}/bench.tlm`);
    await p.click('[data-fsel="together"]');
    await until(() => document.querySelectorAll('#explorer .row.slot').length >= 2);
    check('three files: the first and how many more', await until(() => document.querySelector('#wsTabs .wstab[aria-selected=true] .nm')?.textContent === 'flight + 2 more'), await tab());
    check('Open together on a file that is open already leaves its panes alone', JSON.stringify((await panes()).slice(0, JSON.parse(before).length)) === before, await panes());
  },
  { files, launch: null },
);
