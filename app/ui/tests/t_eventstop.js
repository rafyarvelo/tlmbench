// Events elsewhere: a table's rows inside an event get its category's tint and stripe, with ▸ on a range's first row,
// ◂ on its last and ● on an instant's; the pane's toggle and E hide them. Play's Stop at pauses with the next matching
// event at the playhead and the next play goes on past it.
import { run } from './lib.js';

run(
  'eventstop',
  async ({ p, check, fake, until, plotAt, settingsLS }) => {
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

    // Stop at: a menu with Nothing, Any event and a tick per category (several can be ticked)
    const stop = async (...ids) => {
      await p.click('#playStrip [data-pstop]');
      for (const id of ids) await p.click(`.stopmenu [data-stop="${id}"]`);
      if (await p.$('.stopmenu')) await p.keyboard.press('Escape');
    };
    await p.click('#b-play');
    await stop('any');
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
    await stop('mode');
    await p.evaluate(() => { window.__tlmPlay.seek(5); window.__tlmPlay.speed(30); });
    await p.click('#playStrip [data-pplay]');
    check('a category passes the others by', await until(() => !window.__tlmPlay.state().running && window.__tlmPlay.state().t > 29.9), JSON.stringify(await st()));

    // several categories (S5.2-15): tick another and the first of either stops it
    await stop('alarm');
    check('the button says what is ticked', /Stop at .*Alarm/.test(await p.$eval('#playStrip [data-pstop]', (e) => e.textContent ?? '')) && /Mode/.test(await p.$eval('#playStrip [data-pstop]', (e) => e.textContent ?? '')), await p.$eval('#playStrip [data-pstop]', (e) => e.textContent));
    await p.evaluate(() => { window.__tlmPlay.seek(5); window.__tlmPlay.speed(5); });
    await p.click('#playStrip [data-pplay]');
    check('with two categories ticked it stops at the first event of either', await until(() => !window.__tlmPlay.state().running), JSON.stringify(await st()));
    check('that is the alarm at 10 s', Math.abs((await st()).t - 10) < 0.01, await st());
    await stop('alarm');
    check('unticking leaves the other', (await settingsLS()).stopAt === 'cat:mode', (await settingsLS()).stopAt);
    await stop('off');
    await p.evaluate(() => window.__tlmPlay.seek(5));
    await p.evaluate(() => window.__tlmPlay.speed(30));
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
    // the Event column names the event at each row (S5.2-13): last, display only
    check('an Event column comes last and names the event on its rows', await until(() => [...document.querySelectorAll('#stack .tth')].at(-1)?.dataset.col === 'e') && await p.evaluate(() => [...document.querySelectorAll('#stack .trw.inev')].some((r) => /Early|Climb|Spike/.test(r.lastElementChild?.textContent ?? ''))), await p.$$eval('#stack .tth', (h) => h.map((x) => x.dataset.col)));
    // a tag click shows the event in the Events panel (S5.2-13)
    await p.click('#stack [data-evtag]');
    check('a click on an event tag opens the Events panel on that event', await until(() => !!document.querySelector('#eventsPanel .ev.revealed')), await p.$$eval('#eventsPanel .ev', (e) => e.length));
    await p.click('[data-rail="explorer"]');
    check('and has no sort, filter or column menu', !(await p.$('#stack .tth[data-col="e"] [data-filter]')) && !(await p.$('#stack .tth[data-col="e"] [data-colmenu]')));
    await p.keyboard.press('e');
    check('E hides them', await until(() => document.querySelectorAll('#stack .trw.inev').length === 0));
    await p.keyboard.press('e');
    check('and shows them', await until(() => document.querySelectorAll('#stack .trw.inev').length > 0));
    await p.keyboard.press('e');
    check('with events hidden the Event column goes too', await until(() => !document.querySelector('#stack .tth[data-col="e"]')));
    await p.keyboard.press('e');
  },
  { files: { '/data/flight.tlm': {} }, launch: '/data/flight.tlm' },
);
