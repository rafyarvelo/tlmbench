// Pane modes from the owner's sheet (B5): XY's controls sit in the pane header as the prototype's (X: channel, 1:1, Fit) and
// its plot resizes with a grip like a chart's; Chart and values has its height grip at the pane's bottom, under the tiles;
// the gauges' ⋯ menu is the prototype's compact one (names with a check, no hint under each).
import { run } from './lib.js';

run('panegrips', async ({ p, check, until }) => {
  const mode = async (i, m) => {
    await p.hover(`#stack [data-pane="${i}"] .phead`);
    await p.click(`#stack [data-pane="${i}"] [data-vmenu]`);
    await p.click(`.viewmenu [data-vm="${m}"]`);
  };
  const drag = async (sel, dy) => {
    await p.locator(sel).scrollIntoViewIfNeeded();
    const b = await p.locator(sel).boundingBox();
    await p.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
    await p.mouse.down();
    await p.mouse.move(b.x + b.width / 2, b.y + b.height / 2 + dy, { steps: 5 });
    await p.mouse.up();
  };

  // XY
  await mode(0, 'xy');
  const X = '#stack [data-pane="0"]';
  check('XY: the X picker, 1:1 and Fit are in the pane header', await until((s) => !!document.querySelector(`${s} .phead [data-xsel]`) && !!document.querySelector(`${s} .phead [data-xyeq]`) && !!document.querySelector(`${s} .phead [data-xyfit]`), X));
  check('and not in a row of their own inside the plot', !(await p.$(`${X} [data-xy] [data-xsel]`)));
  check('the picker names the axis: "X: accel_x"', /^X: /.test(await p.$eval(`${X} [data-xsel] option:checked`, (o) => o.textContent)));
  await p.click('#stack [data-pane="1"] .pnum');
  await p.mouse.move(5, 890);
  check('they hide until the pane is hovered or selected (calm UI)', await until((s) => getComputedStyle(document.querySelector(`${s} [data-xyeq]`)).visibility === 'hidden', X));
  const h0 = await p.$eval(`${X} [data-xy] canvas`, (c) => c.getBoundingClientRect().height);
  check('XY has a height grip under the plot', !!(await p.$(`${X} [data-xy] + .hgrip`)));
  await drag(`${X} [data-xy] + .hgrip`, 80);
  check('dragging it makes the plot taller', await until(([s, h]) => document.querySelector(`${s} [data-xy] canvas`).getBoundingClientRect().height > h + 60, [X, h0]), [h0, await p.$eval(`${X} [data-xy] canvas`, (c) => c.getBoundingClientRect().height)]);

  // Chart and values: the grip is the pane's last thing, under the tiles
  await mode(1, 'cv');
  const C = '#stack [data-pane="1"]';
  check('Chart and values: the height grip is at the pane\'s bottom, under the tiles', await until((s) => { const sec = document.querySelector(s); return sec?.lastElementChild?.classList.contains('hgrip') && !sec.querySelector('.plot + .hgrip'); }, C));
  await p.waitForTimeout(500); // the tiles fill in, which moves the grip
  const ph0 = await p.$eval(`${C} .plot`, (e) => e.getBoundingClientRect().height);
  await drag(`${C} > .hgrip`, 60);
  check('dragging it still sizes the chart', await until(([s, h]) => document.querySelector(`${s} .plot`).getBoundingClientRect().height > h + 40, [C, ph0]), [ph0, await p.$eval(`${C} .plot`, (e) => e.getBoundingClientRect().height)]);

  // the gauges' widget menu
  await mode(2, 'gauges');
  await until(() => !!document.querySelector('#stack [data-pane="2"] .gwt'));
  await p.hover('#stack [data-pane="2"] .gw');
  await p.click('#stack [data-pane="2"] .gwt');
  check('the widget menu is the compact channel menu', await until(() => !!document.querySelector('.widgetmenu.chmenu')));
  check('with no hint under each built-in widget name', await p.$$eval('.widgetmenu .mi[data-wt]', (e) => e.filter((x) => !/^(sw_|fw_|cw_)/.test(x.dataset.wt)).every((x) => !x.querySelector('small'))));
  await p.keyboard.press('Escape');
  check('and closes without an error', await until(() => !document.querySelector('.widgetmenu')));
});
