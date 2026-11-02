// One file in the pane stack: panes and the channel list, navigation (wheel, box zoom, Fit, Back), hover
// readouts, cursors and Measure, and the settings that come back after a reload.
import { run } from './lib.js';

run('view', async ({ p, check, text, panes, paneCount, plotAt, settingsLS, until, wait }) => {
  const app = (f, a) => p.evaluate(([f, a]) => window.__tlmApp[f](a), [f, a]);
  check('opens the launch file', (await text('#file')) === 'flight.tlm', await text('#file'));
  check('six channels in three panes', (await paneCount()) === 3 && /accel_x.*accel_y/.test((await panes())[0]), await panes());
  check('every pane drawn', await until(() => window.__tlmApp.drawn().every((d) => d && d.length === 2)), await app('drawn'));
  check('range shows the whole file', /→/.test(await text('#range')) && /3:20|200/.test(await text('#range')), await text('#range'));
  check('overview of pane 1', /Overview of pane 1/.test(await text('.ovhead')), await text('.ovhead'));

  // the channel list follows the selected pane; a click adds a channel to it
  const sel = await plotAt(2, 0.9);
  await p.mouse.click(sel.x, sel.y);
  check('selecting a pane', (await app('sel')) === 2 && /Overview of pane 3/.test(await text('.ovhead')), await app('sel'));
  check('its channels are ticked', (await p.$$eval('#chans .chan.on', (e) => e.map((x) => x.textContent))).join() === 'motor_rpmrpm,motor_rpm_targetrpm');
  await p.click('#chans .chan[data-ch="0"]');
  check('a channel added to the pane', JSON.stringify((await app('panes'))[2]) === '[4,5,0]', await app('panes'));
  await p.click('#chans .chan[data-ch="0"]');
  await p.click('#addPane');
  check('+ Add pane', (await paneCount()) === 4 && (await app('sel')) === 3, await paneCount());
  await p.locator('#stack section.pane >> nth=3 >> [data-close]').dispatchEvent('pointerdown');
  check('closing a pane', (await paneCount()) === 3, await paneCount());

  // wheel zooms around the pointer; Back and Fit
  const full = await app('view');
  const c = await plotAt(0);
  await p.mouse.move(c.x, c.y);
  await p.mouse.wheel(0, -300);
  await wait(200);
  const z = await app('view');
  const span = (v) => BigInt(v.t1) - BigInt(v.t0);
  check('wheel zooms in', span(z) < span(full), [z, full]);
  // box zoom: drag across pane 1
  const a = await plotAt(0, 0.25);
  const b = await plotAt(0, 0.5);
  await p.mouse.move(a.x, a.y);
  await p.mouse.down();
  await p.mouse.move(b.x, b.y, { steps: 4 });
  await p.mouse.up();
  await wait(500);
  const boxed = await app('view');
  check('box zoom narrows the view', span(boxed) < span(z) / 2n, [boxed, z]);
  check('Back is enabled', !(await p.isDisabled('#back')));
  await p.click('#back');
  await wait(100);
  check('Back returns', span(await app('view')) === span(z), await app('view'));
  await p.click('#fit');
  await wait(500);
  check('Fit shows the whole file', span(await app('view')) === span(full), await app('view'));

  // hover: a time line and every chip's value
  const h = await plotAt(1, 0.4);
  await p.mouse.move(h.x, h.y);
  check('hover reads every plotted channel', await until(() => [...document.querySelectorAll('#stack [data-val]')].every((e) => e.textContent.trim() !== '')), await text('#stack'));
  check('pointer time in the bar', /^pointer /.test(await text('#pointer')), await text('#pointer'));
  await p.mouse.move(5, 5);
  check('hover clears on leave', await until(() => window.__tlmApp.hoverT() === null));

  // cursors: C, then two clicks; Measure opens with Δt and stats
  await p.keyboard.press('c');
  check('C picks Cursors mode', (await app('mode')) === 'cursor');
  const ca = await plotAt(0, 0.3);
  const cb = await plotAt(0, 0.6);
  await p.mouse.click(ca.x, ca.y);
  check('one cursor keeps Measure closed', (await app('cursors')).A && !(await p.isVisible('#measure')));
  await p.mouse.click(cb.x, cb.y);
  check('both cursors open Measure', await until(() => !document.getElementById('measure').hidden), await app('cursors'));
  check('Measure shows Δt and 1/Δt', await until(() => /Δt.*1\/Δt/.test(document.getElementById('measure').innerText.replace(/\s+/g, ' '))), await text('#measure'));
  check('exact stats per plotted channel', await until(() => document.querySelectorAll('#measure .mchan').length === 6 && /Std dev/.test(document.getElementById('measure').innerText)), await text('#measure'));
  // drag cursor B
  const before = (await app('cursors')).B;
  await p.mouse.move(cb.x, cb.y);
  await p.mouse.down();
  await p.mouse.move(cb.x + 80, cb.y, { steps: 4 });
  await p.mouse.up();
  check('dragging a cursor moves it', (await app('cursors')).B !== before && BigInt((await app('cursors')).B) > BigInt(before), [before, await app('cursors')]);
  // A over a plot places cursor A at the pointer
  const pa = await plotAt(2, 0.1);
  await p.mouse.move(pa.x, pa.y);
  await until(() => window.__tlmApp.hoverT() !== null);
  await p.keyboard.press('a');
  check('A places cursor A at the pointer', (await app('cursors')).A === (await app('hoverT')), [await app('cursors'), await app('hoverT')]);
  await p.keyboard.press('Escape');
  check('Esc clears the cursors and closes Measure', (await app('cursors')).A === null && (await until(() => document.getElementById('measure').hidden)));

  // settings are saved and come back
  await p.keyboard.press('p');
  const s = await settingsLS();
  check('mode and layout saved', s.mode === 'pan' && s.layouts['/data/flight.tlm']?.length === 3, s);
  await p.reload();
  await wait(800);
  check('they come back after a reload', (await app('mode')) === 'pan' && (await paneCount()) === 3, await app('mode'));
});
