// The top bar's all-panes switch: Charts, Tables or Both sets every pane's view at once (also in the ⋯ menu on a
// narrow bar); the pressed one shows when the panes share a view; a layout lock disables it.
import { run } from './lib.js';

run('allmodes', async ({ p, check, until, wait }) => {
  const modes = () => p.evaluate(() => window.__tlmApp.modes());
  await p.setViewportSize({ width: 1500, height: 900 });
  await wait(300);

  check('every pane is a chart to start with', (await modes()).every((m) => m === 'chart'), await modes());
  check('Charts shows as pressed', (await p.getAttribute('[data-allmode="chart"]', 'aria-pressed')) === 'true');
  await p.click('[data-allmode="table"]');
  check('Tables makes every pane a table', await until(() => window.__tlmApp.modes().every((m) => m === 'table')), await modes());
  check('and shows as pressed', (await p.getAttribute('[data-allmode="table"]', 'aria-pressed')) === 'true' && (await p.getAttribute('[data-allmode="chart"]', 'aria-pressed')) === 'false');
  await p.click('[data-allmode="both"]');
  check('Both makes every pane a chart with its table', await until(() => window.__tlmApp.modes().every((m) => m === 'both')), await modes());

  // one pane on its own: no button is pressed
  await p.hover('#stack [data-pane="0"] .phead');
  await p.click('#stack [data-pane="0"] [data-vmenu]');
  await p.click('.viewmenu [data-vm="chart"]');
  check('panes that differ press none', await until(() => ['chart', 'table', 'both'].every((m) => document.querySelector(`[data-allmode="${m}"]`).getAttribute('aria-pressed') === 'false')), await modes());
  await p.click('[data-allmode="chart"]');
  check('Charts puts them all back', await until(() => window.__tlmApp.modes().every((m) => m === 'chart')), await modes());

  // a narrow bar has it in ⋯
  await p.setViewportSize({ width: 1000, height: 900 });
  await wait(300);
  check('a narrow bar hides the group', !(await p.isVisible('[data-allmode="table"]')));
  await p.click('#b-more');
  await p.click('.menu [data-allmode="table"]');
  check('⋯ has the same choices', await until(() => window.__tlmApp.modes().every((m) => m === 'table')), await modes());
});
