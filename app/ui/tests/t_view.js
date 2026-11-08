// One file in the pane stack: panes and the channel list, navigation (wheel, box zoom, Fit, Back), hover
// readouts, cursors and Measure, and the settings that come back after a reload.
import { run } from './lib.js';

run('view', async ({ p, check, text, panes, paneCount, plotAt, settingsLS, until, wait }) => {
  const app = (f, a) => p.evaluate(([f, a]) => window.__tlmApp[f](a), [f, a]);
  check('opens the launch file', (await text('#wsTabs .wstab')) === 'flight.tlm', await text('#wsTabs'));
  check('six channels in three panes', (await paneCount()) === 3 && /accel_x.*accel_y/.test((await panes())[0]), await panes());
  check('every pane drawn', await until(() => window.__tlmApp.drawn().every((d) => d && d.length === 2)), await app('drawn'));
  check('range shows the whole file', /→/.test(await text('#rangeTxt')) && /3\.33 min/.test(await text('#spanTxt')), [await text('#rangeTxt'), await text('#spanTxt')]);
  check('overview of pane 1', /Overview of pane 1/.test(await text('.ovhead')), await text('.ovhead'));

  // the channel list follows the selected pane; a click adds a channel to it
  const sel = await plotAt(2, 0.9);
  await p.mouse.click(sel.x, sel.y);
  check('selecting a pane', (await app('sel')) === 2 && /Overview of pane 3/.test(await text('.ovhead')), await app('sel'));
  check('its channels are ticked', (await p.$$eval('#chans .chan.on .nm', (e) => e.map((x) => x.textContent))).join() === 'motor_rpm,motor_rpm_target', await p.$$eval('#chans .chan.on .nm', (e) => e.map((x) => x.textContent)));
  await p.click('#chans .chan[data-ch="0"] .box');
  check('a channel added to the pane', JSON.stringify((await app('panes'))[2]) === '[4,5,0]', await app('panes'));
  await p.click('#chans .chan[data-ch="0"] .box');
  await p.click('#addPane');
  check('+ Add pane', (await paneCount()) === 4 && (await app('sel')) === 3, await paneCount());
  await p.hover('#stack [data-pane="3"] .phead');
  await p.click('#stack [data-pane="3"] [data-pmenu]');
  await p.locator('.panemenu .mi', { hasText: /^Close pane$/ }).click();
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
  check('Back is enabled', !(await p.isDisabled('#b-back')));
  await p.click('#b-back');
  await wait(100);
  check('Back returns', span(await app('view')) === span(z), await app('view'));
  await p.click('#b-fit');
  for (let k = 0; k < 30 && span(await app('view')) !== span(full); k++) await wait(100);
  check('Fit shows the whole file', span(await app('view')) === span(full), await app('view'));
  // the view stays inside the recording: zooming out stops at all of it, a pan stops at either end
  await p.mouse.move(c.x, c.y);
  await p.mouse.wheel(0, 600);
  await wait(200);
  check('zooming out stops at the whole file', (await app('view')).t0 === full.t0 && (await app('view')).t1 === full.t1, [await app('view'), full]);
  await p.mouse.wheel(0, -300);
  await wait(200);
  for (let k = 0; k < 8; k++) await p.mouse.wheel(4000, 0);
  await wait(200);
  check('a trackpad pan stops at the end', (await app('view')).t1 === full.t1 && span(await app('view')) === span(z), [await app('view'), full]);
  await p.keyboard.press('p');
  const l = await plotAt(0, 0.1);
  const r = await plotAt(0, 0.9);
  for (let k = 0; k < 4; k++) {
    await p.mouse.move(l.x, l.y);
    await p.mouse.down();
    await p.mouse.move(r.x, r.y, { steps: 4 });
    await p.mouse.up();
  }
  await wait(200);
  check('a drag pan stops at the start', (await app('view')).t0 === full.t0 && span(await app('view')) === span(z), [await app('view'), full]);
  await p.mouse.move(5, 5);
  await p.keyboard.press('b');
  await p.click('#b-fit');
  for (let k = 0; k < 30 && span(await app('view')) !== span(full); k++) await wait(100);

  // hover: a time line and every chip's value
  const h = await plotAt(1, 0.4);
  await p.mouse.move(h.x, h.y);
  check('hover reads every plotted channel', await until(() => [...document.querySelectorAll('#stack [data-val]')].every((e) => e.textContent.trim() !== '')), await text('#stack'));
  check('pointer time in the bar', /\d/.test(await text('#hoverTxt')), await text('#hoverTxt'));
  await p.mouse.move(5, 5);
  check('hover clears on leave', await until(() => window.__tlmApp.hoverT() === null));

  // cursors: C, then two clicks; Measure opens with Δt and stats
  await p.keyboard.press('c');
  check('C picks Cursors mode', (await app('mode')) === 'cursor');
  const ca = await plotAt(0, 0.3);
  const cb = await plotAt(0, 0.6);
  await p.mouse.click(ca.x, ca.y);
  check('one cursor keeps Measure closed', (await app('cursors')).A && !(await p.isVisible('#measure')) && (await p.isVisible('#b-clear')));
  await p.mouse.click(cb.x, cb.y);
  check('both cursors open Measure', await until(() => !document.getElementById('measure').hidden), await app('cursors'));
  check('Measure shows Δt and 1 / Δt', await until(() => /Δt.*1 \/ Δt/.test(document.getElementById('measure').innerText.replace(/\s+/g, ' '))), await text('#measure'));
  check('...the frequency as the prototype\'s (Hz to three decimals, mHz under 1 Hz)', /1 \/ Δt \d+\.\d{3} Hz|1 \/ Δt \d+\.\d{2} mHz/.test((await text('#measure')).replace(/\s+/g, ' ')), await text('#measure'));
  check('exact stats per plotted channel', await until(() => document.querySelectorAll('#measure tr[data-mrow]').length === 6 && /Std dev/.test(document.getElementById('measure').innerText)), await text('#measure'));
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
  check('Measure shows tiles, a table in the channels\' decimals and RMS', await p.evaluate(() => {
    const m = document.getElementById('measure');
    return m.querySelectorAll('.mt').length === 4 && !!m.querySelector('.mt.hi') && /RMS/.test(m.innerText) && [...m.querySelectorAll('tr[data-mrow] td')].every((td) => !/\.\d{5,}/.test(td.textContent));
  }), await text('#measure'));
  await p.selectOption('#measure [data-mch]', '2');
  check('the channel picker changes the stats', await until(() => document.querySelector('#measure [data-mch]').value === '2' && /Samples/.test(document.getElementById('measure').innerText)));
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
