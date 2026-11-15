// Rules and checks says what each kind does (sheet E1, "hard to understand"): the panel's empty state and a new set's
// editor have one plain line for Check and one for Rule; each row says what its kind does and follows a change of kind;
// When, Grace, Settle and the severity carry tips.
import { run } from './lib.js';

run('checkcopy', async ({ p, check, text, until }) => {
  await p.click('[data-rail="checks"]');
  await until(() => !!document.querySelector('#checksBody [data-cempty]'));
  const empty = await text('#checksBody [data-cempty]');
  check('the empty panel says what a set, a check and a rule are', /A set holds checks and rules/.test(empty) && /Check: passes or fails/.test(empty) && /Rule: marks events/.test(empty), empty);

  await p.click('#checksBody [data-cnew]');
  await until(() => !!document.querySelector('#recDlg'));
  const intro = await text('#recDlg [data-kintro]').catch(() => '');
  check('a new set with no rows says the same in the editor', /Check: passes or fails/.test(intro) && /Rule: marks events/.test(intro), intro);
  check('the add buttons carry the same lines as tips', /passes or fails/.test((await p.getAttribute('#recDlg [data-kadd]', 'data-tip')) ?? '') && /marks events/.test((await p.getAttribute('#recDlg [data-kaddr]', 'data-tip')) ?? ''));

  await p.click('#recDlg [data-kadd]');
  check('the intro goes once there is a row', await until(() => !document.querySelector('#recDlg [data-kintro]')));
  check('a check row says what a check does', /passes or fails/.test(await text('#recDlg [data-k="0"] [data-kdesc]')), await text('#recDlg [data-k="0"] [data-kdesc]'));
  await p.selectOption('#recDlg [data-k="0"] [data-kf="kind"]', 'rule');
  check('and follows a change to Rule', await until(() => /marks events/.test(document.querySelector('#recDlg [data-k="0"] [data-kdesc]')?.textContent ?? '')));
  const tips = await p.evaluate(() => ['[data-kwhen]', '.lvg'].map((s) => document.querySelector(`#recDlg [data-k="0"] ${s}`)?.getAttribute('data-tip') ?? ''));
  check('+ When and Grace say what they do', /Only look while/.test(tips[0]) && /How long it may break/.test(tips[1]), tips);
  await p.click('#recDlg [data-kadd]');
  await p.waitForTimeout(300);
  await p.screenshot({ path: new URL('out/checkcopy.png', import.meta.url).pathname });
  await p.keyboard.press('Escape');
});
