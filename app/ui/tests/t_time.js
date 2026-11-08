// Time format: the menu on the time range (Automatic, clock, seconds, milliseconds, minutes and seconds, hours,
// days, wall clock) changing every readout, Time decimals, Time labels (every chart), all saved. Wall clock only for a
// file with an absolute start; a file with no time reads in sample numbers.
import { run } from './lib.js';

run('time', async ({ p, check, text, settingsLS, until, wait, visible, fake }) => {
  const range = () => text('#rangeTxt');
  check('Automatic: a clock', /^\d\d:\d\d:\d\d\.\d → \d\d:\d\d:\d\d\.\d$/.test(await range()), await range());
  await p.click('#rangeTxt');
  check('clicking the range opens the formats', (await p.$$('.timemenu [data-tf]')).length === 10);
  await p.click('.timemenu [data-tf="s"]');
  check('Seconds counts from the start', await until(() => /^0\.\d+ s → \d+\.\d+ s$/.test(document.getElementById('rangeTxt').textContent)), await range());
  check('and it is saved', (await settingsLS()).timeFormat === 's');
  await p.click('#rangeTxt');
  await p.click('.timemenu [data-tf="ms"]');
  check('Milliseconds', await until(() => / ms → \d+ ms$/.test(document.getElementById('rangeTxt').textContent)), await range());
  await p.click('#rangeTxt');
  await p.click('.timemenu [data-tf="minsec"]');
  check('Minutes and seconds', await until(() => /^0:\d\d\.\d → \d+:\d\d\.\d$/.test(document.getElementById('rangeTxt').textContent)), await range());
  await p.click('#rangeTxt');
  await p.click('.timemenu [data-tf="h"]');
  check('Hours', await until(() => / h → [\d.]+ h$/.test(document.getElementById('rangeTxt').textContent)), await range());

  // decimals from Settings
  await p.click('#rangeTxt');
  await p.click('.timemenu [data-tf="s"]');
  await p.click('#b-settings');
  await p.click('#setPanel [data-sv="all"]');
  await p.selectOption('[data-set="timeDecimals"]', '1');
  check('Time decimals', await until(() => /^0\.0 s → \d+\.\d s$/.test(document.getElementById('rangeTxt').textContent)), await range());
  await p.selectOption('[data-set="timeDecimals"]', 'auto');

  // the cursors' readout follows too
  await p.click('#m-cursor');
  const box = await p.evaluate(() => { const r = document.querySelector('#stack [data-pane="0"] .plot').getBoundingClientRect(); return { x: r.left + 200, y: r.top + 40 }; });
  await p.mouse.click(box.x, box.y);
  await p.mouse.click(box.x + 200, box.y);
  await p.click('[data-rail="measure"]').catch(() => {});
  check('cursor tiles read in the chosen format', await until(() => /\d+\.\d+ s/.test(document.querySelector('#measure .mt .v')?.textContent ?? '')));

  // Time labels
  const labelled = () => p.evaluate(() => [...document.querySelectorAll('#stack .plot')].filter((e) => e.style.getPropertyValue('--pb') !== '4px').length);
  check('time labels on the bottom chart only', (await labelled()) === 1, await labelled());
  await p.selectOption('[data-set="timeAxis"]', 'every');
  check('Every chart labels its time axis', await until(() => document.querySelectorAll('#stack .plot').length === 3 && [...document.querySelectorAll('#stack .plot')].every((e) => e.style.getPropertyValue('--pb') !== '4px')));
  await p.reload();
  await wait(1000);
  check('format and labels come back after a reload', /s → /.test(await range()) && (await settingsLS()).timeAxis === 'every');

  // Wall clock: the time of day for a file with an absolute start
  await p.click('#rangeTxt');
  check('the menu offers Wall clock', (await p.$$('.timemenu [data-tf]')).length === 10 && !(await p.isDisabled('.timemenu [data-tf="wall"]')));
  await p.click('.timemenu [data-tf="wall"]');
  check('Wall clock reads the time of day', await until(() => /^\d\d:\d\d:\d\d(\.\d+)? → \d\d:\d\d:\d\d(\.\d+)?$/.test(document.getElementById('rangeTxt').textContent)), await range());
  // a recording that starts at 0 has no wall clock: it reads as Automatic, and the menu says why
  await fake((f) => window.__tlmFake.drop([f]), '/data/rel.tlm');
  await until(() => document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip === '/data/rel.tlm');
  check('a relative recording falls back to Automatic', await until(() => /^\d+\.\d+ s → \d+\.\d+ s$/.test(document.getElementById('rangeTxt').textContent)), await range());
  await p.click('#rangeTxt');
  check('...and Wall clock is greyed with the reason', (await p.isDisabled('.timemenu [data-tf="wall"]')) && /needs a file with absolute time/.test(await text('.timemenu [data-tf="wall"]')), await text('.timemenu [data-tf="wall"]'));
  await p.keyboard.press('Escape');

  // Samples: a file with no time reads in sample numbers, whatever the format
  await fake((f) => window.__tlmFake.drop([f]), '/data/rows.csv');
  await until(() => document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip === '/data/rows.csv');
  check('a file with no time reads in sample numbers', await until(() => /^Sample 0 → [\d,]+$/.test(document.getElementById('rangeTxt').textContent)), await range());
  check('...its span in samples', /^[\d,]+ samples visible$/.test(await text('#spanTxt')), await text('#spanTxt'));
  await p.click('#rangeTxt');
  check('...and the menu says so', /This file has no time: sample numbers/i.test(await text('.timemenu')), await text('.timemenu'));
  await p.keyboard.press('Escape');
}, { files: { '/data/flight.tlm': {}, '/data/rel.tlm': { relative: true }, '/data/rows.csv': { time: 'index', samples: 5000 } }, launch: '/data/flight.tlm' });
