// Table filters and the charts: the filter strip, Whole file, step through matches, Apply to charts (chip, stats),
// channel missing values, and Ignore outliers.
const T = require('./lib');
T.run('filters', async ({ p, check, text, pick, expandTree, wait, settingsLS }) => {
  const strip = () => text('#stack .tfilt');
  check('no strip without a filter', await p.evaluate(() => document.querySelector('#stack .tfilt').hidden));
  await p.fill('#pf-0-accel_x', '> 1'); await wait(1200);
  check('strip appears with a match count', /\d[\d,]* matches/.test(await strip()), await strip());
  await p.click('#stack [data-fscope="file"]'); await wait(1500);
  check('Whole file scope', (await p.getAttribute('#stack [data-fscope="file"]', 'aria-pressed')) === 'true');
  check('status says rows in the file', /rows in the file/.test(await text('#stack .tstat')), await text('#stack .tstat'));
  await p.click('#stack [data-fstep="1"]'); await wait(600);
  check('step: 1 of N', /^.*1 of [\d,]+/.test(await strip()), await strip());
  check('cursor A lands on the match', (await text('#cA')) !== '—');
  await p.click('#stack [data-fstep="1"]'); await wait(500);
  check('step: 2 of N', /2 of [\d,]+/.test(await strip()), await strip());

  await p.check('#stack [data-fapply]'); await wait(1800);
  const chips = await p.evaluate(() => [...document.querySelectorAll('.pmask')].map(e => e.textContent));
  check('a chip on every chart', chips.length === 3 && chips.every(c => /Filtered · [\d,]+ rows hidden/.test(c)), chips);
  check('In view is off while applied', await p.isDisabled('#stack [data-fscope="view"]'));
  await p.evaluate(() => document.querySelector('[data-rail="measure"]').click()); await wait(300);
  check('stats say what is excluded', /Excluding[\s\S]*rows \(table filter\)/.test(await text('#stats')), await text('#stats'));
  await p.evaluate(() => document.querySelector('[data-rail="measure"]').click()); await wait(200);
  await p.evaluate(() => document.querySelector('.pmask button').click()); await wait(1500);
  check('chip × shows the rows again', (await p.evaluate(() => document.querySelectorAll('.pmask').length)) === 0);
  check('...and turns Apply off', !(await p.isChecked('#stack [data-fapply]')));

  // Save as rule: the filter becomes a rule row in Event rules, opened in the editor
  await p.click('#stack [data-fsave]'); await wait(700);
  const rr = await p.evaluate(() => { const r = document.querySelector('.chkdlg .ckrow.focus'); return r && { name: r.querySelector('[data-kf="name"]').value, mark: r.classList.contains('mk'), expr: (r.querySelector('[data-xt="expect"]') || {}).value || r.querySelector('.ckx').innerText }; });
  check('Save as rule opens the editor at a new rule for the filter', !!rr && rr.mark && /accel_x/.test(rr.name), rr);
  const S1 = await settingsLS();
  check('...saved in Event rules with the filter as its condition', (S1.checkSets || []).some(s => s.id === 'cs_rules' && s.checks.some(k => /^accel_x > 1$/.test(k.expect))), (S1.checkSets || []).find(s => s.id === 'cs_rules').checks.slice(-1));
  await p.evaluate(() => document.querySelector('.chkdlg [data-x]').click()); await wait(300);

  await expandTree();
  await p.click('.chan[data-ch="accel_x"] .nm', { button: 'right' }); await wait(150);
  await pick(/^Missing values/); await wait(300);
  await p.fill('.cheditor [data-dm="hi"]', '1'); await p.dispatchEvent('.cheditor [data-dm="hi"]', 'change'); await wait(2500);
  check('missing values saved', ((await settingsLS()).chdisp || {}).accel_x && (await settingsLS()).chdisp.accel_x.missing.hi === 1);
  check('editor counts them', /[\d,]+ values are missing/.test(await text('.cheditor')));
  await p.mouse.click(1000, 880); await expandTree();
  check('Explorer badge', /⌀/.test(await text('.chan[data-ch="accel_x"]')), await text('.chan[data-ch="accel_x"]'));
  // Missing when: a condition on another channel
  await p.click('.chan[data-ch="accel_y"] .nm', { button: 'right' }); await wait(150);
  await pick(/^Missing values/); await wait(300);
  check('Missing when field in Missing values', await p.evaluate(() => !!document.querySelector('.cheditor [data-dm="when"]')));
  await p.fill('.cheditor [data-dm="when"]', 'throttle_pct > 50'); await p.dispatchEvent('.cheditor [data-dm="when"]', 'change'); await wait(2500);
  check('Missing when saved and applied', ((await settingsLS()).chdisp.accel_y.missing || {}).when === 'throttle_pct > 50' && /[\d,]+ values are missing/.test(await text('.cheditor')), await text('.cheditor'));
  await p.fill('.cheditor [data-dm="when"]', 'nope > 1'); await p.dispatchEvent('.cheditor [data-dm="when"]', 'change'); await wait(2500);
  check('an unknown name says so', /No channel called "nope"/.test(await text('.cheditor')), await text('.cheditor'));
  await p.fill('.cheditor [data-dm="when"]', ''); await p.dispatchEvent('.cheditor [data-dm="when"]', 'change'); await wait(1200);
  await p.mouse.click(1000, 880); await expandTree();
  await p.fill('#pf-0-accel_x', ''); await wait(1000);

  // Ignore outliers
  await p.evaluate(() => document.querySelector('#stack section.pane [data-axis]').click()); await wait(250);
  await p.selectOption('.axpop [data-ax="u0.range"]', 'robust'); await wait(600);
  check('Ignore outliers kept with the pane', await p.evaluate(() => !!document.querySelector('.axpop [data-ax="u0.range"] option[value="robust"]:checked')));
}, { settings: { fileOpenNew: 'starterboth', fileOpen: 'starterboth' }, wait: 6000 });
