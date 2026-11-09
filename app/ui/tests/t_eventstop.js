// Events elsewhere: a table's rows inside an event get its category's tint and stripe, with ▸ on a range's first row,
// ◂ on its last and ● on an instant's; the pane's toggle and E hide them. Play's Stop at pauses with the next matching
// event at the playhead and the next play goes on past it.
import { run } from './lib.js';

run(
  'eventstop',
  async ({ p, check, fake, until, plotAt }) => {
    const st = () => p.evaluate(() => window.__tlmPlay.state());
    await fake(() => {
      const t0 = BigInt(window.__tlmApp.view().t0);
      const sec = (s) => String(t0 + BigInt(Math.round(s * 1e9)));
      window.__tlmFake.events['/data/flight.tlm'] = [
        { id: 'a', name: 'Spike', start: sec(10), duration: '0', category: 'alarm', source: 'user', channels: [] },
        { id: 'b', name: 'Climb', start: sec(30), duration: String(5e9), category: 'mode', source: 'user', channels: [] },
        { id: 'c', name: 'Early', start: sec(0.001), duration: '2000000', category: 'mode', source: 'user', channels: [] },
        { id: 'd', name: 'Early spike', start: sec(0.004), duration: '0', category: 'alarm', source: 'user', channels: [] },
      ];
    });
    // reload the file's events through the panel (a marker add reads them all)
    await p.click('[data-rail="events"]');
    await p.keyboard.press('c');
    const a = await plotAt(0, 0.9);
    await p.mouse.click(a.x, a.y);
    await p.click('#evMarker');
    check('the events are in', await until(() => document.querySelectorAll('#eventsPanel .ev[data-ev]').length === 5));
    await p.keyboard.press('Escape');

    // Stop at
    await p.click('#b-play');
    await p.selectOption('#playStrip [data-pstop]', 'any');
    check('Stop at is a setting', true);
    await p.evaluate(() => { window.__tlmPlay.seek(5); window.__tlmPlay.speed(5); });
    await p.click('#playStrip [data-pplay]');
    check('play pauses at the first event', await until(() => !window.__tlmPlay.state().running), JSON.stringify(await st()));
    const s1 = await st();
    check('with the playhead on it', Math.abs(s1.t - 10) < 0.01, s1);
    await p.click('#playStrip [data-pplay]');
    check('the next play goes on past it and stops at the next', await until(() => !window.__tlmPlay.state().running && window.__tlmPlay.state().t > 29.9), JSON.stringify(await st()));
    check('that is the range\'s start', Math.abs((await st()).t - 30) < 0.01, await st());

    // one category only
    await p.selectOption('#playStrip [data-pstop]', 'cat:mode');
    await p.evaluate(() => { window.__tlmPlay.seek(5); window.__tlmPlay.speed(30); });
    await p.click('#playStrip [data-pplay]');
    check('a category passes the others by', await until(() => !window.__tlmPlay.state().running && window.__tlmPlay.state().t > 29.9), JSON.stringify(await st()));
    await p.selectOption('#playStrip [data-pstop]', 'off');
    await p.evaluate(() => window.__tlmPlay.seek(5));
    await p.click('#playStrip [data-pplay]');
    check('with Nothing it plays on', await until(() => window.__tlmPlay.state().t > 36), JSON.stringify(await st()));
    await p.click('#playStrip [data-pplay]');
    await p.click('#playStrip [data-pclose]');
    await p.keyboard.press('f');

    // the table
    await p.click('#stack section.pane [data-vmenu]');
    await p.click('.menu [data-vm="table"]');
    check('table rows inside events are tinted', await until(() => document.querySelectorAll('#stack .trw.inev').length > 0));
    check('with tags: ▸ on the first row, ◂ on the last, ● on an instant', await until(() => { const t = [...document.querySelectorAll('#stack [data-evtag]')].map((e) => e.textContent).join(''); return t.includes('▸') && t.includes('◂') && t.includes('●'); }), await p.$$eval('#stack [data-evtag]', (e) => e.map((x) => x.textContent)));
    await p.keyboard.press('e');
    check('E hides them', await until(() => document.querySelectorAll('#stack .trw.inev').length === 0));
    await p.keyboard.press('e');
    check('and shows them', await until(() => document.querySelectorAll('#stack .trw.inev').length > 0));
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
