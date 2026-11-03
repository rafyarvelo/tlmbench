// Play: the transport (play/pause, Space, the playhead's time, scrubber, speed, window presets, Follow, the A–B
// loop, close), the view following the playhead and holding when you choose one, readouts following the playhead,
// stopping at the end, and each tab keeping its playhead (paused).
import { run } from './lib.js';

const B = '/data/b.csv';
run('playback', async ({ p, check, fake, plotAt, until, wait }) => {
  const st = () => p.evaluate(() => window.__tlmPlay.state());
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const shown = () => p.evaluate(() => !!document.getElementById('playStrip'));
  const label = () => p.getAttribute('#playStrip [data-pplay]', 'aria-label');

  check('Play back in the top bar', await p.isVisible('#b-play'));
  await p.click('#b-play');
  check('the transport opens, paused and following', (await shown()) && (await label()) === 'Play' && /Following/.test(await p.textContent('#playStrip [data-pfollow]')));
  check('the button shows pressed', (await p.getAttribute('#b-play', 'aria-pressed')) === 'true');
  const s0 = await st();
  check('a speed that plays the file in about 3 minutes', s0.end / s0.speed <= 180 && s0.end / s0.speed > 30, s0);

  // play: the playhead moves and the view follows it (the playhead at 85% of the window)
  await p.click('#playStrip [data-pplay]');
  await wait(1200);
  const s1 = await st();
  check('the playhead advances', s1.running && s1.t > s0.t, [s0.t, s1.t]);
  const v = await app('view');
  const at = (s1.t * 1e9 - (Number(BigInt(v.t0) - 1767225600000000000n))) / Number(BigInt(v.t1) - BigInt(v.t0));
  check('the view follows it', Math.abs(at - 0.85) < 0.1, at);
  check('readouts follow the playhead', await until(() => [...document.querySelectorAll('#stack [data-val]')].every((e) => e.textContent.trim() !== '')));

  // speed and window (from early on, so the end isn't reached meanwhile)
  await p.evaluate(() => window.__tlmPlay.seek(20));
  await p.selectOption('#playStrip [data-pspeed]', '30');
  const a = (await st()).t;
  await wait(500);
  check('a faster speed moves faster', (await st()).t - a > 10, [(await st()).t, a]);
  await p.click('#playStrip [data-pwin="10"]');
  const w = await app('view');
  check('a window preset sets the span', Math.abs(Number(BigInt(w.t1) - BigInt(w.t0)) / 1e9 - 10) < 0.01 && (await p.getAttribute('#playStrip [data-pwin="10"]', 'aria-pressed')) === 'true', w);

  // Space pauses; scrubbing moves the playhead
  // (with a transport button focused, Space presses that button instead)
  await p.evaluate(() => document.activeElement?.blur());
  await p.keyboard.press('Space');
  check('Space pauses', await until(() => !window.__tlmPlay.state().running));
  await p.evaluate(() => { const r = document.querySelector('#playStrip [data-pscrub]'); r.value = '500'; r.dispatchEvent(new Event('input', { bubbles: true })); });
  const mid = await st();
  check('scrubbing moves the playhead', Math.abs(mid.t - mid.end / 2) < mid.end / 500, mid);
  check('the time shows it', /^00:01:40/.test(await p.textContent('#playStrip [data-ptime]')), await p.textContent('#playStrip [data-ptime]'));

  // a view you choose stops following; the playhead keeps going; Follow comes back
  await p.click('#playStrip [data-pplay]');
  await p.click('#b-fit');
  // a glide takes about 300 ms of frames, more where drawing is slow: wait for the view to settle
  for (let k = 0, last = ''; k < 40; k++) {
    const v = JSON.stringify(await app('view'));
    if (v === last) break;
    last = v;
    await wait(200);
  }
  check('choosing a view stops following', /Follow playhead/.test(await p.textContent('#playStrip [data-pfollow]')));
  const held = await app('view');
  const t1 = (await st()).t;
  await wait(400);
  check('the playhead keeps going while the view holds', (await st()).t > t1 && JSON.stringify(await app('view')) === JSON.stringify(held));
  await p.click('#playStrip [data-pfollow]');
  check('Follow brings the view back', /Following/.test(await p.textContent('#playStrip [data-pfollow]')) && JSON.stringify(await app('view')) !== JSON.stringify(held));

  // the end stops playback
  await p.evaluate(() => window.__tlmPlay.seek(199));
  check('it stops at the end', await until(() => !window.__tlmPlay.state().running, null, 4000) && (await label()) === 'Play');

  // the A–B loop
  await p.evaluate(() => window.__tlmPlay.cursors(null, null));
  check('the loop is off until both cursors are placed', await p.isDisabled('#playStrip [data-ploop]'));
  await p.evaluate(() => { window.__tlmPlay.cursors(60, 62); window.__tlmPlay.speed(5); });
  await p.click('#playStrip [data-ploop]');
  await p.click('#playStrip [data-pplay]');
  await wait(1500);
  const lp = await st();
  check('looping keeps the playhead between A and B', lp.loop && lp.running && lp.t >= 60 && lp.t <= 62, lp);
  await p.click('#playStrip [data-pplay]');

  // each tab keeps its playhead, paused; another tab opens in Analyze
  await p.evaluate(() => window.__tlmPlay.seek(123));
  await fake((f) => window.__tlmFake.drop([f]), B);
  await until((f) => document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip === f, B);
  check('another tab has no transport', !(await shown()) && (await p.getAttribute('#b-play', 'aria-pressed')) === 'false');
  await p.click('#wsTabs .wstab[data-tip="/data/flight.tlm"]');
  const back = await until(() => window.__tlmPlay.state().open && window.__tlmPlay.state());
  check('coming back restores its playhead, paused', back && Math.abs(back.t - 123) < 0.01 && !back.running, back);

  // close: the view stays
  const before = await app('view');
  await p.click('#playStrip [data-pclose]');
  check('close hides the transport and the view stays', !(await shown()) && JSON.stringify(await app('view')) === JSON.stringify(before));
  await p.keyboard.press('Space');
  check('Space opens it and plays', await until(() => window.__tlmPlay.state().running));
  void plotAt;
}, { files: { '/data/flight.tlm': {}, [B]: { channels: ['volts'] } }, launch: '/data/flight.tlm' });
