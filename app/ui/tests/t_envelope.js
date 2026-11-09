// The min/max band under a reduced line: a trace of min and max per pixel column gets a faint fill between its extremes
// (a spike or a noisy stretch reads as an area); a raw trace has none; Settings has the switch, kept in settings.
import { run } from './lib.js';

run(
  'envelope',
  async ({ p, check, until, wait, settingsLS }) => {
    const env = () => p.evaluate(() => window.__tlmApp.envelopes());
    check('a reduced trace has a band', await until(() => (window.__tlmApp.envelopes()[0] ?? []).some((n) => n > 0)), await env());
    await p.screenshot({ path: new URL('./out/envelope.png', import.meta.url).pathname });
    // zoomed in until every sample is drawn: the band goes
    await p.evaluate(() => document.activeElement?.blur());
    for (let k = 0; k < 28; k++) {
      await p.keyboard.press('+');
      await wait(80);
    }
    check('a raw trace has none', await until(() => (window.__tlmApp.envelopes()[0] ?? [1]).every((n) => n === 0), null, 8000), await env());
    // back out: it returns
    await p.click('#b-fit').catch(() => {});
    await p.keyboard.press('f');
    check('and it comes back zoomed out', await until(() => (window.__tlmApp.envelopes()[0] ?? []).some((n) => n > 0), null, 8000), await env());
    // the setting is listed in Settings and on by default
    await p.click('#b-settings');
    await p.click('#setPanel [data-sv="all"]');
    check('Settings lists the band', await until(() => /Min\/max band/.test(document.body.innerText)));
    check('it is on by default', ((await settingsLS()).envelope ?? true) === true);
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
