// ⌘Z and ⇧⌘Z undo and redo the last change (owner, Nov 14; sheet B1): a layout change (a channel added, a pane closed) comes
// back; the change a note offers to undo goes first; a text field keeps its own ⌘Z; Backspace still goes back a view.
import { run } from './lib.js';

run('undo', async ({ p, check, until }) => {
  const panes = () => p.evaluate(() => window.__tlmApp.panes());
  const start = JSON.stringify(await panes());
  await p.hover('#stack [data-pane="0"] .chip[data-ch="0"]');
  await p.click('#stack [data-pane="0"] .chip[data-ch="0"] .cx');
  check('a channel taken out of a pane', await until(() => !window.__tlmApp.panes()[0].includes(0)));
  await p.mouse.click(700, 880);
  await p.keyboard.press('ControlOrMeta+z');
  check('⌘Z puts it back', await until((s) => JSON.stringify(window.__tlmApp.panes()) === s, start), await panes());
  await p.keyboard.press('ControlOrMeta+Shift+z');
  check('⇧⌘Z takes it out again', await until(() => !window.__tlmApp.panes()[0].includes(0)), await panes());
  await p.keyboard.press('ControlOrMeta+z');
  await until((s) => JSON.stringify(window.__tlmApp.panes()) === s, start);

  // several steps back
  const n = (await panes()).length;
  await p.hover('#stack [data-pane="2"] .phead');
  await p.click('#stack [data-pane="2"] [data-pmenu]');
  await p.locator('.menu .mi', { hasText: /^Close pane$/ }).click();
  check('a pane closed', await until((k) => window.__tlmApp.panes().length === k - 1, n));
  await p.mouse.click(700, 880);
  await p.keyboard.press('ControlOrMeta+z');
  check('⌘Z brings the pane back with its channels', await until((s) => JSON.stringify(window.__tlmApp.panes()) === s, start), await panes());

  // the toast's Undo is the same step as ⌘Z: ⇧⌘Z then closes the pane again (X4, one Undo everywhere)
  await p.hover('#stack [data-pane="2"] .phead');
  await p.click('#stack [data-pane="2"] [data-pmenu]');
  await p.locator('.menu .mi', { hasText: /^Close pane$/ }).click();
  await until((k) => window.__tlmApp.panes().length === k - 1, n);
  await p.click('#wsNote button.tbtn');
  check('the toast\'s Undo brings it back', await until((s) => JSON.stringify(window.__tlmApp.panes()) === s, start), await panes());
  await p.mouse.click(700, 880);
  await p.keyboard.press('ControlOrMeta+Shift+z');
  check('and ⇧⌘Z closes it again, as after ⌘Z', await until((k) => window.__tlmApp.panes().length === k - 1, n), await panes());
  await p.keyboard.press('ControlOrMeta+z');
  await until((s) => JSON.stringify(window.__tlmApp.panes()) === s, start);

  // a text field keeps its own undo
  await p.fill('#explorer input[type=search]', 'acc');
  const before = JSON.stringify(await panes());
  await p.focus('#explorer input[type=search]');
  await p.keyboard.press('ControlOrMeta+z');
  check('⌘Z in a text field is the field\'s', JSON.stringify(await panes()) === before);
  await p.fill('#explorer input[type=search]', '');
  await p.mouse.click(700, 880);
  await p.keyboard.press('ControlOrMeta+z');
  await p.keyboard.press('ControlOrMeta+z');
  await p.keyboard.press('ControlOrMeta+z');
  check('with nothing left, it says so', await until(() => /Nothing to undo/.test(document.querySelector('#wsNote .txt')?.textContent ?? '')));
});
