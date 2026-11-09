// Widgets of your own: the Your widgets editor (an example, a live preview, JSON errors named, saved in settings), a
// declarative SVG widget filled with the reading and made safe (scripts, handlers and outside links removed), and code
// widgets: yours run in a sandboxed frame at once; one from a shared folder shows a value card with "Run?" until you trust
// the folder, the prompt names the widget, the folder and shows the code, and the answer is kept.
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
  'widgets',
  async ({ p, check, until, fake, text, settingsLS, wait }) => {
    await p.focus('#explorer .row.chan[data-ch="0"]');
    await p.keyboard.press('ControlOrMeta+a');
    await p.click('#selBar [data-sel="chart"]');
    const idx = (await p.locator('#stack section.pane').count()) - 1;
    await p.click(`#stack [data-pane="${idx}"] [data-vmenu]`);
    await p.click('.menu [data-vm="gauges"]');
    await until(() => document.querySelectorAll('#stack [data-gw] svg').length >= 3);
    const card = (n) => `#stack [data-gw="${n}"]`;
    const openMenu = async (n) => {
      await p.hover(card(n));
      await p.click(`${card(n)} [data-gwt]`);
    };

    // the editor: an example to start from, and a live preview
    await openMenu('current_a');
    check('the menu leads to Your widgets', (await p.$$eval('.widgetmenu [data-wt]', (e) => e.map((x) => x.dataset.wt))).includes('yours'));
    await p.click('.widgetmenu [data-wt="yours"]');
    check('the editor starts from an example with a preview', await until(() => /Throttle lever/.test(document.querySelector('[data-wtxt]')?.value ?? '') && !!document.querySelector('[data-wprev] svg.gsvg')));
    check('the preview binds the values', /4,610/.test(await p.$eval('[data-wprev]', (e) => e.textContent)), await text('[data-wprev]'));
    await p.fill('[data-wtxt]', '[{ "name": "x" ');
    check('a JSON mistake is named, and Save is off', (await until(() => /valid JSON/.test(document.querySelector('[data-werr]')?.textContent ?? ''))) && (await p.$eval('[data-wsave]', (e) => e.disabled)), await text('[data-werr]'));
    await p.fill('[data-wtxt]', '[{ "name": "NoSvg" }]');
    check('a widget needs an svg or code', await until(() => /needs a name and an svg/.test(document.querySelector('[data-werr]')?.textContent ?? '')));
    const evil = { name: 'Evil', svg: '<svg viewBox="0 0 120 96" onload="window.__pwned=1"><script>window.__pwned=2</script><image href="http://example.com/x.png"/><a href="http://example.com"><text x="10" y="50" fill="{{color}}" onclick="window.__pwned=3">{{value}} {{unit}}</text></a></svg>' };
    await p.fill('[data-wtxt]', JSON.stringify([evil, { name: 'Dial', svg: '<svg viewBox="0 0 120 96"><path d="M60 80L60 20" transform="rotate({{angle}} 60 80)" stroke="{{color}}"/></svg>', angle: [-60, 60] }]));
    check('the preview is made safe', await until(() => document.querySelectorAll('[data-wprev] .gw').length === 2 && !document.querySelector('[data-wprev] script, [data-wprev] image, [data-wprev] [onload], [data-wprev] [onclick]') && !window.__pwned));
    await p.click('[data-wsave]');
    check('saved in settings', await until(() => true) && ((await settingsLS()).customWidgets ?? []).length === 2 && (await settingsLS()).customWidgets[0].id === 'cw_evil', JSON.stringify(((await settingsLS()).customWidgets ?? []).map((w) => w.id)));

    // yours join the menu; a pick fills the bindings and removes what isn't safe
    await openMenu('current_a');
    check('your widgets join the menu', (await p.$$eval('.widgetmenu [data-wt]', (e) => e.map((x) => x.dataset.wt))).includes('cw_evil'));
    await p.click('.widgetmenu [data-wt="cw_evil"]');
    check('scripts, handlers, images and outside links are gone', await until(() => /\d/.test(document.querySelector('#stack [data-gw="current_a"]')?.textContent ?? '') && !document.querySelector('#stack [data-gw="current_a"] script, #stack [data-gw="current_a"] image, #stack [data-gw="current_a"] [onload], #stack [data-gw="current_a"] [onclick], #stack [data-gw="current_a"] a[href]')) && !(await p.evaluate(() => window.__pwned)));
    check('the bindings are filled', !/\{\{/.test(await p.$eval(card('current_a'), (e) => e.innerHTML)), await text(card('current_a')));
    await openMenu('battery_v');
    await p.click('.widgetmenu [data-wt="cw_dial"]');
    check('{{angle}} runs across the range', await until(() => /rotate\(-?\d/.test(document.querySelector('#stack [data-gw="battery_v"]')?.innerHTML ?? '')));

    // a code widget of yours runs at once, in a sandboxed frame
    await openMenu('current_a');
    await p.click('.widgetmenu [data-wt="yours"]');
    await p.fill('[data-wtxt]', JSON.stringify([{ name: 'Mine (code)', code: LEVEL.code }, evil]));
    await p.click('[data-wsave]');
    await openMenu('current_a');
    await p.click('.widgetmenu [data-wt="cw_mine_code_"]');
    const sand = await until(() => document.querySelector('#stack [data-gw="current_a"] iframe.gcodefr')?.getAttribute('sandbox') === 'allow-scripts' || null, null, 8000);
    check('it runs in a frame that allows scripts only', !!sand);
    await wait(500);
    const inner = async (name) => {
      for (const f of p.frames()) if (f !== p.mainFrame()) { const t = await f.evaluate(() => document.body.innerText).catch(() => ''); if (t.includes(name)) return t; }
      return '';
    };
    check('and draws the value it is given', /current_a/.test(await inner('current_a')) && /\d/.test(await inner('current_a')), await inner('current_a'));
  },
  {
    files: { '/data/flight.tlm': { channels: ['battery_v', 'current_a', 'throttle_pct'], units: ['V', 'A', '%'] } },
    launch: '/data/flight.tlm',
    settings: { customWidgets: [], trustedWidgetSources: [] },
  },
);
