// The filter strip under a table's header: In view / Whole file, ▲ ▼ through the matches (runs closer than 0.25 s
// are one), and Apply to charts: the core leaves the other rows out of the charts, with a chip on each chart.
import { run } from './lib.js';

const T0 = 1767225600000000000n;

run('tablefilter', async ({ p, check, text, until, settingsLS, wait, fake }) => {
  const T = '#stack [data-pane="0"] [data-table]';
  const x = (i) => Math.fround(Math.sin((i / 1000) * 0.05) + 0.3 * Math.sin((i / 1000) * 1.7));
  const N = 200000;
  let over = 0;
  const hits = [];
  for (let i = 0; i < N; i++) {
    if (x(i) <= 1.2) continue;
    over++;
    const last = hits.at(-1);
    if (last && i - last[1] <= 250) last[1] = i;
    else hits.push([i, i]);
  }

  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-vmenu]');
  await p.click('.viewmenu [data-vm="table"]');
  await until(() => !!document.querySelector('[data-table] [data-tstat]'));
  check('no strip without a filter', !(await p.$(`${T} [data-tfilt]`)));
  await p.fill(`${T} [data-filter="c0"]`, '> 1.2');
  check('the strip appears with a filter', await until(() => !!document.querySelector('[data-table] [data-tfilt]')));
  check('it counts the matches, runs closer than 0.25 s as one', await until((n) => document.querySelector('[data-table] [data-fpos]')?.textContent === n, `${hits.length.toLocaleString()} ${hits.length === 1 ? 'match' : 'matches'}`), [await text(`${T} [data-fpos]`), hits.length]);
  check('In view is on', (await p.getAttribute(`${T} [data-fscope="view"]`, 'aria-pressed')) === 'true');

  // Whole file asks about every row
  await p.click(`${T} [data-fscope="file"]`);
  check('Whole file is on', await until(() => document.querySelector('[data-table] [data-fscope="file"]')?.getAttribute('aria-pressed') === 'true'));
  check('Whole file also marks the matches on the overview', await until(() => window.__tlmApp.marks()?.overview > 0), await p.evaluate(() => window.__tlmApp.marks()));
  const whole = await fake(() => window.__tlmFake.queriesRun.at(-1));
  check('and the core looks at the whole file', whole.span.t0 === String(T0) && BigInt(whole.span.t1) > T0 + BigInt(N - 1) * 1000000n - 1n, whole.span);

  // ▼ goes to the first match: cursor A lands on it and the table scrolls there
  await p.click(`${T} [data-fstep="1"]`);
  check('▼ says which match', await until((n) => document.querySelector('[data-table] [data-fpos]')?.textContent === n, `1 of ${hits.length.toLocaleString()}`), await text(`${T} [data-fpos]`));
  check('cursor A is on the match', await until((t) => window.__tlmApp.cursors().A === t, String(T0 + BigInt(hits[0][0]) * 1000000n)), [await p.evaluate(() => window.__tlmApp.cursors().A), hits[0]]);
  await p.click(`${T} [data-fstep="-1"]`);
  check('▲ steps back, wrapping to the last', await until((n) => document.querySelector('[data-table] [data-fpos]')?.textContent === n, `${hits.length.toLocaleString()} of ${hits.length.toLocaleString()}`), await text(`${T} [data-fpos]`));

  // the matches are marked on the charts (those in the view)
  check('the charts mark the matches', await until(() => (window.__tlmApp.marks()?.hits ?? 0) > 0 && window.__tlmApp.marks().excluded === 0), await p.evaluate(() => window.__tlmApp.marks()));
  const asked = await fake(() => window.__tlmFake.marksAsked.at(-1));
  check('asked for the view and the width', asked.t0 === (await p.evaluate(() => window.__tlmApp.view().t0)) && asked.widthPx >= 200, asked);

  // Apply to charts
  check('no chip yet', !(await p.$('[data-fchip]')));
  await p.click(`${T} [data-fapply]`);
  check('the core leaves the other rows out', await until(() => window.__tlmFake.masks.size === 1));
  check('of every row in the file', (await fake(() => [...window.__tlmFake.masks.values()]))[0] === N - over, await fake(() => [...window.__tlmFake.masks.values()]));
  check('the charts mark the rows left out instead of the matches', await until(() => window.__tlmApp.marks()?.excluded > 0 && window.__tlmApp.marks().hits === 0), await p.evaluate(() => window.__tlmApp.marks()));
  check('a chip on the charts says how many', await until((n) => document.querySelector('[data-fchip]')?.textContent.includes(n), `${(N - over).toLocaleString()} rows hidden`), await text('[data-fchip]'));
  check('Apply forces Whole file', (await p.getAttribute(`${T} [data-fscope="view"]`, 'disabled')) !== null);
  check('saved with the layout', JSON.stringify((await settingsLS()).layouts['/data/flight.tlm'][0].tfilt) === '{"scope":"file","apply":true}', (await settingsLS()).layouts['/data/flight.tlm'][0].tfilt);

  // the chip's × takes it back
  await p.click('[data-fchip] button');
  check('× shows every row again', await until(() => window.__tlmFake.masks.size === 0 && !document.querySelector('[data-fchip]')));
  check('and unticks Apply', !(await p.isChecked(`${T} [data-fapply]`)));
  check('the hatching goes with it', await until(() => window.__tlmApp.marks()?.excluded === 0));

  // clearing the filter takes the strip away; turning Apply on and deleting the filter lifts the mask
  await p.click(`${T} [data-fapply]`);
  await until(() => window.__tlmFake.masks.size === 1);
  await p.click(`${T} [data-clear]`);
  check('clearing the filters lifts the mask and the strip', await until(() => window.__tlmFake.masks.size === 0 && !document.querySelector('[data-fchip]') && !document.querySelector('[data-table] [data-tfilt]')));
  check('and the marks', await until(() => window.__tlmApp.marks() === null));
});
