// Widgets of your own: the Your widgets editor (a list and a form: examples, kind, inputs and parameters, Duplicate,
// Delete, Edit as JSON, a live preview, problems named, saved in settings; Edit this widget from its menu), a
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

    // the editor (X3, Nov 22): a list of your widgets and a form for the one picked, with a live preview
    await openMenu('current_a');
    check('with none of your own, the menu offers to make one', (await p.$$eval('.widgetmenu [data-wt]', (e) => e.map((x) => x.dataset.wt))).includes('yourlist') && /Make a widget/.test(await text('.widgetmenu [data-wt="yourlist"]')));
    await p.click('.widgetmenu [data-wt="yourlist"]');
    check('the editor opens empty, offering the two examples', await until(() => !!document.querySelector('[data-wempty]')) && (await p.locator('[data-wnew]').count()) === 2);
    await p.click('[data-wnew="svg"]');
    check('New SVG widget adds the example, picked, with a preview', await until(() => document.querySelector('[data-wname]')?.value === 'Throttle lever' && !!document.querySelector('[data-wprev] svg.gsvg')));
    check('the preview binds the values', /4,610/.test(await p.$eval('[data-wprev]', (e) => e.textContent)), await text('[data-wprev]'));
    // D-ui-59: a new widget of either kind, filled in with a working example
    await p.click('[data-wnew="code"]');
    check('New code widget adds a code widget with its example, picked', await until(() => document.querySelectorAll('[data-witem]').length === 2 && document.querySelector('[data-wname]')?.value === 'Level bar (code)' && /build\(el, info\)/.test(document.querySelector('[data-wcode]')?.value ?? '')));
    check('its preview runs the code in a sandboxed frame', await until(() => document.querySelector('[data-wprev] iframe.gcodefr')?.getAttribute('sandbox') === 'allow-scripts'));
    await p.click('[data-wnew="svg"]');
    check('another SVG widget is named so it is new', await until(() => document.querySelector('[data-wname]')?.value === 'Throttle lever 2'));
    // edit in the form: the list follows the name, and a clash stops Save with the reason
    await p.fill('[data-wname]', 'Throttle lever');
    check('two widgets with one name: named, and Save is off', (await until(() => /Two widgets are named/.test(document.querySelector('[data-werr]')?.textContent ?? ''))) && (await p.$eval('[data-wsave]', (e) => e.disabled)), await text('[data-werr]'));
    await p.fill('[data-wname]', 'Dial');
    await p.fill('[data-wsvg]', '<svg viewBox="0 0 120 96"><path d="M60 80L60 20" transform="rotate({{angle}} 60 80)" stroke="{{color}}"/></svg>');
    check('the list follows the form', await until(() => !!document.querySelector('[data-witem="Dial"]')));
    await p.fill('[data-wangle="0"]', '-60');
    await p.fill('[data-wangle="1"]', '60');
    // a code widget's inputs and parameters are rows in the form
    await p.click('[data-witem="Level bar (code)"]');
    await p.click('[data-waddinput]');
    await p.fill('[data-winput="0"] input[aria-label="Role"]', 'pitch');
    await p.click('[data-waddparam]');
    await p.fill('[data-wparam="0"] input[aria-label="Name"]', 'scale');
    await p.fill('[data-wparam="0"] input[aria-label="Default"]', '2');
    // switching kind keeps what the other kind had until Save
    await p.click('[data-wkind="svg"]');
    check('a code widget switched to SVG gets an SVG', await until(() => !!document.querySelector('[data-wsvg]') && !document.querySelector('[data-wcode]')));
    await p.click('[data-wkind="code"]');
    check('and back, its code is still there', /build\(el, info\)/.test(await p.$eval('[data-wcode]', (e) => e.value)) && (await p.$eval('[data-winput="0"] input', (e) => e.value)) === 'pitch');
    // Duplicate and Delete
    await p.click('[data-wdup]');
    check('Duplicate makes a copy, picked', await until(() => document.querySelector('[data-wname]')?.value === 'Level bar (code) copy'));
    await p.click('[data-wdel]');
    check('Delete takes it out', await until(() => document.querySelectorAll('[data-witem]').length === 3));
    // Edit as JSON: the whole list; a mistake keeps the JSON open with the reason; back to the form when it reads
    await p.click('[data-wjson]');
    check('Edit as JSON shows every widget', await until(() => { try { return JSON.parse(document.querySelector('[data-wtxt]').value).length === 3; } catch { return false; } }));
    const good = await p.$eval('[data-wtxt]', (e) => e.value);
    await p.fill('[data-wtxt]', '[{ "name": "x" ');
    await p.click('[data-wjson]');
    check('a JSON mistake is named, and the JSON stays', (await until(() => /valid JSON/.test(document.querySelector('[data-werr]')?.textContent ?? ''))) && !!(await p.$('[data-wtxt]')), await text('[data-werr]'));
    await p.fill('[data-wtxt]', '[{ "name": "NoSvg" }]');
    await p.click('[data-wjson]');
    check('a widget needs an svg or code', await until(() => /needs a name and an svg/.test(document.querySelector('[data-werr]')?.textContent ?? '')));
    const evil = { name: 'Evil', svg: '<svg viewBox="0 0 120 96" onload="window.__pwned=1"><script>window.__pwned=2</script><image href="http://example.com/x.png"/><a href="http://example.com"><text x="10" y="50" fill="{{color}}" onclick="window.__pwned=3">{{value}} {{unit}}</text></a></svg>' };
    await p.fill('[data-wtxt]', JSON.stringify([evil, ...JSON.parse(good)]));
    await p.click('[data-wjson]');
    check('back in the form with the list from the JSON', await until(() => document.querySelectorAll('[data-witem]').length === 4));
    await p.click('[data-witem="Evil"]');
    check('the preview is made safe', await until(() => !!document.querySelector('[data-wprev] .gsvg') && !document.querySelector('[data-wprev] script, [data-wprev] image, [data-wprev] [onload], [data-wprev] [onclick]') && !window.__pwned));
    await p.click('[data-wsave]');
    const saved = async () => (await settingsLS()).customWidgets ?? [];
    check('saved in settings', await until(() => true) && (await saved()).length === 4 && (await saved())[0].id === 'cw_evil', JSON.stringify((await saved()).map((w) => w.id)));
    const lvl = (await saved()).find((w) => w.name === 'Level bar (code)');
    check('a code widget keeps its inputs and parameters, and only its kind\'s fields', lvl && lvl.inputs?.[0]?.role === 'pitch' && lvl.params?.[0]?.name === 'scale' && lvl.params[0].default === '2' && !('svg' in lvl), JSON.stringify(lvl));
    check('the angle is saved', JSON.stringify((await saved()).find((w) => w.name === 'Dial')?.angle) === '[-60,60]', JSON.stringify(await saved()));

    // yours join the menu; a pick fills the bindings and removes what isn't safe
    await openMenu('current_a');
    check('your widgets fold into one ▸ item, so the menu stays short', !(await p.$('.widgetmenu [data-wt="cw_evil"]')) && !!(await p.$('.widgetmenu [data-wt="yourlist"]')));
    await p.click('.widgetmenu [data-wt="yourlist"]');
    check('which lists them', (await p.$$eval('.widgetmenu [data-wt]', (e) => e.map((x) => x.dataset.wt))).includes('cw_evil'));
    await p.click('.widgetmenu [data-wt="cw_evil"]');
    check('scripts, handlers, images and outside links are gone', await until(() => /\d/.test(document.querySelector('#stack [data-gw="current_a"]')?.textContent ?? '') && !document.querySelector('#stack [data-gw="current_a"] script, #stack [data-gw="current_a"] image, #stack [data-gw="current_a"] [onload], #stack [data-gw="current_a"] [onclick], #stack [data-gw="current_a"] a[href]')) && !(await p.evaluate(() => window.__pwned)));
    check('the bindings are filled', !/\{\{/.test(await p.$eval(card('current_a'), (e) => e.innerHTML)), await text(card('current_a')));
    await openMenu('battery_v');
    await p.click('.widgetmenu [data-wt="yourlist"]');
    await p.click('.widgetmenu [data-wt="cw_dial"]');
    check('{{angle}} runs across the range', await until(() => /rotate\(-?\d/.test(document.querySelector('#stack [data-gw="battery_v"]')?.innerHTML ?? '')));

    // a code widget of yours runs at once, in a sandboxed frame
    // a widget in use has "Edit this widget…", which opens the editor on it; a rename keeps it in its panes
    await openMenu('battery_v');
    check('a widget of yours in use has Edit this widget', !!(await p.$('.widgetmenu [data-wt="editw"]')));
    await p.click('.widgetmenu [data-wt="editw"]');
    check('which opens the editor on it', await until(() => document.querySelector('[data-wname]')?.value === 'Dial'));
    await p.fill('[data-wname]', 'Needle');
    await p.click('[data-wsave]');
    check('renamed, it keeps its id, so the pane still shows it', await until(() => true) && (await saved()).some((w) => w.name === 'Needle' && w.id === 'cw_dial') && (await p.getAttribute(card('battery_v'), 'data-gtype')) === 'cw_dial');
    await openMenu('current_a');
    await p.click('.widgetmenu [data-wt="yourlist"]');
    await p.click('.widgetmenu [data-wt="yours"]');
    await p.click('[data-wjson]');
    await p.fill('[data-wtxt]', JSON.stringify([{ name: 'Mine (code)', code: LEVEL.code }, evil]));
    await p.click('[data-wsave]');
    await openMenu('current_a');
    await p.click('.widgetmenu [data-wt="yourlist"]');
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
