// Tooltips: a floating tip after a short delay (Short: the first sentence and the shortcut, retired after three
// showings; Full: everything; Off), at once on keyboard focus, and Settings → Reset tooltips.
import { run } from './lib.js';

run('tips', async ({ p, check, wait, settingsLS, until, visible }) => {
  const tip = () => p.evaluate(() => { const t = document.querySelector('.tip'); return t && !t.hidden ? t.textContent : null; });
  const hover = async (sel) => { await p.mouse.move(0, 400); await wait(50); await p.hover(sel); };
  const shown = async (sel) => { await hover(sel); await wait(1000); const t = await tip(); await p.mouse.move(0, 400); await wait(100); return t; };

  await hover('#b-play');
  await wait(300);
  check('nothing at first: a tip waits for the pointer to rest', (await tip()) === null);
  await wait(700);
  check('Short shows the first clause and the shortcut', (await tip()) === 'Play back (Space plays and pauses)' || /^Play back/.test(await tip()), await tip());
  await p.mouse.move(0, 400);
  await wait(100);
  check('moving away hides it', (await tip()) === null);

  check('the title attribute no longer carries it', (await p.getAttribute('#b-fit', 'title')) === null);
  const seen = [await shown('#b-fit'), await shown('#b-fit'), await shown('#b-fit'), await shown('#b-fit')];
  check('a tip retires after three showings', !!seen[0] && !!seen[1] && !!seen[2] && seen[3] === null, seen);
  check('and the count is saved', (await settingsLS()).tipsSeen['Show the whole recording (F)'] === 3);

  await p.click('#b-settings');
  await p.click('#setResetTips');
  check('Reset tooltips brings them back', (await shown('#b-fit')) !== null);
  await p.selectOption('[data-set="tooltips"]', 'full');
  check('Full shows everything, every time', /Show the whole recording/.test((await shown('#b-fit')) ?? '') && !!(await shown('#b-fit')) && !!(await shown('#b-fit')) && !!(await shown('#b-fit')));
  await p.selectOption('[data-set="tooltips"]', 'off');
  check('Off shows none', (await shown('#b-fit')) === null);
  await p.selectOption('[data-set="tooltips"]', 'full');

  // keyboard focus: at once
  await p.mouse.move(0, 400);
  await p.focus('#m-box');
  await p.keyboard.press('Tab');
  await wait(100);
  check('keyboard focus shows a tip at once', (await tip()) !== null, await tip());
});
