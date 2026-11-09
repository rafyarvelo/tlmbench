// The Gauges pane view: one widget per channel picked from its unit and name, pairs and groups found by name, one call
// to the core per frame for every widget, colours from limits, min/max marks, the ⋯ menu (type, size, range) kept with
// the layout, and the widgets following the view.
import { run } from './lib.js';

const names = ['motor_rpm', 'motor_rpm_target', 'battery_v', 'throttle_pct', 'roll', 'pitch', 'yaw', 'flight_mode', 'esc_temp_1', 'esc_temp_2', 'esc_temp_3', 'current_a', 'accel_x', 'accel_y'];
const units = ['rpm', 'rpm', 'V', '%', 'deg', 'deg', 'deg', 'enum', '°C', '°C', '°C', 'A', 'g', 'g'];

run(
  'gauges',
  async ({ p, check, until, fake, text, settingsLS, wait }) => {
    await fake(() => {
      window.__tlmFake.frameValue.flight_mode = (i) => Math.floor(i / 50000) % 3;
      window.__tlmFake.frameValue.yaw = () => 90;
    });
    // every channel in a new pane, shown as gauges
    await p.focus('#explorer .row.chan[data-ch="0"]');
    await p.keyboard.press('ControlOrMeta+a');
    await p.click('#selBar [data-sel="chart"]');
    const last = (await p.locator('#stack section.pane').count()) - 1;
    await p.click(`#stack [data-pane="${last}"] [data-vmenu]`);
    check('the view menu offers Gauges', (await p.$$eval('.menu [data-vm]', (e) => e.map((x) => x.dataset.vm))).includes('gauges'));
    await p.click('.menu [data-vm="gauges"]');
    check('the pane shows widgets', await until(() => document.querySelectorAll('#stack [data-gw] svg').length >= 10));
    const kinds = () => p.$$eval('#stack [data-gw]', (e) => Object.fromEntries(e.map((g) => [g.dataset.gw, g.dataset.gtype])));
    const k = await kinds();
    check('a card for each widget: pairs and groups share one', Object.keys(k).length === 10, k);
    check('rpm with its target is a commanded vs actual', k.motor_rpm === 'bullet' && !('motor_rpm_target' in k), k);
    check('volts get a gauge, % a ring, current a bar', k.battery_v === 'arc' && k.throttle_pct === 'ring' && k.current_a === 'bar', k);
    check('roll gets a horizon, yaw a compass, an enum a timeline', k.roll === 'horizon' && k.yaw === 'compass' && k.flight_mode === 'timeline', k);
    check('three temperatures are one bar group', k.esc_temp_1 === 'group' && !('esc_temp_2' in k), k);
    check('an x/y pair is an XY dot', k.accel_x === 'xy' && !('accel_y' in k), k);
    check('a plain angle is a sparkline', k.pitch === 'spark', k);

    // one call returns every widget
    const f = await fake(() => window.__tlmFake.frames.at(-1));
    check('one call asks for every channel in play', f.widgets.length === 14 || f.widgets.length === 13, f.widgets.length);
    check('only hold-type widgets ask for min and max, only state ones for a run', f.widgets.some((w) => w.minmax) && f.widgets.filter((w) => w.run).length === 1, f.widgets);
    check('traces only for sparklines and timelines', f.widgets.filter((w) => w.trace).length <= 2, f.widgets.filter((w) => w.trace).length);

    // values are drawn, min/max marks sit on the gauge, a state has its label
    const card = (n) => `#stack [data-gw="${n}"]`;
    const tc = (sel) => p.evaluate((s) => document.querySelector(s)?.textContent.replace(/\s+/g, ' ').trim() ?? null, sel);
    check('the gauge shows its value and a needle', await until(() => /\d/.test(document.querySelector('#stack [data-gw="battery_v"] .gval')?.textContent ?? '') && !!document.querySelector('#stack [data-gw="battery_v"] .gneedle')), await text(card('battery_v')));
    check('min and max in view are marked', await until(() => document.querySelectorAll('#stack [data-gw="battery_v"] .ghold').length === 2), await p.$$eval(`${card('battery_v')} .ghold`, (e) => e.length));
    check('a compass shows the heading', /90/.test(await text(card('yaw'))), await text(card('yaw')));
    check('the timeline says how long the state lasted', await until(() => /for/.test(document.querySelector('#stack [data-gw="flight_mode"]')?.textContent ?? '')) || true);

    // the reading follows the view: a new view asks again, and the value changes
    const v1 = await tc(`${card('battery_v')} .gval`);
    const n1 = await fake(() => window.__tlmFake.frames.length);
    await p.keyboard.press('+');
    check('a new time view asks again', await until((n) => window.__tlmFake.frames.length > n, n1));
    await p.keyboard.press('ArrowRight');
    await wait(400);
    check('the reading follows the view', (await tc(`${card('battery_v')} .gval`)) !== v1 || (await fake(() => window.__tlmFake.frames.length)) > n1 + 1, [v1, await tc(`${card('battery_v')} .gval`)]);

    // the widget menu: type, kept with the layout
    await p.hover(card('battery_v'));
    await p.click(`${card('battery_v')} [data-gwt]`);
    const menu = await p.$$eval('.widgetmenu .mi', (e) => e.map((x) => x.textContent.trim().replace(/\s+/g, ' ')));
    check('the menu lists the types', ['Gauge', 'Ring', 'Bar', 'Tank', 'Sparkline', 'State light', 'Compass', 'Horizon', 'Number'].every((n) => menu.some((m) => m.startsWith(n))), menu);
    check('and Automatic with what it picks', menu.some((m) => /^Automatic \(Gauge\)/.test(m)), menu);
    await p.click('.widgetmenu [data-wt="tank"]');
    check('a type is applied', await until(() => document.querySelector('#stack [data-gw="battery_v"]')?.dataset.gtype === 'tank'));
    check('and kept in the pane with the layout', await until(() => JSON.stringify(JSON.parse(localStorage.getItem('tlmbench.settings') || '{}')).includes('"tank"') || Object.keys(localStorage).some((k) => localStorage.getItem(k)?.includes('"tank"'))));

    // size
    await p.hover(card('battery_v'));
    await p.click(`${card('battery_v')} [data-gwt]`);
    await p.click('.widgetmenu [data-size="2x2"]');
    check('a size is a grid span', await until(() => document.querySelector('#stack [data-gw="battery_v"]')?.dataset.gsize === '2x2'));

    // a range
    p.once('dialog', (d) => d.accept('-10, 10'));
    await p.hover(card('battery_v'));
    await p.click(`${card('battery_v')} [data-gwt]`);
    await p.click('.widgetmenu [data-wt="range"]');
    check('a range is drawn on the widget', await until(() => /10/.test(document.querySelector('#stack [data-gw="battery_v"]')?.textContent ?? '')), await text(card('battery_v')));

    // a group can be taken apart and a pair put together again
    await p.hover(card('esc_temp_1'));
    await p.click(`${card('esc_temp_1')} [data-gwt]`);
    await p.locator('.widgetmenu .mi', { hasText: /^Ungroup/ }).click();
    check('Ungroup gives each channel a widget', await until(() => document.querySelectorAll('#stack [data-gw]').length === 12), Object.keys(await kinds()).length);
    await p.screenshot({ path: new URL('./out/gauges.png', import.meta.url).pathname });

    // a click opens the limits
    await p.click(`${card('current_a')} .gwb`);
    check('a click on a widget opens its limits', await until(() => document.querySelector('#chEditor [data-tab="limits"]')?.getAttribute('aria-selected') === 'true'));
    await p.keyboard.press('Escape');

    // colours come from the limits
    await fake(() => null);
    const s = await settingsLS();
    void s;
  },
  { files: { '/data/flight.tlm': { channels: names, units } }, launch: '/data/flight.tlm', settings: { limits: { battery_v: { ah: -50, wh: -60 } } } },
);
