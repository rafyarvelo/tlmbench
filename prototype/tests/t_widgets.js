// More widgets: commanded vs actual, bar group, flags, state timeline, XY dot (g-g and GPS), min/max marks; pairs and groups found by name, undone from the menu.
const T = require('./lib');
T.run('widgets', async ({ p, check, expandTree, menu, pick, wait, until }) => {
  await expandTree();
  const ids = ['motor_rpm', 'motor_rpm_target', 'motor_2_rpm', 'motor_3_rpm', 'motor_4_rpm', 'status_bits', 'flight_mode', 'accel_x', 'accel_y', 'gps_lat', 'gps_lon', 'battery_v'];
  await p.click(`.chan[data-ch="${ids[0]}"] .nm`);
  for (const id of ids.slice(1)) await p.click(`.chan[data-ch="${id}"] .nm`, { modifiers: ['ControlOrMeta'] });
  await p.evaluate(() => [...document.querySelectorAll('#exSelBar button')].find(b => /^Chart$/.test(b.textContent)).click()); await wait(1500);
  const idx = await p.evaluate(() => [...document.querySelectorAll('#stack section.pane')].findIndex(sec => sec.querySelector('[data-chip="status_bits"]')));
  await p.evaluate(i => document.querySelectorAll('#stack section.pane [data-viewmenu]')[i].click(), idx); await wait(200);
  await p.evaluate(() => document.querySelector('.viewmenu [data-vm="gauge"]').click());
  await until(() => document.querySelectorAll('#stack .gw[data-gtype]').length >= 6 && !document.querySelector('#stack .gw .gpend'), null, 15000);
  const cards = () => p.evaluate(() => Object.fromEntries([...document.querySelectorAll('#stack .gw[data-gw]')].filter(g => !g.hidden).map(g => [g.dataset.gw, g.dataset.gtype])));
  const c1 = await cards();
  check('commanded vs actual pairs rpm with its target', c1.motor_rpm === 'bullet' && !('motor_rpm_target' in c1), c1);
  check('three similar motors make a bar group', c1.motor_2_rpm === 'group' && !('motor_3_rpm' in c1) && !('motor_4_rpm' in c1), c1);
  check('status word gets flags', c1.status_bits === 'flags');
  check('mode gets a state timeline', c1.flight_mode === 'timeline');
  check('accel x/y make an XY dot', c1.accel_x === 'xy' && !('accel_y' in c1));
  check('lat/lon make a GPS track', c1.gps_lat === 'xy' && !('gps_lon' in c1));
  check('battery stays a gauge', c1.battery_v === 'arc');
  const svgText = id => p.evaluate(id => { const g = document.querySelector(`#stack .gw[data-gw="${id}"]`); return g ? g.querySelector('svg').textContent.replace(/\s+/g, ' ').trim() : null; }, id);
  check('bullet shows the target and the difference', await p.evaluate(() => { const g = document.querySelector('#stack .gw[data-gw="motor_rpm"]'); return !!g.querySelector('.gtarget') && /\d/.test(g.querySelector('[data-delta]').textContent); }));
  check('group labels its members', /spread/.test(await svgText('motor_2_rpm')) && await p.evaluate(() => document.querySelectorAll('#stack .gw[data-gw="motor_2_rpm"] rect').length >= 6));
  check('flags name their bits', /armed/.test(await svgText('status_bits')) && /failsafe/.test(await svgText('status_bits')) && /0x/.test(await svgText('status_bits')));
  check('timeline shows the mode by name', /Disarmed|Takeoff|Mission|Land/.test(await svgText('flight_mode')), await svgText('flight_mode'));
  check('XY draws a dot and a trail', await p.evaluate(() => !!document.querySelector('#stack .gw[data-gw="accel_x"] .gdot')));
  check('GPS track gives its size', /km across/.test(await svgText('gps_lat')), await svgText('gps_lat'));
  check('min/max marks on gauges and groups', await p.evaluate(() => document.querySelectorAll('#stack .gw[data-gw="battery_v"] .ghold').length === 2 && document.querySelectorAll('#stack .gw[data-gw="motor_2_rpm"] .ghold').length === 6));
  await p.locator('#stack section.pane').nth(idx).screenshot({ path: T.DIR + '/out/widgets.png' });

  // the failsafe bit lights up in the failsafe
  await p.evaluate(() => { const b = document.getElementById('b-play'); b.click(); });
  await p.evaluate(() => { const r = document.querySelector('#playStrip [data-pscrub]'); r.value = String(Math.round(5015 / 7200 * 1000)); r.dispatchEvent(new Event('input', { bubbles: true })); }); await wait(500);
  check('a bad bit set marks the card', await p.evaluate(() => !!document.querySelector('#stack .gw[data-gw="status_bits"].alarm .gbad')));
  await p.evaluate(() => document.querySelector('#playStrip [data-pclose]').click()); await wait(200);

  // menus: hold marks off, ungroup, separate, put back
  const openMenu = async id => { await p.evaluate(id => document.querySelector(`#stack .gw[data-gw="${id}"] [data-gwt]`).click(), id); await wait(200); return menu(); };
  let m = await openMenu('battery_v');
  check('single widget menu offers automatic and types', m.some(x => /^Automatic \(Gauge\)/.test(x)) && m.includes('Flags') && m.includes('State timeline') && !m.includes('Bar group'), m);
  await pick(/^Min\/max in view/);
  check('marks turn off per widget', await p.evaluate(() => document.querySelectorAll('#stack .gw[data-gw="battery_v"] .ghold').length === 0));
  m = await openMenu('motor_2_rpm');
  check('group menu can ungroup', m.some(x => /^Ungroup/.test(x)), m);
  await pick(/^Ungroup/);
  let c2 = await cards();
  check('ungrouped: a widget each, and they stay apart', c2.motor_2_rpm === 'arc' && c2.motor_3_rpm === 'arc' && c2.motor_4_rpm === 'arc', c2);
  m = await openMenu('motor_3_rpm');
  check('a single widget can join a group', m.some(x => /^Bar group, with 2 similar/.test(x)), m);
  await pick(/^Bar group/);
  c2 = await cards();
  check('regrouped from any member', c2.motor_3_rpm === 'group' && !('motor_2_rpm' in c2) && !('motor_4_rpm' in c2), c2);
  m = await openMenu('motor_rpm');
  await pick(/^Separate/);
  c2 = await cards();
  check('separate the pair', c2.motor_rpm === 'arc' && c2.motor_rpm_target === 'arc', c2);

  await wait(2300); await p.reload();
  const c3 = await until(() => { const g = document.querySelectorAll('#stack .gw[data-gtype]'); return g.length ? Object.fromEntries([...g].filter(x => !x.hidden).map(x => [x.dataset.gw, x.dataset.gtype])) : null; }, null, 15000);
  check('choices persist with the layout', c3 && c3.motor_3_rpm === 'group' && c3.motor_rpm_target === 'arc', c3);

  // size per widget: Large spans two columns and is kept after a reload
  await p.evaluate(() => document.querySelector('#stack .gw[data-gw="battery_v"] [data-gwt]').click()); await wait(200);
  check('the widget menu offers sizes', /Small[\s\S]*Medium[\s\S]*Large \(2 wide\)[\s\S]*2 × 2/.test((await menu()).join('|')), await menu());
  await pick(/^Large \(2 wide\)$/); await wait(400);
  const sz = await p.evaluate(() => { const g = document.querySelector('#stack .gw[data-gw="battery_v"]'); return { s: g.dataset.gsize, col: getComputedStyle(g).gridColumnStart }; });
  check('Large spans two columns', sz.s === 'L' && /span 2/.test(sz.col), sz);
  // reload only once the session (saved every 2 s) holds the size, so a slow machine can't race it
  check('the size is saved', await until(() => Object.keys(localStorage).some(k => /^plotter\.cache\.session\.[^.]+$/.test(k) && /"size":"L"/.test(localStorage.getItem(k))), null, 10000));
  await p.reload();
  const sz2 = await until(() => { const g = document.querySelector('#stack .gw[data-gw="battery_v"]'); return g && g.dataset.gsize ? g.dataset.gsize : null; }, null, 15000);
  check('the size is kept', sz2 === 'L', sz2);

  // a code widget from the shared folder: a value card until you trust the folder, then it runs sandboxed
  await p.evaluate(() => document.querySelector('[data-rail="layouts"]').click()); await wait(300);
  await p.evaluate(() => { const b = [...document.querySelectorAll('#layoutList [data-lopen]')].find(x => /Widgets tour/.test(x.textContent)); b.click(); }); await wait(800);
  await p.evaluate(() => { const g = document.querySelector('#slotDlg:not([hidden]) [data-go]'); if (g) g.click(); }); await wait(2500);
  const off = await until(() => { const g = document.querySelector('#stack .gw[data-gw="throttle_pct"]'); return g && g.querySelector('[data-trustw]') ? g.innerText : null; }, null, 10000);
  check('a shared code widget waits for your OK and shows the value meanwhile', !!off && /%/.test(off) && /Run\?/.test(off), off);
  await p.evaluate(() => document.querySelector('[data-trustw]').click()); await wait(300);
  check('the trust prompt names the widget, the folder and shows the code', await p.evaluate(() => { const t = document.getElementById('recDlg').innerText; return /Level bar \(code\)/.test(t) && /\.tlmbench\/widgets\/level-bar/.test(t) && /Show the code/.test(t); }));
  await p.evaluate(() => document.querySelector('#recDlg [data-trust]').click());
  const ran = await until(() => { const f = document.querySelector('#stack .gw[data-gw="throttle_pct"] iframe.gcodefr'); return f && f.getAttribute('sandbox') === 'allow-scripts' && f._ready ? true : null; }, null, 8000);
  check('trusted: it runs in a sandboxed frame (scripts only)', !!ran);
  const fr = p.frames().find(f => f !== p.mainFrame());
  const inner = fr ? await fr.evaluate(() => document.body.innerText).catch(() => '') : '';
  check('the widget draws its value', /throttle_pct/.test(inner) && /\d/.test(inner), inner);

  // a widget's title drags like a chip: along the grid to reorder, onto another pane to move the channel there
  const order = () => p.evaluate(() => [...document.querySelectorAll('#stack section.pane')[0].querySelectorAll('.gw[data-gw]:not([hidden])')].map(g => g.dataset.gw));
  const o0 = await order();
  await p.locator('#stack section.pane >> nth=0').locator('.gw[data-gw="current_a"] .gwh').dragTo(p.locator('#stack section.pane >> nth=0').locator('.gw[data-gw="motor_rpm"]'), { targetPosition: { x: 5, y: 40 } }); await wait(800);
  const o1 = await order();
  check('dragging a widget title along the grid reorders it', o1[0] === 'current_a' && o0[0] !== 'current_a', [o0, o1]);
  const last = await p.locator('#stack section.pane').count();
  await p.locator('#stack section.pane >> nth=0').locator('.gw[data-gw="esc_temp_1"] .gwh').dragTo(p.locator(`#stack section.pane >> nth=1`).locator('.ggrid')); await wait(900);
  const moved = await p.evaluate(() => { const secs = [...document.querySelectorAll('#stack section.pane')]; return { to: secs.findIndex(x => x.querySelector('[data-chip="esc_temp_1"]')), from: !!secs[0].querySelector('.gw[data-gw="esc_temp_1"]'), heads: secs.map(x => x.querySelector('.phead').innerText.replace(/\s+/g, ' ').slice(0, 60)) }; });
  check('dropping it on another pane moves the channel there', moved.to === 1 && !moved.from && last >= 3, moved);
});
