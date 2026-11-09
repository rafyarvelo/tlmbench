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
