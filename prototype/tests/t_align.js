// Time alignment between files: Align starts, Same clock, Time channel (GPS time of week), Cross-correlation, Offset.
const T = require('./lib');
T.run('align', async ({ p, check, text, wait }) => {
  await p.dblclick('.fitem[data-file="gpslog"] .fname'); await wait(2500);
  // the GPS logger's tab: it is file 1, the flight log (open from the start) is aligned to it
  const sel = '#openFiles [data-falign="main"]';
  const opts = await p.evaluate(s => { const e = document.querySelector(s); return e ? [...e.options].map(o => o.textContent + (o.disabled ? ' (off)' : '')) : null; }, sel);
  check('alignment offers Time channel and Cross-correlation', !!opts && opts.some(o => /^Time channel \(gps_tow\)$/.test(o)) && opts.some(o => /Cross-correlation/.test(o)), opts);
  await p.selectOption(sel, 'clock'); await wait(800);
  const clockOpt = await p.evaluate(s => document.querySelector(s).selectedOptions[0].textContent, sel);
  await p.selectOption(sel, 'timesrc'); await wait(1200);
  const note = await text('#openFiles .anote');
  check('Time channel lines them up from gps_tow, the same offset as the recording clock', /gps_tow: −3\.33 min|gps_tow: −3:20|gps_tow: −200/.test(note || '') && /−/.test(clockOpt), [note, clockOpt]);
  await p.selectOption(sel, 'xcorr'); await wait(2500);
  const xn = await text('#openFiles .anote');
  check('Cross-correlation names the channel it matched, the offset and r', /Matched on \w+: [+−].+, r = [01]\.\d\d/.test(xn || ''), xn);
});
