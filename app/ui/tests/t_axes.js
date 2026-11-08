// Pane axes: a pane's units get an axis each (left, then right, with room on the right for every plot); "Axis and
// scale…" in the ⋯ menu sets each axis's range (fit, ignore outliers, include zero, fixed), decimals, label and log
// scale; the settings are kept with the pane and Reset axes clears them. Y zoom (Shift + scroll, a drag on the y-axis)
// sets the fixed range; a double-click on the axis fits it again.
import { run } from './lib.js';

const FILE = '/data/flight.tlm';

run(
  'axes',
  async ({ p, check, until, settingsLS, wait }) => {
    const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
    const axes = () => app('axes');
    const saved = async (i) => (await settingsLS()).layouts[FILE][i].axis;
    const paneMenu = async (i) => {
      await p.hover(`#stack [data-pane="${i}"] .phead`);
      await p.click(`#stack [data-pane="${i}"] [data-pmenu]`);
    };
    const openAxes = async (i) => {
      await paneMenu(i);
      await p.locator('.panemenu .mi', { hasText: 'Axis and scale…' }).click();
    };
    const sec = (k) => `.axdlg [data-axis="u${k}"]`;

    // pane 1 holds V and A: two axes, and every plot leaves room for the right one's labels
    check('two units, two axes', await until(() => { const a = window.__tlmApp.axes()[0]; return a && a.u0 && a.u1; }), await axes());
    check('one unit, one axis', !(await axes())[1].u1, (await axes())[1]);
    const pr = await p.$$eval('#stack .plot', (ps) => ps.map((e) => e.style.getPropertyValue('--pr')));
    check('room on the right of every plot', pr.every((x) => x === '52px'), pr);

    await openAxes(0);
    const heads = await p.$$eval('.axdlg .axhead', (h) => h.map((x) => x.textContent.trim()));
    check('the dialog lists each axis with its unit', heads.join('|') === 'Left axisV|Right axisA', heads);

    // fixed: starts from what the axis shows, then takes min and max
    const before = (await axes())[0].u0;
    await p.selectOption(`${sec(0)} [data-ax="range"]`, 'fixed');
    const ax0 = await saved(0);
    check('fixed starts from the range shown', ax0?.u0?.range === 'fixed' && Math.abs(ax0.u0.min - before.min) < 0.01 * (before.max - before.min), [ax0, before]);
    await p.fill(`${sec(0)} [data-ax="min"]`, '-10');
    await p.fill(`${sec(0)} [data-ax="max"]`, '10');
    await p.press(`${sec(0)} [data-ax="max"]`, 'Tab');
    check('a fixed range is drawn', await until(() => { const a = window.__tlmApp.axes()[0].u0; return a.min === -10 && a.max === 10; }), (await axes())[0]);

    // decimals and a label
    await p.selectOption(`${sec(0)} [data-ax="dec"]`, '3');
    await p.fill(`${sec(0)} [data-ax="label"]`, 'Volts');
    await p.press(`${sec(0)} [data-ax="label"]`, 'Tab');
    check('decimals and label', await until(() => { const a = window.__tlmApp.axes()[0].u0; return a.dec === 3 && a.label === 'Volts'; }), (await axes())[0]);

    // the right axis: include zero, then log
    await p.selectOption(`${sec(1)} [data-ax="range"]`, 'zero');
    check('include zero', await until(() => { const a = window.__tlmApp.axes()[0].u1; return a.min <= 0 && a.max >= 0; }), (await axes())[0]);
    const lin = (await axes())[0].u1;
    await p.check(`${sec(1)} [data-ax="log"]`);
    check('log scale: the range in decades', await until(() => window.__tlmApp.axes()[0].u1.log === true), (await axes())[0]);
    check('the log axis is drawn after the next reply', await until(() => (window.__tlmApp.drawn()[0] || []).length === 2));
    check('its range changed', (await axes())[0].u1.max !== lin.max, [(await axes())[0].u1, lin]);
    check('saved with the pane', JSON.stringify(await saved(0)) === JSON.stringify({ u0: { range: 'fixed', min: -10, max: 10, dec: 3, label: 'Volts' }, u1: { range: 'zero', log: true } }), await saved(0));

    // Escape closes it; the settings come back after a reload
    await p.keyboard.press('Escape');
    check('Escape closes the dialog', !(await p.$('.axdlg')));
    await p.reload();
    await wait(800);
    check('kept after a reload', await until(() => { const a = window.__tlmApp.axes()[0]; return a && a.u0.min === -10 && a.u1.log === true; }), (await axes())[0]);

    // ignore outliers fits inside the plain fit
    const fit = (await axes())[1].u0;
    await openAxes(1);
    await p.selectOption(`${sec(0)} [data-ax="range"]`, 'robust');
    check('ignore outliers', await until((f) => { const a = window.__tlmApp.axes()[1].u0; return a.min >= f.min - 1e-9 && a.max <= f.max + 1e-9; }, fit) && (await saved(1))?.u0?.range === 'robust', [(await axes())[1], fit]);
    await p.click('.axdlg [data-close].tbtn');

    // Reset axes clears them
    await openAxes(0);
    await p.click('.axdlg [data-axreset]');
    check('Reset axes', (await saved(0)) === undefined && (await until(() => window.__tlmApp.axes()[0].u0.min !== -10)), await saved(0));
    await p.keyboard.press('Escape');

    // y zoom (design: Shift + scroll, or a drag on the y-axis): it sets the axis's fixed range
    const y0 = (await axes())[1].u0;
    const view0 = JSON.stringify(await app('view'));
    const c = await p.evaluate(() => { const r = document.querySelectorAll('#stack .plot')[1].getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2, gx: r.left + 20 }; });
    await p.mouse.move(c.x, c.y);
    await p.keyboard.down('Shift');
    await p.mouse.wheel(0, -300);
    await p.keyboard.up('Shift');
    check('Shift + scroll zooms y in', await until((b) => { const a = window.__tlmApp.axes()[1].u0; return a.max - a.min < (b.max - b.min) * 0.8; }, y0), [(await axes())[1].u0, y0]);
    check('...as the axis\'s fixed range, saved with the pane', (await saved(1))?.u0?.range === 'fixed' && typeof (await saved(1)).u0.min === 'number', await saved(1));
    check('...and the time view stays', JSON.stringify(await app('view')) === view0);
    const zoomed = (await axes())[1].u0;
    await p.mouse.move(c.gx, c.y);
    await p.mouse.down();
    await p.mouse.move(c.gx, c.y + 60, { steps: 4 });
    await p.mouse.up();
    check('dragging down the y-axis zooms out', await until((z) => { const a = window.__tlmApp.axes()[1].u0; return a.max - a.min > (z.max - z.min) * 1.5; }, zoomed), [(await axes())[1].u0, zoomed]);
    const out = (await axes())[1].u0;
    await p.mouse.dblclick(c.gx, c.y);
    check('double-click on the y-axis fits it again', await until((o) => { const a = window.__tlmApp.axes()[1].u0; return Math.abs(a.max - a.min - (o.max - o.min)) > 1e-6; }, out) && (await saved(1))?.u0?.range === undefined, [(await axes())[1].u0, await saved(1)]);

    // a table pane has no axes
    await p.hover('#stack [data-pane="2"] .phead');
    await p.click('#stack [data-pane="2"] [data-vmenu]');
    await p.click('.viewmenu [data-vm="table"]');
    await paneMenu(2);
    check('no axes for a table', !(await p.locator('.panemenu .mi', { hasText: 'Axis and scale…' }).count()));
  },
  { files: { [FILE]: { channels: ['volts', 'amps', 'v2', 'v3', 'm1', 'm2', 'x1', 'x2'], units: ['V', 'A', 'V', 'V', 'mV', 'mV', 'x', 'x'] } } },
);
