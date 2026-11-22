// Widget traces on a canvas (S5.4-06, owner Nov 17): the sparkline and the XY dot's trail and cloud draw on a canvas under
// the widget's SVG (text, grid and the dot stay SVG; dials stay SVG). The canvas is sized for the screen's pixel ratio,
// follows the widget's box, takes the theme's colours, and draws again as the reading moves.
import { run } from './lib.js';

const names = ['pitch', 'accel_x', 'accel_y', 'battery_v'];
const units = ['deg', 'g', 'g', 'V'];

run(
  'canvaswidgets',
  async ({ p, check, until }) => {
    // polls a check made here (until() runs its function in the page)
    const poll = async (f, ms = 4000) => {
      for (const end = Date.now() + ms; Date.now() < end; await new Promise((r) => setTimeout(r, 50))) {
        const v = await f();
        if (v) return v;
      }
      return null;
    };
    await p.focus('#explorer .row.chan[data-ch="0"]');
    await p.keyboard.press('ControlOrMeta+a');
    await p.click('#selBar [data-sel="chart"]');
    const last = (await p.locator('#stack section.pane').count()) - 1;
    await p.click(`#stack [data-pane="${last}"] [data-vmenu]`);
    await p.click('.menu [data-vm="gauges"]');
    await until(() => document.querySelectorAll('#stack [data-gw] svg').length >= 3);
    const kinds = await p.$$eval('#stack [data-gw]', (e) => Object.fromEntries(e.map((g) => [g.dataset.gw, g.dataset.gtype])));
    check('a sparkline, an XY dot and a dial', kinds.pitch === 'spark' && kinds.accel_x === 'xy' && kinds.battery_v === 'arc', kinds);

    const cv = (name) =>
      p.evaluate((name) => {
        const c = document.querySelector(`#stack [data-gw="${name}"] canvas.gcv`);
        if (!c) return null;
        const r = c.getBoundingClientRect();
        const svg = c.parentElement.querySelector('svg').getBoundingClientRect();
        const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
        let ink = 0;
        for (let i = 3; i < d.length; i += 4) if (d[i]) ink++;
        return { w: c.width, h: c.height, cw: r.width, ch: r.height, sw: svg.width, sh: svg.height, pts: +c.dataset.pts, ink, paths: c.parentElement.querySelectorAll('svg path[stroke-width="1.4"], svg path[stroke-width="1.8"], svg path[stroke-width=".7"]').length };
      }, name);
    const spark = await poll(async () => {
      const s = await cv('pitch');
      return s && s.pts > 10 && s.ink > 0 ? s : null;
    });
    check('the sparkline is drawn on a canvas, with ink', !!spark, await cv('pitch'));
    check('its line is no longer an SVG path', spark && spark.paths === 0, spark);
    check('the canvas covers the widget\'s box', spark && Math.abs(spark.cw - spark.sw) < 1 && Math.abs(spark.ch - spark.sh) < 1, spark);
    check('and has a pixel per device pixel (HiDPI: 2×)', spark && spark.w === Math.round(spark.cw * 2) && spark.h === Math.round(spark.ch * 2), spark);
    const xy = await poll(async () => {
      const s = await cv('accel_x');
      return s && s.pts > 2 && s.ink > 0 ? s : null;
    });
    check('the XY dot\'s cloud and trail are on a canvas too', !!xy && xy.paths === 0, await cv('accel_x'));
    check('its dot and text stay SVG', !!(await p.$('#stack [data-gw="accel_x"] svg .gdot')));
    check('a dial has no canvas', !(await p.$('#stack [data-gw="battery_v"] canvas')));

    // the theme's colours: the sparkline takes the channel's colour as the theme gives it
    const inkColour = () =>
      p.evaluate(() => {
        const c = document.querySelector('#stack [data-gw="pitch"] canvas.gcv');
        const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
        for (let i = 3; i < d.length; i += 4) if (d[i] > 200) return [d[i - 3], d[i - 2], d[i - 1]].join();
        return null;
      });
    const dark = await inkColour();
    await p.evaluate(() => window.__tlmSettings.set('theme', 'light'));
    const light = await poll(async () => {
      const c = await inkColour();
      return c && c !== dark ? c : null;
    });
    check('a theme change redraws it in the new colours', !!light && light !== dark, [dark, light]);

    // it draws again as the reading moves
    const before = (await cv('pitch')).pts;
    const t = await p.evaluate(() => window.__tlmApp.view());
    await p.evaluate((t) => window.__tlmApp.hoverAt(String(BigInt(t.t0) + (BigInt(t.t1) - BigInt(t.t0)) / 3n)), t);
    check('and keeps drawing as the reading moves', !!(await poll(async () => ((await cv('pitch'))?.pts ?? 0) > 0)) && before > 0);
  },
  { files: { '/data/flight.tlm': { channels: names, units } }, launch: '/data/flight.tlm', dpr: 2 },
);
