// A code widget from a shared folder: a value card with "Run?" until you trust the folder; the prompt names the widget,
// who shared it and the folder, shows the code, and the answer is kept per folder.
import { run } from './lib.js';

const LEVEL = {
  id: 'sw_levelbar',
  name: 'Level bar (code)',
  shared: '.tlmbench/widgets/level-bar',
  by: 'j.kim',
  code: `{
  build(el, info) { el.innerHTML = '<b class="t"></b><i class="n"></i>'; el.querySelector('.n').textContent = info.name; },
  update(el, x) { el.querySelector('.t').textContent = x.txt; }
}`,
};

run(
  'widgetshared',
  async ({ p, check, until, wait, text, settingsLS }) => {
    await p.focus('#explorer .row.chan[data-ch="0"]');
    await p.keyboard.press('ControlOrMeta+a');
    await p.click('#selBar [data-sel="chart"]');
    const idx = (await p.locator('#stack section.pane').count()) - 1;
    await p.click(`#stack [data-pane="${idx}"] [data-vmenu]`);
    await p.click('.menu [data-vm="gauges"]');
    await until(() => document.querySelectorAll('#stack [data-gw] svg').length >= 3);
    const card = (n) => `#stack [data-gw="${n}"]`;
    await p.hover(card('current_a'));
    await p.click(`${card('current_a')} [data-gwt]`);
    check('a shared widget says where it came from', /Shared in \.tlmbench\/widgets\/level-bar/.test(await p.$eval('.widgetmenu', (e) => e.textContent)));
    await p.click('.widgetmenu [data-wt="sw_levelbar"]');
    check('it waits for your OK and shows the value meanwhile', await until(() => !!document.querySelector('#stack [data-gw="current_a"] [data-trustw]') && /A/.test(document.querySelector('#stack [data-gw="current_a"]').innerText) && /Run\?/.test(document.querySelector('#stack [data-gw="current_a"]').innerText)), await text(card('current_a')));
    check('no frame yet', !(await p.$(`${card('current_a')} iframe`)));
    await p.click(`${card('current_a')} [data-trustw]`);
    check('the prompt names the widget, its sharer and the folder', await until(() => { const t = document.getElementById('trustDlg')?.innerText ?? ''; return /Level bar \(code\)/.test(t) && /j\.kim/.test(t) && /\.tlmbench\/widgets\/level-bar/.test(t); }));
    check('and shows the code', await p.$eval('#trustDlg pre', (e) => /build\(el, info\)/.test(e.textContent)));
    await p.click('#trustDlg [data-x]');
    check('keeping value cards leaves it a card', !!(await p.$(`${card('current_a')} [data-trustw]`)) && !((await settingsLS()).trustedWidgetSources ?? []).length);
    await p.click(`${card('current_a')} [data-trustw]`);
    await p.click('#trustDlg [data-trust]');
    check('trusting runs it in a sandboxed frame', await until(() => document.querySelector('#stack [data-gw="current_a"] iframe.gcodefr')?.getAttribute('sandbox') === 'allow-scripts' || null, null, 8000));
    check('the folder is remembered', ((await settingsLS()).trustedWidgetSources ?? []).includes('.tlmbench/widgets/level-bar'), (await settingsLS()).trustedWidgetSources);
  },
  {
    files: { '/data/flight.tlm': { channels: ['battery_v', 'current_a', 'throttle_pct'], units: ['V', 'A', '%'] } },
    launch: '/data/flight.tlm',
    settings: { customWidgets: [LEVEL], trustedWidgetSources: [] },
  },
);
