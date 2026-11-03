// The start page: what shows with no file (open a folder or recording, connect a live source, recent recordings),
// the rail's logo opening it over the tabs (Esc or opening something closes it), recent recordings kept newest
// first, and "Show this page at launch" (on: it is over the restored tabs at launch; off: it isn't).
import { run } from './lib.js';

const files = { '/data/a.tlm': {}, '/data/b.tlm': {}, '/data/c.tlm': {} };

run('start', async ({ p, check, text, visible, fake, until, settingsLS, wait }) => {
  check('no file: the start page', (await p.isVisible('#startPage >> visible=true')) && /Open folder/.test(await text('#startPage')) && /None yet/.test(await text('#startPage')));
  await p.click('[data-start="live"] >> visible=true');
  check('Connect live source… opens the dialog', await visible('#lvConnect'));
  await p.keyboard.press('Escape');

  await fake((d) => (window.__tlmFake.pick = d), '/data/a.tlm');
  await p.click('[data-start="open"] >> visible=true');
  check('Open recording… opens a file and the page goes', await until(() => document.querySelectorAll('#wsTabs .wstab').length === 1) && !(await p.isVisible('#startPage >> visible=true')));
  for (const f of ['/data/b.tlm', '/data/c.tlm']) {
    await fake((d) => (window.__tlmFake.pick = d), f);
    await p.click('#b-start');
    await p.click('[data-start="open"] >> visible=true');
    await wait(300);
  }
  await until(() => document.querySelectorAll('#wsTabs .wstab').length === 3);
  check('recent recordings, newest first', (await settingsLS()).recent.join() === '/data/c.tlm,/data/b.tlm,/data/a.tlm', (await settingsLS()).recent);

  await p.click('#b-start');
  check('the logo opens the page over the tabs', (await p.isVisible('#startPage >> visible=true')) && (await p.$$eval('.start.overlay [data-recent]', (e) => e.map((x) => x.dataset.recent))).join() === '/data/c.tlm,/data/b.tlm,/data/a.tlm');
  await p.keyboard.press('Escape');
  check('Esc closes it', !(await p.isVisible('#startPage >> visible=true')));
  await p.click('#b-start');
  await p.click('[data-recent="/data/a.tlm"] >> visible=true');
  check('a recent recording opens its tab and closes the page', !(await p.isVisible('#startPage >> visible=true')) && (await text('#wsTabs [aria-selected=true] .nm')) === 'a.tlm');

  // at launch
  await p.reload();
  await wait(1200);
  check('at launch the page is over the restored tabs', (await p.isVisible('#startPage >> visible=true')) && (await p.$$('#wsTabs .wstab')).length === 3);
  await p.click('#startPage [data-set="showStart"] >> visible=true');
  check('the switch turns it off, saved', (await settingsLS()).showStart === false);
  await p.reload();
  await wait(1200);
  check('then it stays closed at launch', !(await p.isVisible('#startPage >> visible=true')));
}, { files, launch: null });
