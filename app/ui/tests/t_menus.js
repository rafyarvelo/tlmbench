// Every menu is the one Menu component: it sits under its button (above when there's no room), the first item takes
// focus, ↑ ↓ Home End move, Esc closes and gives focus back to the button, a press outside closes it, and a
// right-click menu stays inside the window.
import { run } from './lib.js';

run('menus', async ({ p, check, until, wait }) => {
  const focused = () => p.evaluate(() => document.activeElement?.textContent?.trim() ?? '');
  const box = (sel) => p.evaluate((s) => { const r = [...document.querySelectorAll(s)].pop()?.getBoundingClientRect(); return r && { x: r.x, y: r.y, w: r.width, h: r.height, b: r.bottom, r: r.right }; }, sel);

  // the ⋯ menu: under its button, inside the window, the first item focused
  await p.click('#b-more');
  await wait(250);
  const m = await box('.moremenu');
  const b = await box('#b-more');
  check('the ⋯ menu sits under its button', m && Math.abs(m.y - (b.b + 4)) < 1.5 && m.r <= 1440 - 15, [m, b]);
  check('the first item takes focus', /^Open recording/.test(await focused()), await focused());
  await p.keyboard.press('ArrowDown');
  await p.keyboard.press('ArrowDown');
  await p.keyboard.press('ArrowDown');
  check('↓ skips disabled items', /^Overview above the panes/.test(await focused()), await focused());
  await p.keyboard.press('End');
  check('End goes to the last', (await focused()) === 'Settings…', await focused());
  await p.keyboard.press('ArrowDown');
  check('↓ wraps to the first', /^Open recording/.test(await focused()), await focused());
  await p.keyboard.press('Home');
  await p.keyboard.press('ArrowUp');
  check('↑ wraps to the last', (await focused()) === 'Settings…', await focused());
  await p.keyboard.press('Escape');
  check('Esc closes it', await until(() => !document.querySelector('.moremenu')));
  check('and gives focus back to its button', await p.evaluate(() => document.activeElement?.id === 'b-more'));

  // Explorer's menus close on a press outside (they didn't before)
  await p.click('#exMore');
  check('Explorer ⋯ opens', await until(() => !!document.querySelector('.exmenu')));
  await p.mouse.click(900, 300);
  check('a press outside closes it', await until(() => !document.querySelector('.exmenu')));
  await p.click('#exFolder');
  await p.keyboard.press('Escape');
  check('Esc closes the folder menu', await until(() => !document.querySelector('.exmenu')));

  // Add pane's ▾ near the bottom (enough panes that the stack scrolls, Add pane at its end): its menu opens above
  for (let k = 0; k < 4; k++) await p.click('#addPane');
  await p.evaluate(() => document.getElementById('stack').scrollTo(0, 1e6));
  await wait(100);
  await p.click('#addPaneKinds');
  await wait(250);
  const k = await box('.addkinds');
  const kb = await box('#addPaneKinds');
  check('a menu with no room below opens above its button', k && Math.abs(k.b - (kb.y - 4)) < 1.5, [k, kb]);
  await p.keyboard.press('Escape');

  // a right-click by the window's corner stays inside it
  const plot = await box('#stack section.pane .phead');
  await p.mouse.click(plot.r - 4, plot.y + 10, { button: 'right' });
  await wait(250);
  const pm = await box('.panemenu');
  check('a right-click menu stays inside the window', pm && pm.r <= 1440 - 8 && pm.b <= 900 - 8 && pm.x >= 8, pm);
  await p.keyboard.press('Escape');

  // the view menu: the current view checked, and focused
  await p.locator('#stack section.pane .phead').first().hover();
  await p.click('#stack [data-vmenu]');
  check('the view menu focuses the checked view', await until(() => document.activeElement?.getAttribute('aria-checked') === 'true' && document.activeElement?.dataset.vm === 'chart'));
  check('with a check mark', await p.evaluate(() => !!document.querySelector('.viewmenu [aria-checked="true"] .ck svg')));
  await p.keyboard.press('ArrowDown');
  await p.keyboard.press('Enter');
  check('Enter picks the focused item', await until(() => !!document.querySelector('#stack [data-pane="0"] [data-table]')));
});
