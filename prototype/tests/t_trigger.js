// Live triggers: arm on a channel crossing a level; the view freezes on the window around it, an event marks it.
const T = require('./lib');
T.run('trigger', async ({ p, check, text, wait, until }) => {
  await p.click('#railLive'); await wait(300);
  await p.click('#liveDlg [data-go]'); await wait(2500);
  check('Trigger… in the live strip', /Trigger…/.test(await text('#lvTrig')));
  await p.click('#lvTrig'); await wait(200);
  const opts = await p.evaluate(() => [...document.querySelectorAll('.trigpop [data-tg="ch"] option')].map(o => o.value));
  check('it lists the live channels', opts.length > 3, opts);
  const ch = opts.find(o => /^accel_x/.test(o)) || opts[0];
  await p.selectOption('.trigpop [data-tg="ch"]', ch);
  await p.selectOption('.trigpop [data-tg="kind"]', 'rise');
  await p.fill('.trigpop [data-tg="level"]', '0'); await p.dispatchEvent('.trigpop [data-tg="level"]', 'change');
  await p.fill('.trigpop [data-tg="pre"]', '1'); await p.dispatchEvent('.trigpop [data-tg="pre"]', 'change');
  await p.fill('.trigpop [data-tg="post"]', '1'); await p.dispatchEvent('.trigpop [data-tg="post"]', 'change');
  await p.click('.trigpop [data-tgarm]'); await wait(200);
  check('armed', /Armed|Triggered/.test(await text('#lvTrig')), await text('#lvTrig'));
  const fired = await until(() => /Triggered · 1×/.test(document.getElementById('lvTrig').textContent), null, 20000);
  check('a single trigger fires once and freezes the view', fired && /Jump to live/.test(await text('#lvFollow')), await text('#lvTrig'));
  await wait(500); const span = await text('#spanTxt');
  check('the view is the window around it (1 s before, 1 s after)', /^2(\.0+)? s visible|^2\.00 s/.test(span), span);
  await wait(1500);
  check('it stays frozen (single)', /Triggered · 1×/.test(await text('#lvTrig')) && (await text('#spanTxt')) === span);
  await p.evaluate(() => document.querySelector('[data-rail="events"]').click()); await wait(400);
  check('an event in the Trigger category marks it', /Trigger: .* rises through 0/.test(await text('#evList')), (await text('#evList')).slice(0, 200));
  await p.click('#lvFollow'); await wait(500);
  check('Follow returns to live', /Following/.test(await text('#lvFollow')));
});
