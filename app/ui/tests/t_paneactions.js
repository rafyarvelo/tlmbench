// A pane's actions on hover, in the prototype's order (owner decision 9): view, axis settings, Events, Duplicate, Split, ⋯,
// Close. They show on hover or selection (calm UI), each works in one click, and those that don't fit give way, rarer
// first, before chips fold (they stay in ⋯).
import { run } from './lib.js';

run('paneactions', async ({ p, check, until }) => {
  const H = (i) => `#stack [data-pane="${i}"] .phead`;
  const panes = () => p.evaluate(() => window.__tlmApp.panes());
  const order = await p.$$eval(`${H(0)} .pmore`, (b) => b.map((x) => Object.keys(x.dataset).find((k) => /^(vmenu|paxis|pevents|pdup|psplit|pmenu|pclose)$/.test(k))));
  check('the prototype\'s order', order.join() === 'vmenu,paxis,pevents,pdup,psplit,pmenu,pclose', order);
  await p.mouse.move(5, 890);
  await p.click('#stack [data-pane="1"] .pnum');
  await p.mouse.move(5, 890);
  check('hidden until the pane is hovered or selected', await until((h) => getComputedStyle(document.querySelector(`${h} [data-pdup]`)).visibility === 'hidden', H(0)));
  await p.hover(H(0));
  check('shown on hover', await until((h) => getComputedStyle(document.querySelector(`${h} [data-pdup]`)).visibility === 'visible', H(0)));

  const n = (await panes()).length;
  await p.click(`${H(0)} [data-pdup]`);
  check('Duplicate: a copy below, in one click', await until((k) => window.__tlmApp.panes().length === k + 1, n) && JSON.stringify((await panes())[1]) === JSON.stringify((await panes())[0]), await panes());
  await p.hover(H(1));
  await p.click(`${H(1)} [data-pclose]`);
  check('Close: gone, in one click', await until((k) => window.__tlmApp.panes().length === k, n));
  await p.hover(H(0));
  await p.click(`${H(0)} [data-psplit]`);
  check('Split: an empty pane beside it', await until(() => window.__tlmApp.layout()[1] === 'beside' && window.__tlmApp.panes()[1].length === 0), await p.evaluate(() => window.__tlmApp.layout()));
  await p.hover(H(0));
  await p.click(`${H(0)} [data-paxis]`);
  check('the axis button opens the pane\'s axis settings', await until(() => !!document.querySelector('.axdlg')));
  await p.keyboard.press('Escape');

  // narrow panes: four in a row drop the rarer actions, which stay in ⋯
  for (let k = 0; k < 2; k++) {
    await p.hover(H(0));
    await p.click(`${H(0)} [data-pmenu]`);
    await p.locator('.panemenu .mi', { hasText: /^Split right$/ }).click();
  }
  await until(() => window.__tlmApp.layout().filter((x) => x === 'beside').length === 3);
  await p.setViewportSize({ width: 1100, height: 800 });
  await p.waitForTimeout(300);
  await p.hover(H(0));
  const shown = await p.$$eval(`${H(0)} .pmore`, (b) => b.filter((x) => getComputedStyle(x).display !== 'none').map((x) => Object.keys(x.dataset).find((k) => /^(vmenu|paxis|pevents|pdup|psplit|pmenu|pclose)$/.test(k))));
  check('a narrow pane keeps the view button and ⋯, and drops Split and Duplicate', shown.includes('vmenu') && shown.includes('pmenu') && !shown.includes('psplit') && !shown.includes('pdup'), shown);
  await p.click(`${H(0)} [data-pmenu]`);
  check('which are still in ⋯', await until(() => /Duplicate/.test(document.querySelector('.panemenu')?.textContent ?? '') && /Split right/.test(document.querySelector('.panemenu').textContent)));
  await p.keyboard.press('Escape');
});
