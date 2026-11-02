// Opening files (Open…, ⌘O, a drop), a file whose summaries are still being read (hatching, Measure waiting for
// exact stats), the coarse label on a cold file, and a channel's gap factor dialog.
import { run } from './lib.js';

const A = '/data/flight.tlm';
const B = '/data/bench run.csv';
const COLD = '/data/cold.tlm';

run('open', async ({ p, check, text, visible, fake, paneCount, plotAt, settingsLS, until, wait }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  check('empty state with no file', (await visible('#empty')) && (await paneCount()) === 0);

  // Open… asks the dialog; ⌘O too
  await fake((a) => (window.__tlmFake.pick = a), A);
  await p.click('#exOpen');
  check('Open… opens the picked file', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm'));
  check('empty state hidden', !(await visible('#empty')) && (await paneCount()) === 3);
  await fake((b) => (window.__tlmFake.pick = b), B);
  await p.keyboard.press('ControlOrMeta+o');
  check('⌘O opens another in a tab of its own', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'bench run.csv'));
  check('the first stays open in its tab', (await fake(() => window.__tlmFake.open.size)) === 2 && (await p.$$('#wsTabs .wstab')).length === 2);

  // a drop opens the first dropped file
  await fake((a) => window.__tlmFake.drop([a]), A);
  check('a dropped file opens', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm'));

  // a file being read: hatched, Measure waits for exact stats, then fills in
  await fake((c) => (window.__tlmFake.files[c] = { built: false }), COLD);
  await fake((c) => window.__tlmFake.drop([c]), COLD);
  check('a file being read', await until(() => window.__tlmApp.building() !== null));
  const asked = await fake(() => window.__tlmFake.precisions.length);
  await fake((c) => window.__tlmFake.build(c, 0.4), COLD);
  check('progress', await until(() => window.__tlmApp.building()?.done === 80000), await app('building'));
  check('the read part is drawn', await until(() => window.__tlmApp.drawn().some((d) => d && d.length)));
  await wait(400);
  check('refreshes while reading are exact, not coarse first', (await fake((n) => window.__tlmFake.precisions.slice(n), asked)).every((x) => x === 'exact'), await fake((n) => window.__tlmFake.precisions.slice(n), asked));
  check('no coarse label while reading', (await p.$$('.pill[data-coarse]')).length === 0);
  await p.keyboard.press('c');
  for (const fx of [0.1, 0.3]) {
    const pt = await plotAt(0, fx);
    await p.mouse.click(pt.x, pt.y);
  }
  check('Measure waits for exact stats', await until(() => /exact stats between the cursors once it's read \(40%\)/.test(document.getElementById('measure').innerText)), await text('#measure'));
  await fake((c) => window.__tlmFake.build(c, 1), COLD);
  check('built: no hatching', await until(() => window.__tlmApp.building() === null));
  check('Measure fills in its stats', await until(() => /Std dev/.test(document.getElementById('measure').innerText)), await text('#measure'));
  await p.keyboard.press('Escape');

  // a cold file: the first view is coarse and labelled, the exact one replaces it
  // closed, then opened again cold (its tab would keep the view it had)
  await p.hover(`#wsTabs .wstab[data-tip="${COLD}"]`);
  await p.click(`#wsTabs .wstab[data-tip="${COLD}"] .tx`);
  check('closing a tab closes its file', await until((c) => ![...window.__tlmFake.open.values()].some((o) => o.r.path === c), COLD));
  await fake((c) => (window.__tlmFake.files[c] = { cold: true }), COLD);
  await fake(() => {
    const seen = (window.__coarseSeen = []);
    new MutationObserver(() => seen.push(document.querySelectorAll('.pill[data-coarse]').length)).observe(document.getElementById('stack'), { subtree: true, childList: true });
  });
  await fake((c) => window.__tlmFake.drop([c]), COLD);
  check('coarse label shown first', await until(() => window.__coarseSeen.some((n) => n > 0)));
  check('then the exact view hides it', await until(() => document.querySelectorAll('.pill[data-coarse]').length === 0));

  // a coarse view the exact one replaces at once isn't labelled
  await fake(() => (window.__tlmFake.files['/data/quick.tlm'] = { cold: true, exactMs: 60 }));
  await fake(() => (window.__coarseSeen.length = 0));
  await fake(() => window.__tlmFake.drop(['/data/quick.tlm']));
  await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'quick.tlm');
  await wait(700);
  check('a quickly replaced coarse view has no label', await p.evaluate(() => !window.__coarseSeen.some((n) => n > 0)));
  await fake(() => window.__tlmFake.drop(['/data/cold.tlm']));
  await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'cold.tlm');

  // gap factor: right-click a chip
  await p.click('#stack .chip[data-ch="1"]', { button: 'right' });
  check('the channel menu', /accel_y.*Gap factor….*4×/s.test(await text('.menu')), await text('.menu'));
  await p.click('.menu [data-gap]');
  check('the dialog', /Gap factor/.test(await text('.dlg')) && (await p.inputValue('.dlg input')) === '4');
  await p.fill('.dlg input', '1');
  await p.click('.dlg [data-apply]');
  check('below 2 is refused', await visible('.dlg [data-err]'));
  await p.fill('.dlg input', '8');
  const sent = await fake(() => window.__tlmFake.calls.filter((c) => c === 'view').length);
  await p.keyboard.press('Enter');
  check('applied and closed', !(await visible('.dlg')) && (await settingsLS()).gapFactors?.[COLD]?.[1] === 8, await settingsLS());
  check('the panes asked again', await until((n) => window.__tlmFake.calls.filter((c) => c === 'view').length > n, sent));
  await p.click('#stack .chip[data-ch="1"]', { button: 'right' });
  check('the menu shows the new factor', /8×/.test(await text('.menu')), await text('.menu'));
  await p.keyboard.press('Escape');
  check('Esc closes the menu', !(await visible('.menu')));
  await p.click('#chans .chan[data-ch="1"]', { button: 'right' });
  await p.click('.menu [data-gap]');
  await p.click('.dlg [data-reset]');
  check('Reset to 4 forgets it', !(await settingsLS()).gapFactors?.[COLD], await settingsLS());
  await wait(100);
}, { files: { [A]: {}, [B]: { channels: ['volts', 'amps', 'temp'], units: ['V', 'A', '°C'] } }, launch: null });
