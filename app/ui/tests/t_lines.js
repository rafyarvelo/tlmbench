// Plot lines have a width (1.25 CSS pixels, as in the prototype), not WebGL's one device pixel: drawn as quads, a
// trace covers clearly more pixels than a one-pixel line. (Software GL, as in these tests, defaults to the thin
// lines because quads draw slowly there, so the test forces each.)
import { run } from './lib.js';

run('lines', async ({ p, check, wait, until }) => {
  const lit = async () => {
    await wait(700);
    const png = (await p.screenshot()).toString('base64');
    const rect = await p.evaluate(() => { const r = document.querySelector('#stack [data-pane="0"] .plot').getBoundingClientRect(); return { x: r.left + 60, y: r.top + 4, w: r.width - 80, h: r.height - 8 }; });
    return p.evaluate(async ([png, r]) => {
      const img = new Image();
      img.src = 'data:image/png;base64,' + png;
      await img.decode();
      const c = document.createElement('canvas');
      c.width = img.width;
      c.height = img.height;
      const g = c.getContext('2d');
      g.drawImage(img, 0, 0);
      const d = g.getImageData(Math.round(r.x), Math.round(r.y), Math.round(r.w), Math.round(r.h)).data;
      let n = 0;
      // a trace pixel is bright and coloured; the background and grid are dark
      for (let i = 0; i < d.length; i += 4) if (d[i] + d[i + 1] + d[i + 2] > 330) n++;
      return n;
    }, [png, rect]);
  };
  const mode = async (m) => {
    await p.evaluate((m) => { window.__tlmLines = m; localStorage.setItem('lines', m); }, m);
    await p.reload();
    await wait(1200);
  };
  await p.addInitScript(() => { const m = localStorage.getItem('lines'); if (m) window.__tlmLines = m; });
  await mode('strip');
  const thin = await lit();
  await mode('quads');
  const thick = await lit();
  check('quads cover clearly more pixels than one-pixel lines', thick > thin * 1.3 && thin > 200, { thin, thick });
});
