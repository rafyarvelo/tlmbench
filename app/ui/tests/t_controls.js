// The top bar's Lock, the keyboard (arrows pan, + − ↑ ↓ zoom, T theme, L lock, Ctrl+. focus), the ⋯ menu's groups
// (Remove every channel / pane with Undo), Clear cursors always on the bar, and Focus mode.
import { run } from './lib.js';

run('controls', async ({ p, check, text, visible, plotAt, settingsLS, until, wait, paneCount }) => {
  const range = () => p.evaluate(() => [document.getElementById('rangeTxt').textContent, document.getElementById('spanTxt').textContent].join('|'));
  const wheel = async () => { const c = await plotAt(0); await p.mouse.move(c.x, c.y); await p.mouse.wheel(0, -300); await wait(300); };
  const more = async () => { await p.click('#b-more'); };

  // zoom in first so there is room to pan
  await p.keyboard.press('ArrowUp');
  await wait(500);
  const zoomed = await range();
  check('↑ zooms in', !/09:59|3\.33 min visible/.test(zoomed.split('|')[1]) && zoomed !== '', zoomed);
  await p.keyboard.press('ArrowRight');
  await wait(300);
  check('→ pans', (await range()) !== zoomed, await range());
  const panned = await range();
  await p.keyboard.press('ArrowLeft');
  await wait(300);
  check('← pans back', (await range()) !== panned);
  await p.keyboard.press('ArrowDown');
  await wait(500);
  await p.keyboard.press('+');
  await wait(500);
  const afterPlus = await range();
  await p.keyboard.press('-');
  await wait(500);
  check('+ and − zoom', (await range()) !== afterPlus);

  // Lock
  check('Lock off at first', (await p.getAttribute('#b-lock', 'aria-pressed')) === 'false');
  await p.keyboard.press('l');
  check('L locks', (await p.getAttribute('#b-lock', 'aria-pressed')) === 'true' && (await settingsLS()).locked === true);
  const before = await range();
  await wheel();
  check('a locked wheel does nothing and says so', (await range()) === before && /locked/.test(await text('#wsNote')) && /Unlock/.test(await text('#wsNote')), [await range(), await text('#wsNote')]);
  await p.keyboard.press('ArrowRight');
  await p.keyboard.press('f');
  await wait(400);
  check('keys and Fit are refused too', (await range()) === before, await range());
  const c = await plotAt(0, 0.4);
  await p.click('#m-cursor');
  await p.mouse.click(c.x, c.y);
  await wait(300);
  check('cursors still work while locked', await p.evaluate(() => !document.getElementById('b-clear').disabled));
  await p.click('#wsNote button:has-text("Unlock")');
  check('Unlock in the note unlocks', (await p.getAttribute('#b-lock', 'aria-pressed')) === 'false');
  await p.click('#b-lock');
  await p.click('#b-clear');
  check('Clear cursors stays on the bar, disabled with none', await p.evaluate(() => document.getElementById('b-clear').disabled));
  await p.click('#b-lock');

  // theme
  const theme = () => p.evaluate(() => document.documentElement.dataset.theme);
  await p.keyboard.press('t');
  const t1 = await theme();
  await p.keyboard.press('t');
  check('T switches the theme and back', !!t1 && t1 !== (await theme()), [t1, await theme()]);

  // the ⋯ menu's groups and the bulk removals
  await more();
  const heads = await p.$$eval('.moremenu .mgrp', (e) => e.map((x) => x.textContent));
  check('⋯ has View, All panes and Window groups', heads.join() === 'View,All panes,Window', heads);
  await p.click('[data-more="nochan"]');
  check('Remove every channel empties the panes, keeping them', (await paneCount()) === 3 && (await p.$$eval('#stack .chip', (e) => e.length)) === 0);
  await p.click('#wsNote button:has-text("Undo")');
  check('Undo puts the channels back', (await p.$$eval('#stack .chip', (e) => e.length)) === 6);
  await more();
  await p.click('[data-more="nopanes"]');
  check('Remove every pane', (await paneCount()) === 0);
  await p.click('#wsNote button:has-text("Undo")');
  check('Undo brings the panes back', (await paneCount()) === 3);

  // focus mode
  await p.keyboard.press('ControlOrMeta+.');
  await wait(200);
  check('Ctrl+. hides the rail, sidebar and tabs; Exit focus shows', !(await visible('#side')) && !(await visible('.arail')) && !(await visible('#wsTabs')) && (await visible('#focusExit')) && (await visible('#rangeTxt')));
  await p.keyboard.press('Escape');
  await wait(200);
  check('Esc leaves focus mode', (await visible('#side')) && (await visible('.arail')) && !(await visible('#focusExit')));
  await more();
  await p.click('[data-more="focus"]');
  check('⋯ → Focus mode', await visible('#focusExit'));
  await p.click('#focusExit');
  check('Exit focus', await visible('.arail'));

  // lock is kept after a reload
  await p.keyboard.press('l');
  await p.reload();
  await wait(800);
  check('Lock comes back after a reload', (await p.getAttribute('#b-lock', 'aria-pressed')) === 'true');
});
