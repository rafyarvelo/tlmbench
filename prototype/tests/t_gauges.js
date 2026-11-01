// Gauges pane: widgets pick themselves, follow the time, colour by limits; change type and range; your own SVG widgets (sanitised).
const T = require('./lib');
T.run('gauges', async ({ p, check, paneCount, expandTree, menu, pick, wait, settingsLS, until }) => {
  await expandTree();
  const ids = ['motor_rpm', 'esc_temp_1', 'battery_v', 'throttle_pct', 'roll', 'yaw', 'flight_mode', 'current_a', 'gps_sats'];
  await p.click(`.chan[data-ch="${ids[0]}"] .nm`);
  for (const id of ids.slice(1)) await p.click(`.chan[data-ch="${id}"] .nm`, { modifiers: ['Control'] });
  await p.evaluate(() => [...document.querySelectorAll('#exSelBar button')].find(b => /^Chart$/.test(b.textContent)).click()); await wait(1500);
  const last = await p.evaluate(() => [...document.querySelectorAll('#stack section.pane')].findIndex(sec => sec.querySelector('[data-chip="yaw"]')));
  await p.evaluate(i => document.querySelectorAll('#stack section.pane [data-viewmenu]')[i].click(), last); await wait(200);
  await p.evaluate(() => document.querySelector('.viewmenu [data-vm="gauge"]').click()); await wait(1500);
  const kinds = await p.evaluate(() => [...document.querySelectorAll('#stack .gw')].map(g => g.dataset.gw + ':' + (g.querySelector('svg') ? 'svg' : g.textContent.trim())));
  check('a widget per channel', kinds.length === 9 && kinds.every(k => /:svg$/.test(k)), kinds);
  const pick1 = id => p.evaluate(id => { const g = document.querySelector(`#stack .gw[data-gw="${id}"]`); return { needle: !!g.querySelector('.gneedle'), clip: !!g.querySelector('clipPath'), dash: !!g.querySelector('[stroke-dasharray]'), text: g.querySelector('svg').textContent.replace(/\s+/g, ' ').trim() }; }, id);
  check('rpm gets a needle gauge', (await pick1('motor_rpm')).needle);
  check('roll gets a horizon', (await pick1('roll')).clip);
  check('throttle % gets a ring', (await pick1('throttle_pct')).dash);
  check('yaw gets a compass', /N/.test((await pick1('yaw')).text) && (await pick1('yaw')).needle);
  await p.locator('#stack section.pane').nth(last).screenshot({ path: T.DIR + '/out/gauges.png' });

  // values follow the pointer / end of view: zoom to a different time and the rpm reading changes
  const v1 = (await pick1('motor_rpm')).text;
  await p.evaluate(() => { const b = document.getElementById('b-fit'); b && b.click(); }); await wait(300);
  const plot = await p.locator('#stack section.pane .plot').first().boundingBox();
  await p.mouse.move(plot.x + 120, plot.y + 50); await p.mouse.down(); await p.mouse.move(plot.x + 200, plot.y + 50, { steps: 5 }); await p.mouse.up(); await wait(900);
  check('reading follows the view', (await pick1('motor_rpm')).text !== v1, [v1, (await pick1('motor_rpm')).text]);

  // change the widget type and range
  await p.evaluate(() => document.querySelector('#stack .gw[data-gw="battery_v"] [data-gwt]').click()); await wait(200);
  const m = await menu();
  check('widget menu lists the types', ['Gauge', 'Ring', 'Bar', 'Tank', 'Sparkline', 'State light', 'Compass', 'Horizon', 'Number'].every(n => m.includes(n)), m);
  await pick(/^Tank$/); await wait(300);
  check('type kept with the pane', await p.evaluate(() => !!document.querySelector('#stack .gw[data-gw="battery_v"] svg rect[rx="8"]')));
  p.once('dialog', d => d.accept('20, 26'));
  await p.evaluate(() => document.querySelector('#stack .gw[data-gw="battery_v"] [data-gwt]').click()); await wait(200);
  await pick(/^Range/); await wait(300);
  check('custom range drawn', /26/.test((await pick1('battery_v')).text) && /20/.test((await pick1('battery_v')).text), (await pick1('battery_v')).text);

  // your own widget, sanitised
  await p.evaluate(() => document.querySelector('#stack .gw[data-gw="current_a"] [data-gwt]').click()); await wait(200);
  await pick(/^Your widgets/); await wait(300);
  check('editor with an example and preview', await p.evaluate(() => /Throttle lever/.test(document.querySelector('[data-wtxt]').value) && !!document.querySelector('[data-wprev] svg')));
  await p.fill('[data-wtxt]', JSON.stringify([{ name: 'Evil', svg: '<svg viewBox="0 0 120 96" onload="window.__pwned=1"><script>window.__pwned=2</script><text x="10" y="50" fill="{{color}}">{{value}} {{unit}}</text></svg>' }, { name: 'Throttle lever', angle: [-60, 60], svg: '<svg viewBox="0 0 120 96"><g transform="rotate({{angle}} 60 86)"><path d="M60 86V30" stroke="{{color}}" stroke-width="6"/></g><text x="60" y="14" text-anchor="middle">{{value}}</text></svg>' }]));
  await p.dispatchEvent('[data-wtxt]', 'input'); await wait(200);
  await p.click('[data-wsave]'); await wait(300);
  check('saved in settings', ((await settingsLS()).customWidgets || []).length === 2);
  await p.evaluate(() => document.querySelector('#stack .gw[data-gw="current_a"] [data-gwt]').click()); await wait(200);
  check('your widgets join the menu', (await menu()).includes('Evil') && (await menu()).includes('Throttle lever'));
  await pick(/^Evil$/); await wait(400);
  check('scripts and handlers removed', await p.evaluate(() => !window.__pwned && !document.querySelector('#stack .gw[data-gw="current_a"] script') && !document.querySelector('#stack .gw[data-gw="current_a"] [onload]')));
  check('bindings filled', /\d/.test((await pick1('current_a')).text), (await pick1('current_a')).text);

  // reload: the pane keeps its widget choices
  await wait(2300); await p.reload(); await wait(4000);
  check('widget choices persist with the layout', await until(() => !!document.querySelector('#stack .gw[data-gw="battery_v"] svg rect[rx="8"]'), null, 15000)); // (once its data is read again)
});
