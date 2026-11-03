// Time format: the menu on the time range (Automatic, clock, seconds, milliseconds, minutes and seconds, hours,
// days) changing every readout, Time decimals, Time labels (every chart), all saved.
import { run } from './lib.js';

run('time', async ({ p, check, text, settingsLS, until, wait, visible }) => {
  const range = () => text('#rangeTxt');
  check('Automatic: a clock', /^\d\d:\d\d:\d\d\.\d → \d\d:\d\d:\d\d\.\d$/.test(await range()), await range());
  await p.click('#rangeTxt');
  check('clicking the range opens the formats', (await p.$$('.timemenu [data-tf]')).length === 9);
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
}, {});
