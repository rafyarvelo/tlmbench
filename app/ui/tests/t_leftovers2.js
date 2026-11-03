// More Phase 3 leftovers: the view button's icon, the values pane's "where" line, three panes at most in a row, A and
// B over a table row, the transport in the main column.
import { run } from './lib.js';

run('leftovers2', async ({ p, check, text, wait, until, paneCount }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  const menu = async (i, item) => {
    await p.hover(`#stack [data-pane="${i}"] .phead`);
    await p.click(`#stack [data-pane="${i}"] [data-pmenu]`);
    await p.locator('.panemenu .mi', { hasText: item }).click();
  };
  const view = async (i, mode) => {
    await p.hover(`#stack [data-pane="${i}"] .phead`);
    await p.click(`#stack [data-pane="${i}"] [data-vmenu]`);
    await p.click(`.viewmenu [data-vm="${mode}"]`);
  };

  const before = await p.$eval('#stack [data-pane="0"] [data-vmenu]', (e) => e.innerHTML);
  await view(0, 'values');
  const after = await p.$eval('#stack [data-pane="0"] [data-vmenu]', (e) => e.innerHTML);
  check('the view button shows the current view\'s icon', before !== after && /svg/.test(after), [before.length, after.length]);
  check('the values pane says where it reads', /at the end of the view · /.test(await text('#stack [data-pane="0"] [data-where-line]')), await text('#stack [data-pane="0"] [data-where-line]'));
  const c = await p.evaluate(() => { const r = document.querySelector('#stack [data-pane="1"] .plot').getBoundingClientRect(); return { x: r.left + 200, y: r.top + 50 }; });
  await p.mouse.move(c.x, c.y);
  await p.keyboard.press('a');
  await p.mouse.move(20, 400);
  await wait(400);
  check('…and at cursor A once it is placed', /at cursor A · /.test(await text('#stack [data-pane="0"] [data-where-line]')), await text('#stack [data-pane="0"] [data-where-line]'));
  check('the values pane takes only the room it needs', await p.evaluate(() => document.querySelector('#stack [data-pane="0"]').getBoundingClientRect().height < document.querySelector('#stack [data-pane="1"]').getBoundingClientRect().height));
  await view(0, 'chart');

  // at most three in a row
  await menu(0, /^Split right$/);
  await menu(0, /^Split right$/);
  check('two splits make three beside each other', (await p.$$('#stack .prow:first-child section.pane')).length === 3);
  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-pmenu]');
  check('and a fourth is refused', await p.locator('.panemenu .mi', { hasText: /^Split right$/ }).isDisabled());
  await p.keyboard.press('Escape');

  // A over a table row
  await view(1, 'table');
  await until(() => document.querySelectorAll('#stack [data-pane="1"] [data-row][data-t]').length > 3);
  await p.hover('#stack [data-pane="1"] [data-row="4"]');
  await p.keyboard.press('b');
  await wait(300);
  const cur = await app('cursors');
  check('B over a table row places the cursor at that row', cur.B !== null && cur.B === (await p.getAttribute('#stack [data-pane="1"] [data-row="4"]', 'data-t')), cur);

  // the transport sits in the main column
  await p.click('#b-play');
  const inCenter = await p.evaluate(() => !!document.querySelector('#center #playStrip'));
  check('the transport is inside the main column', inCenter);
});
