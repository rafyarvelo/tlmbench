// Opening never waits for the whole file: rows show at once, columns fill in, charts say "Summarizing…", rules say "checking".
const T = require('./lib');
T.run('loading', async ({ p, check, wait }) => {
  const probe = () => p.evaluate(() => ({
    rows: document.querySelectorAll('#stack .trw:not([hidden])').length, pend: document.querySelectorAll('#stack .pendcell').length,
    stat: (document.querySelector('.tstat') || {}).textContent || '', bar: !!document.getElementById('loadBar') && !document.getElementById('loadBar').hidden,
    ov: [...document.querySelectorAll('.pload')].map(e => e.textContent), ev: (document.querySelector('.evsum') || {}).textContent || '' }));
  // a table of every channel in the second file
  const t0 = Date.now();
  await p.dblclick('.fitem[data-file="0415"] .fname');
  let first = null, mid = null;
  for (let i = 0; i < 200; i++) { const s = await probe(); if (s.rows && first == null) { first = Date.now() - t0; mid = s; } if (first != null && !s.bar) break; await wait(50); }
  check('rows show in under 1.5 s', first != null && first < 1500, first);
  check('cells still loading show …', mid && mid.pend > 0, mid && mid.pend);
  check('status says what is being read', mid && /reading \d+ of \d+ column/.test(mid.stat), mid && mid.stat);
  await wait(300);
  const done = await probe();
  check('all columns filled', done.pend === 0 && /rows in view/.test(done.stat), done);
  await wait(600);
  check('rules ran once their data was in', !/checking rules/.test((await probe()).ev), (await probe()).ev);
}, { settings: { fileOpenNew: 'table', fileOpen: 'remember' }, wait: 9000 });
