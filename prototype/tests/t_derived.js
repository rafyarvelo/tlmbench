// Derived channels: the expression engine, Derive… from two channels, recipes in an import profile, remove + undo, reload.
const T = require('./lib');
T.run('derived', async ({ p, check, drop, csv, expandTree, menu, pick, wait, until }) => {
  const ev = await p.evaluate(() => {
    const D = window.__tlmDerived, S = a => ({ a: Float32Array.from(a), r: 1 });
    const run = (src, m) => { try { const r = D.evalExpr(D.parseExpr(src), n => m[n]); return r.a ? [...r.a].map(v => Number.isNaN(v) ? 'NaN' : +v.toFixed(4)).join(',') : +(+r).toFixed(4); } catch (e) { return 'ERR ' + e.msg + ' @' + e.at; } };
    const x = S([10, 11, NaN, 12.1, 11]);
    return { delta: run('delta(x)', { x }), pct: run('pct_change(x) * 100', { x }), sma: run('sma(x, 2)', { x }), mean: run('mean(x)', { x }), max2: run('max(x, 11)', { x }),
      quoted: run('"Adj Close" / 2', { 'Adj Close': S([4, 6]) }), unicode: run('x × 2 − 1', { x: S([1, 2]) }), errEnd: run('x * (2 +', { x }), errFn: run('foo(x)', { x }) };
  });
  check('delta skips gaps', ev.delta === 'NaN,1,NaN,1.1,-1.1', ev.delta);
  check('pct_change', ev.pct === 'NaN,10,NaN,10,-9.0909', ev.pct);
  check('sma over samples', ev.sma === 'NaN,10.5,NaN,11.55,11.55', ev.sma);
  check('whole-file mean', ev.mean === 11.025, ev.mean);
  check('max element by element', ev.max2 === '11,11,NaN,12.1,11', ev.max2);
  check('quoted names', ev.quoted === '2,3', ev.quoted);
  check('× and − accepted', ev.unicode === '1,3', ev.unicode);
  check('error names the spot', /ends too soon @8/.test(ev.errEnd), ev.errEnd);
  check('unknown function', /no function called foo/.test(ev.errFn), ev.errFn);

  // filters, interp, spectrum
  const fl = await p.evaluate(() => {
    const D = window.__tlmDerived, run = (src, m) => { try { return D.evalExpr(D.parseExpr(src), n => m[n]); } catch (e) { return { err: e.msg }; } };
    const N = 400, r = 100, sig = (f1, f2) => ({ a: Float32Array.from({ length: N }, (_, i) => Math.sin(2 * Math.PI * f1 * i / r) + Math.sin(2 * Math.PI * f2 * i / r)), r });
    const amp = a => { let m = 0; for (let i = 100; i < 300; i++) m = Math.max(m, Math.abs(a[i])); return m; };
    const x = sig(1, 30), lo = run('lowpass(x, 5)', { x }), hi = run('highpass(x, 10)', { x }), bp = run('bandpass(x, 10, 40)', { x });
    const med = run('median(x, 3)', { x: { a: Float32Array.from([1, 9, 2, 3, 100, 4]), r: 1 } });
    const fast = { a: Float32Array.from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), r: 10 }, slow = { a: Float32Array.from([0, 10]), r: 1 };
    const held = run('slow + fast', { slow, fast }), lin = run('interp(slow) + fast', { slow, fast });
    const err = run('lowpass(x, 80)', { x });
    return { lo: amp(lo.a), hi: amp(hi.a), bp: amp(bp.a), med: [...med.a].map(v => Number.isNaN(v) ? 'NaN' : v).join(','), held: held.a[5], lin: +lin.a[5].toFixed(3), err: err.err };
  });
  check('lowpass keeps the slow wave and removes the fast one', fl.lo > 0.8 && fl.lo < 1.2, fl);
  check('highpass and bandpass keep the fast wave only', fl.hi > 0.8 && fl.hi < 1.2 && fl.bp > 0.7 && fl.bp < 1.2, fl);
  check('median over 3 samples', fl.med === 'NaN,NaN,2,3,3,4', fl.med);
  check('interp() resamples a slow channel linearly (plain names hold it)', fl.held === 0 && fl.lin === 5, fl);
  check('a cutoff above half the rate is refused with a reason', /below half the sample rate/.test(fl.err || ''), fl.err);

  await expandTree();
  await p.click('.chan[data-ch="battery_v"] .nm'); await p.click('.chan[data-ch="current_a"] .nm', { modifiers: ['ControlOrMeta'] }); await wait(200);
  const bar = await p.evaluate(() => document.getElementById('exSelBar').innerText);
  check('selection bar offers Derive…', /Derive/.test(bar), bar);
  await p.evaluate(() => [...document.querySelectorAll('#exSelBar button')].find(b => b.textContent === 'Derive…').click()); await wait(300);
  check('dialog opens with A/B templates', (await p.evaluate(() => [...document.querySelectorAll('#recDlg [data-r="tpl"] option')].map(o => o.textContent).join('|'))) === 'A − B|A ÷ B|A × B|A + B');
  await p.selectOption('#recDlg [data-r="tpl"]', 'prod'); await wait(150);
  check('product expression', (await p.inputValue('#recDlg [data-r="expr"]')) === 'battery_v * current_a');
  await p.fill('#recDlg [data-r="expr"]', 'battery_v * (current_a'); await wait(100);
  check('live syntax check', /Expected "\)"/.test(await p.textContent('#recStat')));
  await p.fill('#recDlg [data-r="expr"]', 'battery_v * current_a'); await wait(100);
  check('preview stats', /samples/.test(await p.textContent('#recStat')));
  await p.click('#recDlg [data-ok]'); await wait(800);
  check('added to a pane', (await p.evaluate(() => document.querySelector('#stack').innerText)).includes('battery_v × current_a'));
  check('ƒ row in Explorer', await p.evaluate(() => [...document.querySelectorAll('.chan')].some(c => c.querySelector('.dglyph') && /battery_v × current_a/.test(c.textContent))));

  // recipes from an import profile, a whole-file number, remove + undo
  await drop([csv('AAPL.csv')]);
  const offered = await p.evaluate(() => [...document.querySelectorAll('.impder label')].map(l => l.textContent.trim()));
  check('import offers stock recipes', offered.length >= 6 && offered.some(x => /Daily return/.test(x)), offered);
  await p.evaluate(() => ['r_ret', 'r_vol20', 'r_annvol'].forEach(id => { const x = document.querySelector(`[data-irec="${id}"]`); x.checked = true; x.dispatchEvent(new Event('change')); }));
  await p.click('#liveDlg [data-go]'); await wait(2000);
  const tree = await p.evaluate(() => [...document.querySelectorAll('.chan .nm')].map(e => e.textContent).join(' | '));
  check('picked recipes on the file', /Daily return/.test(tree) && /Volatility 20d/.test(tree), tree);
  check('one-number recipe shows its value', /Annualised volatility = \d/.test(tree), tree);
  await p.click('.chan[data-ch="Volatility 20d"] .nm', { button: 'right' }); await wait(150);
  await pick(/^Remove from/);
  check('removed from the profile', !(await p.$('.chan[data-ch="Volatility 20d"]')));
  await p.evaluate(() => { const b = [...document.querySelectorAll('#wsNote button')].find(b => /undo/i.test(b.textContent)); b && b.click(); }); await wait(400);
  check('undo brings it back', !!(await p.$('.chan[data-ch="Volatility 20d"]')));
  await wait(2200);
  await p.reload();
  check('recipes persist', await until(() => window.__tlmDerived && window.__tlmDerived.recipes().length >= 8, null, 25000));
  check('file-only recipe persists', await until(() => document.querySelectorAll('.dglyph').length >= 1, null, 25000));
  // Spectrum: a pane view with the frequency content over the time view
  await p.evaluate(() => document.querySelector('#stack section.pane [data-viewmenu]').click()); await wait(200);
  check('Spectrum is a pane view', await p.evaluate(() => !!document.querySelector('.viewmenu [data-vm="spectrum"]')));
  await p.evaluate(() => document.querySelector('.viewmenu [data-vm="spectrum"]').click()); await wait(1200);
  const si = await until(() => { const e = document.querySelector('#stack .specinfo'); return e && /peak/.test(e.textContent) ? e.textContent : null; }, null, 8000);
  check('it draws and names each channel\'s peak', !!si && await p.evaluate(() => document.querySelector('#stack .specplot canvas').width > 0), si);
  const pk = await p.evaluate(() => window.__tlmDerived.spectrum('accel_x', 0, 600));
  check('spectrum of a channel finds a peak below Nyquist', pk && pk.n >= 1024 && pk.peak > 0, pk);
});
