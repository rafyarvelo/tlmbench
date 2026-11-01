// Playback: a file tab played like a live one — transport, playhead, follow/hold, scrub, speed, window, tiles follow, Space, tab switch closes it.
const T = require('./lib');
T.run('playback', async ({ p, check, wait, until, pick }) => {
  const st = () => p.evaluate(() => { const s = document.getElementById('playStrip'); return { shown: !!s && !s.hidden, time: s && s.querySelector('[data-ptime]') ? s.querySelector('[data-ptime]').textContent : '', follow: s && s.querySelector('[data-pfollow]') ? s.querySelector('[data-pfollow]').textContent : '', play: s && s.querySelector('[data-pplay]') ? s.querySelector('[data-pplay]').getAttribute('aria-label') : '', scrub: s && s.querySelector('[data-pscrub]') ? +s.querySelector('[data-pscrub]').value : -1, view: document.querySelector('#stack .phead') ? 1 : 0 }; });
  check('play button in the toolbar', await p.evaluate(() => { const b = document.getElementById('b-play'); return !!b && !!b.offsetParent; }));
  await p.click('#b-play'); await wait(300);
  let s = await st();
  check('transport opens', s.shown && s.play === 'Play' && /Following/.test(s.follow), s);
  check('button shows pressed', await p.evaluate(() => document.getElementById('b-play').getAttribute('aria-pressed') === 'true'));

  const t0 = s.time;
  await p.click('#playStrip [data-pplay]'); await wait(1500);
  s = await st();
  check('playhead advances', s.play === 'Pause' && s.time !== t0, [t0, s.time]);
  const rangeA = await p.evaluate(() => document.querySelector('.ovlabel, #ovLabel') ? 1 : 0);
  void rangeA;

  // speed change
  await p.selectOption('#playStrip [data-pspeed]', '600'); await wait(800);
  const sc1 = (await st()).scrub; await wait(800);
  check('faster speed moves faster', (await st()).scrub > sc1, [sc1, (await st()).scrub]);

  // window preset
  await p.click('#playStrip [data-pwin="60"]'); await wait(300);
  check('window preset pressed', await p.evaluate(() => document.querySelector('#playStrip [data-pwin="60"]').getAttribute('aria-pressed') === 'true'));

  // pause with Space
  await p.mouse.click(5, 5); await p.keyboard.press('Space'); await wait(300);
  s = await st();
  check('Space pauses', s.play === 'Play', s);

  // scrub
  await p.evaluate(() => { const r = document.querySelector('#playStrip [data-pscrub]'); r.value = '500'; r.dispatchEvent(new Event('input', { bubbles: true })); }); await wait(300);
  const mid = (await st()).time;
  check('scrub moves the playhead to the middle', /\d/.test(mid), mid);

  // tiles follow the playhead: switch a pane to tiles and compare values at two playhead positions
  const vals = async () => p.evaluate(() => [...document.querySelectorAll('#stack .ftile .fv, #stack .gw svg')].slice(0, 3).map(e => e.textContent.trim()).join('|'));
  await p.evaluate(() => { const b = document.querySelector('#stack section.pane [data-viewmenu]'); b && b.click(); }); await wait(200);
  await p.evaluate(() => { const b = document.querySelector('.viewmenu [data-vm="gauge"]'); b && b.click(); }); await wait(800);
  const vMid = await vals();
  await p.evaluate(() => { const r = document.querySelector('#playStrip [data-pscrub]'); r.value = '150'; r.dispatchEvent(new Event('input', { bubbles: true })); }); await wait(600);
  const vEarly = await vals();
  check('gauges follow the playhead', vMid && vEarly && vMid !== vEarly, [vMid, vEarly]);
  await p.evaluate(() => { const b = document.querySelector('#stack section.pane [data-viewmenu]'); b && b.click(); }); await wait(200);
  await p.evaluate(() => { const b = document.querySelector('.viewmenu [data-vm="chart"]') || document.querySelector('.viewmenu [data-vm]'); b && b.click(); }); await wait(600);

  // a view you choose stops following; Follow brings it back
  await p.click('#playStrip [data-pplay]'); await wait(400);
  await p.evaluate(() => { const b = document.getElementById('b-fit'); b && b.click(); }); await wait(400);
  s = await st();
  check('choosing a view stops following', /Follow playhead/.test(s.follow), s);
  const sc2 = s.scrub; await wait(700);
  check('playhead keeps going while held', (await st()).scrub > sc2);
  await p.click('#playStrip [data-pfollow]'); await wait(300);
  check('Follow resumes', /Following/.test((await st()).follow));

  // the end stops playback
  await p.evaluate(() => { const r = document.querySelector('#playStrip [data-pscrub]'); r.value = '998'; r.dispatchEvent(new Event('input', { bubbles: true })); }); await wait(1500);
  check('stops at the end', (await until(() => document.querySelector('#playStrip [data-pplay]').getAttribute('aria-label') === 'Play', null, 4000)));

  // A–B loop: the playhead stays between the cursors
  check('A–B loop is off until both cursors are placed', await p.evaluate(() => { window.__tlmPlay.cursors(null, null); return document.querySelector('#playStrip [data-ploop]').disabled; }));
  await p.evaluate(() => { window.__tlmPlay.cursors(600, 620); window.__tlmPlay.speed(10); });
  await p.click('#playStrip [data-ploop]'); await wait(200);
  await p.click('#playStrip [data-pplay]'); await wait(3500);
  const lp = await p.evaluate(() => window.__tlmPlay.state());
  check('looping keeps the playhead inside A–B while it plays', lp.loop && lp.running && lp.t >= 600 && lp.t <= 620, lp);
  await p.click('#playStrip [data-pplay]'); await p.click('#playStrip [data-ploop]'); await wait(200);
  // Stop at: pause on the next event of a chosen category
  const ev = await p.evaluate(() => { const r = document.querySelector('#playStrip [data-pstop]').getBoundingClientRect(); return [r.left + 5, r.top + 5]; });
  await p.mouse.click(ev[0], ev[1]); await wait(200);
  check('Stop at offers any event and each category', await p.evaluate(() => /Any event/.test(document.querySelector('.catmenu').innerText) && /Alarm/.test(document.querySelector('.catmenu').innerText)));
  await pick(/^Any event$/); await wait(200);
  await p.evaluate(() => { window.__tlmPlay.seek(0); window.__tlmPlay.speed(600); });
  await p.click('#playStrip [data-pplay]');
  const stopped = await until(() => { const s = window.__tlmPlay.state(); return !s.running && /Stopped at/.test(s.note) ? s : null; }, null, 15000);
  check('playback pauses at the first event and says which', !!stopped && /Stopped at/.test(await p.evaluate(() => document.querySelector('#playStrip [data-pnote]').textContent)), stopped);
  await p.evaluate(() => { window.__tlmPlay.cursors(null, null); });
  await p.mouse.click(ev[0], ev[1]); await wait(200); await pick(/^Never$/); await wait(200);

  await p.click('#playStrip [data-pclose]'); await wait(300);
  check('close hides the transport', !(await st()).shown);

  // each tab keeps its own playhead: leave and come back, it's there (paused)
  await p.click('#b-play'); await wait(200);
  await p.evaluate(() => window.__tlmPlay.seek(1234)); await wait(200);
  await p.click('#wsTabNew'); await wait(250);
  await p.evaluate(() => document.querySelector('#addMenu [data-new]').click()); await wait(300);
  await p.fill('#wsName', 'Second'); await p.click('#wsNameOk'); await wait(800);
  const opened = (await p.locator('#wsTabs .wstab').count()) === 2;
  check('another tab opens without the playhead', opened && !(await st()).shown);
  await p.locator('#wsTabs .wstab').nth(0).click(); await wait(600);
  const back = await p.evaluate(() => window.__tlmPlay.state());
  check('coming back restores that tab\'s playhead, paused', (await st()).shown && back && Math.abs(back.t - 1234) < 1 && !back.running, back);
});
