// Gauges at scale: 100+ widgets in two Gauges panes, redrawn as during playback. Each frame must stay well inside a
// 60 fps frame; values that don't change, and cards that are off screen, cost (almost) nothing.
const T = require('./lib');
T.run('perf_gauges', async ({ p, check, wait, until, expandTree }) => {
  await expandTree();
  // every channel of the flight in one pane, as gauges, then a copy of that pane: 100+ cards
  const ids = await p.evaluate(() => [...document.querySelectorAll('.chan[data-ch]')].map(e => e.dataset.ch).filter(id => !/[·@]/.test(id)));
  await p.click(`.chan[data-ch="${ids[0]}"] .nm`);
  for (const id of ids.slice(1)) await p.click(`.chan[data-ch="${id}"] .nm`, { modifiers: ['ControlOrMeta'] });
  await p.evaluate(() => [...document.querySelectorAll('#exSelBar button')].find(b => /^Chart$/.test(b.textContent)).click()); await wait(1500);
  const idx = await p.evaluate(() => [...document.querySelectorAll('#stack section.pane')].findIndex(sec => sec.querySelector('[data-chip="status_bits"]')));
  await p.evaluate(i => document.querySelectorAll('#stack section.pane [data-viewmenu]')[i].click(), idx); await wait(200);
  await p.evaluate(() => document.querySelector('.viewmenu [data-vm="gauge"]').click()); await wait(800);
  await p.evaluate(i => document.querySelectorAll('#stack section.pane [data-dup]')[i].click(), idx); await wait(800);
  await until(() => document.querySelectorAll('#stack .gw[data-gw]').length >= 100 && !document.querySelector('#stack .gw .gpend'), null, 30000);
  const cards = await p.evaluate(() => document.querySelectorAll('#stack .gw[data-gw]').length);
  check('100+ widget cards', cards >= 100, cards);
  // all of them on screen for the timing: a tall window
  await p.setViewportSize({ width: 1600, height: 4200 }); await wait(1500);
  const r = await p.evaluate(() => {
    const G = window.__tlmGauges, t0 = G.now(), vis = [...document.querySelectorAll('#stack .gw[data-gw]')].filter(g => !g.hidden).length;
    const time = f => { const T = []; for (let i = 0; i < 40; i++) { const a = performance.now(); f(i); T.push(performance.now() - a); } T.sort((x, y) => x - y); return { med: +T[20].toFixed(2), p90: +T[36].toFixed(2) }; };
    G.draw(t0 - 100); G.reset();
    const moving = time(i => G.draw(t0 - 100 + i * 0.4));            // values change every frame (playback)
    const st1 = G.stats(); G.reset();
    const still = time(() => G.draw(t0 - 100 + 39 * 0.4));            // nothing changes (paused)
    const st2 = G.stats();
    return { vis, moving, still, st1, st2 };
  });
  console.log('   ', JSON.stringify(r));
  check('a moving frame of 100+ widgets fits a 60 fps frame (16 ms; about 7–12 ms on this 2-core sandbox)', r.moving.med < 16, r.moving);
  check('nothing is rebuilt while values move', r.st1.built === 0, r.st1);
  check('values are patched in place', r.st1.patches > 0 && r.st1.writes > 0);
  check('a still frame costs almost nothing', r.still.med < r.moving.med / 2 && r.st2.writes === 0, [r.still, r.st2]);
  // off screen: a normal window, scrolled to the top; cards below aren't drawn until they scroll in
  await p.setViewportSize({ width: 1440, height: 900 }); await wait(1000);
  const off = await p.evaluate(() => { const G = window.__tlmGauges, t0 = G.now(); G.reset(); G.draw(t0 - 50); const s = G.stats(); return { drawn: s.patches + s.built + s.skips, cards: [...document.querySelectorAll('#stack .gw[data-gw]')].filter(g => !g.hidden).length }; });
  check('off-screen cards are skipped', off.drawn < off.cards * 0.6, off);
  const last = await p.evaluate(() => { const g = [...document.querySelectorAll('#stack .gw[data-gw]')].filter(x => !x.hidden).pop(); g.scrollIntoView(); return g.dataset.gw; }); await wait(600);
  check('a card scrolled into view is drawn', await p.evaluate(id => { const g = [...document.querySelectorAll(`#stack .gw[data-gw="${id}"]`)].pop(); return !!g.querySelector('svg'); }, last));
}, { wait: 2500 });
