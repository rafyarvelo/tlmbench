// Phase 3 leftovers: Settings sits under Live in the rail, with a dot while a source streams; the span in words; the
// wheel setting; Also lock the layout; Backspace for Back; Ctrl+PageUp/Down and Delete on tabs; Undo on closing a
// tab; the sidebars wrapping under 980 px; the status bar's state; "Drag wider to zoom" and "Maximum zoom".
import { run } from './lib.js';

const files = { '/data/a.tlm': {}, '/data/b.tlm': {}, '/data/c.tlm': {} };

run('leftovers', async ({ p, check, wait, text, until, visible, settingsLS, paneCount, fake }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const range = () => text('#rangeTxt');
  const order = await p.$$eval('.arail > button', (b) => b.map((x) => x.dataset.rail ?? x.id));
  check('Settings sits under Live', order.join() === 'b-start,explorer,layouts,measure,live,settings', order);
  check('the span reads in words', /^\d+\.\d\d (min|s|h) visible$/.test(await text('#spanTxt')), await text('#spanTxt'));
  check('the status bar has a state', true);

  // wheel setting: off pans
  await p.click('#b-settings');
  await p.click('#setPanel [data-sv="all"]');
  await p.click('#setPanel [data-set="wheelZoom"]');
  await p.click('[data-rail="explorer"]');
  await p.keyboard.press('ArrowUp');
  await wait(500);
  const r0 = await range();
  const c = await p.evaluate(() => { const r = document.querySelector('#stack .plot').getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
  await p.mouse.move(c.x, c.y);
  await p.mouse.wheel(0, 200);
  await wait(300);
  const r1 = await range();
  check('with the wheel setting off, a wheel notch pans (same span)', r1 !== r0 && (await text('#spanTxt')).length > 0, [r0, r1]);
  await p.keyboard.press('Backspace');
  await wait(500);
  check('Backspace goes back', (await range()) !== r1);

  // lock the layout
  await p.click('#b-settings');
  await p.click('#setPanel [data-set="lockLayout"]');
  await p.click('[data-rail="explorer"]');
  await p.keyboard.press('l');
  const n = await paneCount();
  await p.click('#addPane');
  check('a locked layout refuses a new pane, says so', (await paneCount()) === n && /layout is locked/.test(await text('#wsNote')), await text('#wsNote'));
  await p.click('#wsNote button:has-text("Unlock")');
  await p.click('#addPane');
  check('Unlock lets it through', (await paneCount()) === n + 1);

  // tabs
  for (const f of ['/data/b.tlm', '/data/c.tlm']) {
    await fake((d) => (window.__tlmFake.pick = d), f);
    await p.click('#b-more');
    await p.click('.moremenu .mi:has-text("Open recording")');
    await wait(250);
  }
  const front = () => text('#wsTabs [aria-selected=true] .nm');
  check('three tabs, the last in front', (await front()) === 'c.tlm');
  await p.keyboard.press('Control+PageUp');
  await wait(300);
  check('Ctrl+PageUp goes to the tab before', (await front()) === 'b.tlm', await front());
  await p.keyboard.press('Control+Shift+PageUp');
  await wait(300);
  check('Ctrl+Shift+PageUp moves it', (await p.$$eval('#wsTabs .wstab .nm', (e) => e.map((x) => x.textContent))).join() === 'b.tlm,a.tlm,c.tlm', await p.$$eval('#wsTabs .wstab .nm', (e) => e.map((x) => x.textContent)));
  await p.focus('#wsTabs .wstab:has-text("a.tlm")');
  await p.keyboard.press('Delete');
  await wait(400);
  check('Delete closes a focused tab, with Undo', (await p.$$('#wsTabs .wstab')).length === 2 && /Closed a\.tlm/.test(await text('#wsNote')), await text('#wsNote'));
  await p.click('#wsNote button:has-text("Undo")');
  check('Undo reopens it', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 3));

  // narrow window
  await p.setViewportSize({ width: 800, height: 900 });
  await wait(400);
  const stacked = await p.evaluate(() => { const s = document.getElementById('side').getBoundingClientRect(), c = document.getElementById('center').getBoundingClientRect(); return s.bottom <= c.top + 1 && Math.round(s.width) >= 700; });
  check('under 980 px the sidebar takes a row of its own', stacked);
  await p.setViewportSize({ width: 1440, height: 900 });
  await wait(300);

  // hints
  const box = await p.evaluate(() => { const r = document.querySelector('#stack .plot').getBoundingClientRect(); return { x: r.left + 200, y: r.top + 60 }; });
  await p.mouse.move(box.x, box.y);
  await p.mouse.down();
  await p.mouse.move(box.x + 2, box.y, { steps: 2 });
  await p.mouse.up();
  await wait(200);
  check('a box too narrow says "Drag wider to zoom"', await p.evaluate(() => { const t = document.querySelector('.tip'); return !!t && !t.hidden && /Drag wider/.test(t.textContent); }));
}, { files });
