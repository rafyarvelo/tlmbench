// A recording opened from outside the folder Explorer shows (Open…, a drop), or a live source such as a file playback,
// is listed with its channels, above the folder's own files; opening one of the folder's files lists it in the folder as before.
import { run } from './lib.js';

const files = { '/data/a/flight.tlm': {}, '/other/x.tlm': {} };

run('outside', async ({ p, check, text, fake, until }) => {
  await fake((d) => (window.__tlmFake.pickFolder = d), '/data/a');
  await p.click('#exFolder');
  await until(() => document.querySelector('#explorer .row.file'));
  check('the folder lists its file', (await p.$$eval('#explorer .tree .row.file', (r) => r.map((x) => x.dataset.path))).join() === '/data/a/flight.tlm');
  await fake(() => (window.__tlmFake.pick = '/other/x.tlm'));
  await p.keyboard.press('ControlOrMeta+o');
  check('the recording opens in a tab', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'x.tlm'));
  check('and is listed in Explorer, with its channels', await until(() => document.querySelector('#explorer .tree .row.file[data-path="/other/x.tlm"]') && document.querySelectorAll('#explorer .tree .row.chan').length === 6), await p.$$eval('#explorer .tree .row', (r) => r.map((x) => x.className.split(' ').slice(0, 2).join('.'))));
  check('above the folder\'s files', (await p.$$eval('#explorer .tree .row.file', (r) => r.map((x) => x.dataset.path))).join() === '/other/x.tlm,/data/a/flight.tlm');
  await p.dblclick('#explorer .tree .row.file[data-path="/data/a/flight.tlm"]');
  check('a file of the folder lists once, in the folder', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'flight.tlm') && (await p.$$eval('#explorer .tree .row.file', (r) => r.map((x) => x.dataset.path))).join() === '/data/a/flight.tlm');

  // a live source is a file of its own in Explorer, with its channels, while a folder is showing
  await p.click('[data-rail="live"]');
  await until(() => !!document.getElementById('liveDlg'));
  await p.fill('#lvRecording', '/other/x.tlm');
  await p.click('#lvConnect');
  await until(() => /playback/.test(document.querySelector('#wsTabs [aria-selected=true]')?.textContent ?? ''));
  await p.click('[data-rail="explorer"]');
  check('a live playback is listed with its channels while a folder is showing', await until(() => /playback/.test(document.querySelector('#explorer .tree .row.file')?.textContent ?? '') && document.querySelectorAll('#explorer .tree .row.chan').length === 6), await p.$$eval('#explorer .tree .row', (r) => r.map((x) => x.className.split(' ').slice(0, 2).join('.'))));
}, { files, launch: null });
