// A pane can have a name of its own (owner, Nov 15; ledger D-ui-43): Rename pane… in its ⋯ menu or a double-click on its
// number; the name shows before the chips (none by default), is cut at 60 characters, an empty name takes it off, Escape
// leaves it as it was, ⌘Z undoes it; Duplicate names the copy "… copy"; a layout saved from the tab keeps it (an optional
// `name` per pane) and the overview's picker uses it.
import { run } from './lib.js';

run('panenames', async ({ p, check, until, text, settingsLS }) => {
  const H = (i) => `#stack [data-pane="${i}"] .phead`;
  const names = () => p.$$eval('#stack [data-pname]', (e) => e.map((x) => x.textContent));
  check('no pane has a name to start with', (await names()).length === 0);

  // the ⋯ menu
  await p.hover(H(0));
  await p.click(`${H(0)} [data-pmenu]`);
  await p.click('.menu [data-pm="rename"]');
  check('Rename pane… opens a field in the header, with the name to type', await until(() => document.activeElement?.matches('[data-pname-input]')));
  await p.keyboard.type('Power rail');
  await p.keyboard.press('Enter');
  check('Enter gives the pane its name, shown before the chips', await until(() => document.querySelector('#stack [data-pane="0"] [data-pname]')?.textContent === 'Power rail'), await names());
  check('and the field is gone', !(await p.$('[data-pname-input]')));

  // a double-click on its number, and Escape
  await p.dblclick(`${H(0)} .pnum`);
  check('a double-click on the number opens the field with the current name', (await p.inputValue('[data-pname-input]')) === 'Power rail');
  await p.fill('[data-pname-input]', 'Something else');
  await p.keyboard.press('Escape');
  check('Escape leaves the name as it was', (await names())[0] === 'Power rail' && !(await p.$('[data-pname-input]')), await names());

  // a double-click on the name
  await p.dblclick(`${H(0)} [data-pname]`);
  await p.fill('[data-pname-input]', 'x'.repeat(80));
  await p.keyboard.press('Enter');
  check('a name is cut at 60 characters', (await names())[0].length === 60, (await names())[0]?.length);
  await p.dblclick(`${H(0)} [data-pname]`);
  await p.fill('[data-pname-input]', 'Power rail');
  await p.keyboard.press('Enter');

  // Duplicate, the overview's picker, and the saved layout
  await p.hover(H(0));
  await p.click(`${H(0)} [data-pdup]`);
  check('Duplicate names the copy "… copy"', await until(() => document.querySelectorAll('#stack [data-pname]').length === 2) && (await names()).join() === 'Power rail,Power rail copy', await names());
  await p.click(`${H(0)} .pnum`);
  check('the overview\'s picker says the pane by its name', await until(() => /Power rail ·/.test(document.querySelector('[data-ovch] option')?.textContent ?? '')), await p.$eval('[data-ovch] option', (o) => o.textContent));
  await p.click('#wsTabs .wstab[aria-selected=true] .tm');
  await p.click('[data-ws="saveas"]');
  await p.fill('#wsName', 'Named');
  await p.press('#wsName', 'Enter');
  await until(async () => true);
  const ws = await until(() => true).then(async () => (await settingsLS()).workspaces?.find((w) => w.name === 'Named'));
  check('a layout saved from the tab keeps the names', ws?.panes?.[0]?.name === 'Power rail' && ws?.panes?.[1]?.name === 'Power rail copy' && ws?.panes?.[2]?.name === undefined, ws?.panes?.map((x) => x.name));

  // empty takes the name off, and ⌘Z puts it back
  await p.dblclick(`${H(1)} [data-pname]`);
  await p.fill('[data-pname-input]', '   ');
  await p.keyboard.press('Enter');
  check('an empty name takes it off', await until(() => document.querySelectorAll('#stack [data-pname]').length === 1));
  await p.keyboard.press('Control+z');
  await p.keyboard.press('Meta+z');
  check('and undo brings it back', await until(() => document.querySelectorAll('#stack [data-pname]').length === 2), await names());
});
