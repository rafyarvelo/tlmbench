// Play on a file tab (design: Time modes; ledger D-ui-13, D-ui-14), the app's own test restored (it was overwritten by the
// file-playback test): the transport opens paused, Play moves the playhead, speed, window presets, Space, the scrubber,
// Follow (a view you choose holds it, Follow brings it back), the end stops it, the A–B loop keeps the playhead between the
// cursors, Stop at pauses on an event, closing hides it, and each tab keeps its own playhead, restored paused.
import { run } from './lib.js';

const A = '/data/work/flight.tlm';
const B = '/data/work/other.tlm';

run(
  'transport',
  async ({ p, check, until, wait, fake }) => {
    const strip = () =>
      p.evaluate(() => {
        const s = document.getElementById('playStrip');
        return {
          shown: !!s && !s.hidden && !!s.offsetParent,
          time: s?.querySelector('[data-ptime]')?.textContent ?? '',
          play: s?.querySelector('[data-pplay]')?.getAttribute('aria-label') ?? '',
          follow: s?.querySelector('[data-pfollow]')?.getAttribute('aria-pressed') ?? '',
          scrub: Number(s?.querySelector('[data-pscrub]')?.value ?? 0),
        };
      });
    const state = () => p.evaluate(() => window.__tlmPlay.state());
    const scrubTo = (v) =>
      p.evaluate((v) => {
        const r = document.querySelector('#playStrip [data-pscrub]');
        r.value = String(v);
        r.dispatchEvent(new Event('input', { bubbles: true }));
      }, v);

    check('a Play button is in the top bar', await p.isVisible('#b-play'));
    await p.click('#b-play');
    let s = await strip();
    check('the transport opens paused, following', s.shown && s.play === 'Play' && s.follow === 'true', s);
    check('and the button shows pressed', (await p.getAttribute('#b-play', 'aria-pressed')) === 'true');

    const t0 = s.time;
    await p.click('#playStrip [data-pplay]');
    check('the playhead advances', await until((t) => document.querySelector('#playStrip [data-ptime]')?.textContent !== t, t0), await strip());
    check('and the button says Pause', (await strip()).play === 'Pause');

    await p.selectOption('#playStrip [data-pspeed]', '600');
    const a = (await state()).t;
    await wait(500);
    const b = (await state()).t;
    await wait(500);
    const c = (await state()).t;
    check('a faster speed moves the playhead faster', c - b > 0 && (await state()).speed === 600, [a, b, c]);

    await p.click('#playStrip [data-pwin="60"]');
    check('a window preset shows pressed', (await p.getAttribute('#playStrip [data-pwin="60"]', 'aria-pressed')) === 'true');

    // Space toggles play and pause when no field has the focus
    if (!(await state()).running) await p.click('#playStrip [data-pplay]');
    check('playing before Space', (await state()).running);
    await p.evaluate(() => document.activeElement?.blur());
    await p.keyboard.press('Space');
    check('Space pauses', await until(() => !window.__tlmPlay.state().running), await state());
    await p.keyboard.press('Space');
    check('and Space plays again', await until(() => window.__tlmPlay.state().running), await state());
    await p.keyboard.press('Space');
    await until(() => !window.__tlmPlay.state().running);

    // the scrubber
    await scrubTo(500);
    check('the scrubber moves the playhead to the middle', await until(() => window.__tlmPlay.state().t > 0), await state());
    const mid = (await state()).t;
    await scrubTo(150);
    check('and moves it back', await until((m) => window.__tlmPlay.state().t < m, mid), await state());

    // a view you choose holds the playhead; Follow brings the view back
    await p.click('#playStrip [data-pplay]');
    await p.click('#b-fit');
    check('choosing a view stops following', await until(() => document.querySelector('#playStrip [data-pfollow]')?.getAttribute('aria-pressed') === 'false'), await strip());
    const h0 = (await state()).t;
    await wait(500);
    check('the playhead keeps going while the view is held', (await state()).t > h0);
    await p.click('#playStrip [data-pfollow]');
    check('Follow resumes', (await strip()).follow === 'true');

    // the end stops it
    await scrubTo(999);
    check('playback stops at the end', await until(() => document.querySelector('#playStrip [data-pplay]')?.getAttribute('aria-label') === 'Play', null, 8000), await strip());

    // the A–B loop
    check('the loop is off until both cursors are placed', await p.evaluate(() => (window.__tlmPlay.cursors(null, null), document.querySelector('#playStrip [data-ploop]').disabled)));
    await p.evaluate(() => (window.__tlmPlay.cursors(60, 80), window.__tlmPlay.speed(10)));
    await p.click('#playStrip [data-ploop]');
    await p.click('#playStrip [data-pplay]');
    await wait(3500);
    const lp = await state();
    check('looping keeps the playhead between A and B while it plays', lp.loop && lp.running && lp.t >= 60 && lp.t <= 80, lp);
    await p.click('#playStrip [data-pplay]');
    await p.click('#playStrip [data-ploop]');
    await p.evaluate(() => window.__tlmPlay.cursors(null, null));

    // Stop at: the transport pauses at an event (there are none in the fake recording, so the choices are offered)
    const opts = await p.$$eval('#playStrip [data-pstop] option', (o) => o.map((x) => x.textContent ?? ''));
    check('Stop at offers nothing, any event and each category', opts.some((t) => /nothing/.test(t)) && opts.some((t) => /any event/.test(t)) && opts.some((t) => /Alarm/.test(t)), opts);

    await p.click('#playStrip [data-pclose]');
    check('closing hides the transport', !(await strip()).shown);

    // each tab keeps its own playhead, restored paused
    await p.click('#b-play');
    await p.evaluate(() => window.__tlmPlay.seek(1234 / 10));
    const kept = (await state()).t;
    await fake((b) => window.__tlmFake.drop([b]), B);
    check('another file opens without the transport', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 2) && !(await strip()).shown, await strip());
    await p.click('#wsTabs .wstab >> nth=0');
    await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent?.startsWith('flight'));
    const back = await state();
    check("coming back restores that tab's playhead, paused", (await strip()).shown && Math.abs(back.t - kept) < 1 && !back.running, [back, kept]);
  },
  { files: { [A]: { samples: 600000, rate: 100 }, [B]: {} }, launch: A },
);
