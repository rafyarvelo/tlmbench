// The Spectrum pane view: chosen from the pane's view menu, it asks the core for each channel's spectrum over the time
// view, draws it and names each channel's peak; a view under 16 samples says to zoom out; the view is saved with the layout.
import { run } from './lib.js';

run(
  'spectrum',
  async ({ p, check, text, until, fake, settingsLS }) => {
    const asked = () => fake(() => window.__tlmFake.spectra);
    await p.click('#stack section.pane [data-vmenu]');
    check('the view menu offers Spectrum', (await p.$$eval('.menu [data-vm]', (e) => e.map((x) => x.dataset.vm))).includes('spectrum'));
    await p.click('.menu [data-vm="spectrum"]');
    check('the pane shows a spectrum', await until(() => !!document.querySelector('#stack [data-spectrum] canvas')));
    check('the core is asked for its channels over the time view', await until(() => window.__tlmFake.spectra.length >= 1));
    const a = (await asked()).at(-1);
    check('the pane\'s channels, the view\'s two times', a.channels.join() === '0,1' && BigInt(a.t1) > BigInt(a.t0), a);
    check('each channel\'s peak is named', await until(() => /accel_x: peak 12\.5 Hz/.test(document.querySelector('#stack .specinfo')?.textContent ?? '')), await text('#stack .specinfo'));
    check('the canvas has a size', await p.$eval('#stack [data-spectrum] canvas', (c) => c.width > 0 && c.height > 0));
    check('a spectrum pane has no chart plot', await p.evaluate(() => !document.querySelector('#stack section.pane').querySelector('.plot')));
    // a theme change repaints it at once (rc.1 test plan 3.7: canvases drawn here kept the old theme's colours)
    const hash = () => { const c = document.querySelector('#stack [data-spectrum] canvas'); const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let h = 0; for (let k = 0; k < d.length; k += 4) h = (h * 31 + d[k] + 7 * d[k + 1] + 13 * d[k + 2] + 17 * d[k + 3]) >>> 0; return h; };
    await p.mouse.move(5, 5);
    await p.evaluate((src) => (window.__look = (0, eval)(`(${src})`)()), hash.toString());
    await p.keyboard.press('t');
    check('the theme repaints the spectrum', await until((src) => (0, eval)(`(${src})`)() !== window.__look, hash.toString()));
    await p.keyboard.press('t');

    // a new time view asks again
    const n = (await asked()).length;
    await p.keyboard.press('+');
    check('a new time view asks again', await until((n) => window.__tlmFake.spectra.length > n, n));

    // nothing to show
    await fake(() => (window.__tlmFake.spectrumNone = true));
    await p.keyboard.press('+');
    check('a view too short to transform says to zoom out', await until(() => /Zoom out/.test(document.querySelector('#specNone')?.textContent ?? '')), await text('#stack'));
    await fake(() => (window.__tlmFake.spectrumNone = false));

    check('the pane\'s view is kept with the layout', JSON.stringify((await settingsLS()).layouts ?? {}).includes('spectrum'), JSON.stringify((await settingsLS()).layouts ?? {}).slice(0, 200));
    await p.click('#stack section.pane [data-vmenu]');
    await p.click('.menu [data-vm="chart"]');
    check('back to a chart', await until(() => !!document.querySelector('#stack section.pane .plot')));
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
