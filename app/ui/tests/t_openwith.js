// Open with layout ▸ on a file's right-click in Explorer (D-ui-42): your layouts and the folder's shared ones; the file
// is shown, then the layout opens on it in a tab of its own; a layout the open file can't fill is greyed with what it needs.
import { run } from './lib.js';

const DIR = '/data/work';
const A = `${DIR}/flight.tlm`;
const B = `${DIR}/second.tlm`;
const files = { [A]: {}, [B]: {} };
const shared = { [DIR]: { 'needy.tlmview.json': { schema: 1, name: 'Needy', by: 'a.chen', panes: [{ channels: ['volts'] }], files: [{ path: 'flight.tlm', slot: 1 }] } } };

run(
  'openwith',
  async ({ p, check, fake, until, text }) => {
    await fake(([d, v]) => { window.__tlmFake.pickFolder = d; Object.assign(window.__tlmFake.views, v); }, [DIR, shared]);
    await p.click('#exFolder');
    await until(() => document.querySelectorAll('#explorer .row.file').length >= 2);
    await p.dblclick(`#explorer .row.file[data-path="${A}"]`);
    await until(() => document.querySelectorAll('#wsTabs .wstab').length === 1);
    await p.click('[data-rail="layouts"]');
    await p.click('#lyNew');
    await p.fill('#lyName', 'Scratch');
    await p.press('#lyName', 'Enter');
    await until(() => !!document.querySelector('[data-lopen="Scratch"]'));

    await p.click('[data-rail="explorer"]');
    await p.click(`#explorer .row.file[data-path="${B}"]`, { button: 'right' });
    check("a file's menu has Open with layout ▸", await until(() => !!document.querySelector('.filemenu [data-fm="withlayout"]')));
    await p.click('.filemenu [data-fm="withlayout"]');
    check("it lists your layouts and the folder's", await until(() => !!document.querySelector('.filemenu [data-fmlayout="Scratch"]') && !!document.querySelector('.filemenu [data-fmlayout="needy.tlmview.json"]')), await text('.filemenu'));
    check("Back returns to the file's menu", (await p.$$('.filemenu [data-fm="layouts-back"]')).length === 1);
    await p.click('.filemenu [data-fmlayout="Scratch"]');
    check(
      'the layout opens on that file in a tab of its own',
      await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'Scratch' && /second\.tlm/.test(document.querySelector('#wsTabs [aria-selected=true]')?.dataset.tip ?? '')),
      await p.$$eval('#wsTabs .wstab', (t) => t.map((x) => x.textContent + x.dataset.tip)),
    );

    // a layout whose channel the open file lacks is greyed, with what it needs
    await p.click(`#explorer .row.file[data-path="${B}"]`, { button: 'right' });
    await p.click('.filemenu [data-fm="withlayout"]');
    check(
      'a layout the open file cannot fill is greyed with what it needs',
      await until(() => document.querySelector('.filemenu [data-fmlayout="needy.tlmview.json"]')?.disabled === true && /needs volts/.test(document.querySelector('.filemenu [data-fmlayout="needy.tlmview.json"]')?.textContent ?? '')),
      await text('.filemenu'),
    );
    await p.keyboard.press('Escape');
  },
  { files, launch: A },
);
