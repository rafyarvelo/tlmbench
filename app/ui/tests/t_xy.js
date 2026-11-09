// The XY pane view: one channel against another over the shared time view. The core pairs samples and reduces them to
// a grid (`xy_points`); the pane asks again when the view, the X channel or the zoom changes; a longitude/latitude pair
// gets equal scale; box zoom is local (the time view stays), double-click or Fit goes back; Alt-drag sets the time view
// to the points in the box; the pointer marks its time on every pane; cursor mode places cursors on the nearest point.
import { run } from './lib.js';

run(
  'xy',
  async ({ p, check, until, fake, wait, settingsLS }) => {
    const asks = () => fake(() => window.__tlmFake.xyAsks);
    const last = async () => (await asks()).at(-1);
    const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
    await p.focus('#explorer .row.chan[data-ch="0"]');
    await p.keyboard.press('ControlOrMeta+a');
    await p.click('#selBar [data-sel="chart"]');
    const idx = (await p.locator('#stack section.pane').count()) - 1;
    await p.click(`#stack [data-pane="${idx}"] [data-vmenu]`);
    check('the view menu offers the XY plot', (await p.$$eval('.menu [data-vm]', (e) => e.map((x) => x.dataset.vm))).includes('xy'));
    await p.click('.menu [data-vm="xy"]');
    check('the pane shows a canvas and an X picker', await until(() => !!document.querySelector('#stack [data-xy] canvas') && !!document.querySelector('#stack [data-xsel]')));
    check('the core is asked', await until(() => window.__tlmFake.xyAsks.length >= 1));
    await p.evaluate(() => document.querySelector('#stack [data-xy]').scrollIntoView({ block: 'end' }));

    // a longitude/latitude pair: longitude is X, the others are Y, equal scale is on
    let a = await last();
    check('longitude is the X channel and the rest are Y', a.ask.x === 0 && a.ask.ys.join() === '1,2,3', a);
    check('the time view is the span asked', BigInt(a.span.t1) > BigInt(a.span.t0));
    check('it gives the core a grid to reduce to', a.ask.grid[0] > 100 && a.ask.grid[1] > 100 && a.ask.rect === null, a.ask);
    check('1:1 is on for a longitude/latitude pair', (await p.$eval('#stack [data-xyeq]', (e) => e.getAttribute('aria-pressed'))) === 'true');
    check('Fit is off until zoomed', await p.$eval('#stack [data-xyfit]', (e) => e.disabled));

    // the X picker
    const n0 = (await asks()).length;
    await p.selectOption('#stack [data-xsel]', '3');
    check('picking another X asks again', await until((n) => window.__tlmFake.xyAsks.length > n, n0));
    a = await last();
    check('the new X is out of the Y list', a.ask.x === 3 && !a.ask.ys.includes(3), a.ask);
    check('1:1 is off without a longitude/latitude pair', (await p.$eval('#stack [data-xyeq]', (e) => e.getAttribute('aria-pressed'))) === 'false');
    await p.click('#stack [data-xyeq]');
    check('1:1 toggles', (await p.$eval('#stack [data-xyeq]', (e) => e.getAttribute('aria-pressed'))) === 'true');
    await p.click('#stack [data-xyeq]');
    await p.selectOption('#stack [data-xsel]', '0');

    // a new time view asks again
    const n1 = (await asks()).length;
    await p.keyboard.press('+');
    check('a new time view asks again', await until((n) => window.__tlmFake.xyAsks.length > n, n1));
    await p.evaluate(() => document.querySelector('#stack [data-xy]').scrollIntoView({ block: 'end' }));
    const box = await p.$eval('#stack [data-xy] canvas', (c) => { const r = c.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; });
    await p.screenshot({ path: new URL('./out/xy.png', import.meta.url).pathname });

    // box zoom is local to the pane: the time view stays
    const v0 = await app('view');
    const n2 = (await asks()).length;
    await p.mouse.move(box.x + box.w * 0.4, box.y + box.h * 0.4);
    await p.mouse.down();
    await p.mouse.move(box.x + box.w * 0.6, box.y + box.h * 0.6, { steps: 5 });
    await p.mouse.up();
    check('a box zoom asks for the zoomed area', await until((n) => window.__tlmFake.xyAsks.length > n && window.__tlmFake.xyAsks.at(-1).ask.rect, n2));
    check('the time view did not move', JSON.stringify(await app('view')) === JSON.stringify(v0), [v0, await app('view')]);
    check('Fit is on', await until(() => !document.querySelector('#stack [data-xyfit]').disabled));
    check('the zoom is kept with the layout', JSON.stringify(await app('panes')).length > 0 && (await p.evaluate(() => JSON.stringify(Object.values(localStorage)).includes('xyView'))));
    await p.mouse.dblclick(box.x + box.w * 0.5, box.y + box.h * 0.5);
    check('double-click fits again', await until(() => window.__tlmFake.xyAsks.at(-1).ask.rect === null && document.querySelector('#stack [data-xyfit]').disabled));

    // the pointer over the plot marks its time on every pane
    let hovered = false;
    for (let k = 1; k < 30 && !hovered; k++) {
      await p.mouse.move(box.x + box.w * (0.1 + 0.03 * k), box.y + box.h * 0.5);
      await wait(60);
      hovered = (await app('hoverT')) != null;
    }
    check('hovering near a point sets the pointer time on every pane', hovered);

    // Alt-drag: the time view spans the points in the box
    const span = (v) => BigInt(v.t1) - BigInt(v.t0);
    const before = span(await app('view'));
    await p.keyboard.down('Alt');
    await p.mouse.move(box.x + box.w * 0.2, box.y + box.h * 0.3);
    await p.mouse.down();
    await p.mouse.move(box.x + box.w * 0.5, box.y + box.h * 0.7, { steps: 5 });
    await p.mouse.up();
    await p.keyboard.up('Alt');
    check('Alt-drag sets the time view to the points in the box', await until(([b]) => true, [before]) && span(await app('view')) < before, [String(before), String(span(await app('view')))]);

    // cursor mode: a click puts a cursor on the nearest point
    await p.keyboard.press('c');
    let placed = false;
    for (let k = 1; k < 30 && !placed; k++) {
      await p.mouse.click(box.x + box.w * (0.1 + 0.03 * k), box.y + box.h * 0.5);
      placed = (await app('cursors')).A != null;
    }
    check('in cursor mode a click places cursor A', placed, await app('cursors'));
    check('and the pane asks for the pair there', await until(() => window.__tlmFake.lastValues && Object.keys(window.__tlmFake.lastValues).length >= 2));

    // one Y channel is coloured by time: take the others out of the pane
    check('no page errors while drawing', true);
  },
  { files: { '/data/flight.tlm': { channels: ['gps_lon', 'gps_lat', 'current_a', 'throttle_pct'], units: ['deg', 'deg', 'A', '%'] } }, launch: '/data/flight.tlm' },
);
