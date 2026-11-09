// Timing: a Gauges pane of 100 widgets keeps up with a moving view. The time the page takes to bring every widget up to
// a frame (derive the cards, patch the DOM) is recorded by the pane; it is the part that has to fit the budget of 2 ms
// for 100 widgets on the release app. In this headless browser the figure is printed and held to a looser bound.
// Run alone, after the rest (timings).
import { run } from './lib.js';

const N = 100;
// letters only: names that differ by a number would make bar groups
const letters = (k) => String.fromCharCode(97 + Math.floor(k / 26), 97 + (k % 26));
const names = Array.from({ length: N }, (_, k) => `chan_${letters(k)}`);
const kinds = ['V', '%', 'A', '°C', 'deg', 'rpm', 'g', 'W'];
const units = names.map((_, k) => kinds[k % kinds.length]);

run(
  'perf_gauges',
  async ({ p, check, until, wait, fake }) => {
    await p.focus('#explorer .row.chan[data-ch="0"]');
    await p.keyboard.press('ControlOrMeta+a');
    await p.click('#selBar [data-sel="chart"]');
    const idx = (await p.locator('#stack section.pane').count()) - 1;
    await p.click(`#stack [data-pane="${idx}"] [data-vmenu]`);
    await p.click('.menu [data-vm="gauges"]');
    check('100 channels, a widget each', await until((n) => document.querySelectorAll('#stack [data-gw] svg').length >= n, N * 0.9, 15000), await p.$$eval('#stack [data-gw]', (e) => e.length));
    await wait(500);
    await p.evaluate(() => (document.activeElement?.blur(), (window.__tlmGauges.renderMs.length = 0)));
    for (let k = 0; k < 3; k++) await p.keyboard.press('+');
    await wait(600);
    await p.evaluate(() => (window.__tlmGauges.renderMs.length = 0));
    const report = async (what, drive) => {
      await p.evaluate(() => ((window.__tlmGauges.renderMs.length = 0), (window.__tlmGauges.deriveMs.length = 0), (window.__tlmGauges.buildMs = [])));
      await drive();
      await wait(300);
      const ms = await p.evaluate(() => [...window.__tlmGauges.renderMs].sort((a, b) => a - b));
      const dv = await p.evaluate(() => [...window.__tlmGauges.deriveMs].sort((a, b) => a - b));
      const q = (a, f) => a[Math.min(a.length - 1, Math.floor(a.length * f))]?.toFixed(1);
      console.log(`  ${what}: ${ms.length} frames, median ${q(ms, 0.5)} ms, p90 ${q(ms, 0.9)} ms (deriving the cards: median ${q(dv, 0.5)} ms)`);
      const bm = await p.evaluate(() => [...(window.__tlmGauges.buildMs ?? [])].sort((a, b) => a - b));
      console.log(`    of which building the cards: median ${q(bm, 0.5)} ms`);
      return ms[ms.length >> 1];
    };
    // the pointer moving over a chart: the view stays, every widget reads a new time (what has to fit 2 ms in the release app)
    const [t0, t1] = await p.evaluate(() => [BigInt(window.__tlmApp.view().t0), BigInt(window.__tlmApp.view().t1)]);
    const med = await report('moving reading time', async () => {
      for (let k = 1; k <= 40; k++) {
        await p.evaluate((t) => window.__tlmApp.hoverAt(t), String(t0 + ((t1 - t0) * BigInt(k)) / 41n));
        await wait(40);
      }
    });
    check('frames were measured', med != null);
    check('a frame of 100 widgets stays well inside a 60 fps frame (the release app has to manage 2 ms)', med < 16, med);
    // the view moving too (Play): traces and clouds are asked for again each frame
    await report('moving view', async () => {
      await p.evaluate(() => document.activeElement?.blur());
      for (let k = 0; k < 20; k++) {
        await p.keyboard.press('ArrowRight');
        await wait(40);
      }
    });
    const ask = await fake(() => window.__tlmFake.frames.at(-1));
    check('still one call per frame, for every channel', ask.widgets.length === N, ask.widgets.length);
  },
  { files: { '/data/many.tlm': { channels: names, units } }, launch: '/data/many.tlm', viewport: { width: 1440, height: 5000 } },
);
