// The Canvas 2D fallback renderer: with no WebGL2 (the browser started without 3D APIs) the traces are still drawn, from the
// same uploads, into the same canvas; `?renderer=2d` asks for it on a machine that has WebGL.
import { run } from './lib.js';

/** How many pixels of the trace canvas have ink. */
const ink = () => {
  const c = document.getElementById('gl');
  const ctx = c.getContext('2d');
  if (!ctx) return -1;
  const d = ctx.getImageData(0, 0, c.width, c.height).data;
  let n = 0;
  for (let i = 3; i < d.length; i += 4) if (d[i] > 0) n++;
  return n;
};

run(
  'canvas2d',
  async ({ p, check, until }) => {
    check('without WebGL2 the canvas is a 2D one', await p.evaluate(() => !document.createElement('canvas').getContext('webgl2')));
    check('the traces are drawn into it', await until(() => {
      const c = document.getElementById('gl');
      const x = c.getContext('2d');
      return !!x && x.getImageData(0, 0, c.width, c.height).data.some((v, i) => i % 4 === 3 && v > 0);
    }), await p.evaluate(ink));
    const n = await p.evaluate(ink);
    check('with a few thousand pixels of line', n > 500, n);
    // a pan redraws: the ink moves, the canvas stays
    await p.keyboard.press('ArrowRight');
    await p.waitForTimeout(300);
    check('and still draws after the view moves', (await p.evaluate(ink)) > 500);
  },
  { launchArgs: ['--disable-3d-apis', '--disable-gpu'] },
);
