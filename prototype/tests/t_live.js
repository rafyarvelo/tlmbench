// Live tabs are file tabs whose files grow: start, follow/hold, Explorer, tables, derived, sources, pause, tab switch, close, reload.
const T = require('./lib');
T.run('live', async ({ p, check, tabs, paneCount, text, expandTree, wait }) => {
  const rng = () => text('#rangeTxt');
  await p.click('#railLive'); await wait(300);
  check('connect dialog', await p.isVisible('#liveDlg'));
  await p.click('#liveDlg [data-go]'); await wait(2500);
  const t1 = await tabs();
  check('live tab opens in front', t1.length === 2 && /\*$/.test(t1[1]), t1);
  check('live strip', /LIVE/.test(await text('#liveStrip')) && /Following/.test(await text('#liveStrip')));
  check('source is a file in Explorer', await p.evaluate(() => [...document.querySelectorAll('.fitem.open')].some(f => /LIVE/.test(f.textContent))));
  check('starter panes', (await paneCount()) >= 3);
  const tiles = await p.evaluate(() => [...document.querySelectorAll('[data-tile] [data-tv]')].map(e => e.textContent));
  check('value tiles show newest samples', tiles.length > 0 && tiles.every(v => v !== '—' && v !== '…'), tiles);
  check('overview hidden in live tabs', !(await p.isVisible('.overview')));

  // Explorer works the same: add a channel in a new chart
  await expandTree();
  const before = await paneCount();
  await p.click('.chan[data-ch="motor_3_rpm"] .nm'); await wait(100);
  await p.evaluate(() => document.querySelector('.chan[data-ch="motor_3_rpm"] [data-cnew="chart"]').click()); await wait(800);
  check('new chart from Explorer', (await paneCount()) === before + 1);

  // following moves the view; a chosen view holds; End returns
  await p.click('#liveStrip [data-win="10"]'); await wait(9000);
  const a = await rng(); await wait(1200); const b = await rng();
  check('following scrolls the view', a !== b, [a, b]);
  const box = await p.locator('#stack section.pane .plot').first().boundingBox();
  await p.mouse.move(box.x + 300, box.y + 40); await p.mouse.down(); await p.mouse.move(box.x + 500, box.y + 40, { steps: 6 }); await p.mouse.up(); await wait(900);
  check('a box zoom holds the view', /Jump to live/.test(await text('#lvFollow')));
  let h1 = await rng(); for (let k = 0; k < 20; k++) { await wait(300); const r = await rng(); if (r === h1) break; h1 = r; } await wait(800); // once the zoom's glide has settled
  check('held view stays put', h1 === await rng());
  await p.keyboard.press('End'); await wait(500);
  check('End returns to live', /Following/.test(await text('#lvFollow')));
  // the scrubber: drag back through the session, the right end is live
  const setScrub = v => p.evaluate(v => { const r = document.getElementById('lvScrub'); r.value = String(v); r.dispatchEvent(new Event('input', { bubbles: true })); }, v);
  check('the live strip has a scrubber at live', await p.evaluate(() => { const r = document.getElementById('lvScrub'); return !!r && +r.value >= 990 && /live/.test(document.getElementById('lvAt').textContent); }));
  const rLive = await rng(); await setScrub(300); await wait(400);
  check('dragging back holds the view earlier', /Jump to live/.test(await text('#lvFollow')) && /^−/.test(await text('#lvAt')) && (await rng()) !== rLive, [await text('#lvAt'), await rng()]);
  const held = await rng(); await wait(1200);
  check('the held position stays while data arrives', (await rng()) === held);
  await setScrub(1000); await wait(400);
  check('the right end returns to live', /Following/.test(await text('#lvFollow')) && /live/.test(await text('#lvAt')));

  // chart + table on live data: rows end at the newest sample
  await p.evaluate(() => document.querySelector('#stack section.pane [data-viewmenu]').click()); await wait(200);
  await p.evaluate(() => [...document.querySelectorAll('.viewmenu .mi')].find(x => /Chart and table/.test(x.textContent)).click()); await wait(1500);
  const st = await text('.tstat');
  check('live table counts rows up to now', /rows in view/.test(st) && !/2,160,001|60,001/.test(st), st);

  // derived channel on the live file
  await p.click('.chan[data-ch="battery_v"] .nm'); await p.click('.chan[data-ch="current_a"] .nm', { modifiers: ['ControlOrMeta'] }); await wait(150);
  await p.evaluate(() => [...document.querySelectorAll('#exSelBar button')].find(b => b.textContent === 'Derive…').click()); await wait(300);
  await p.selectOption('#recDlg [data-r="tpl"]', 'prod'); await wait(150);
  check('derive on live data', /samples/.test(await p.textContent('#recStat')));
  await p.click('#recDlg [data-ok]'); await wait(1500);
  check('derived row on the live file', (await p.evaluate(() => document.querySelectorAll('.dglyph').length)) >= 1);

  // sources popover, pause, second source
  await p.click('#lvSrcBtn'); await wait(300);
  check('sources popover', /STREAM HEALTH|Stream health/i.test(await text('.lvpop')));
  await p.evaluate(() => document.querySelector('.lvpop [data-spause]').click()); await wait(800);
  check('pause shows PAUSED', /PAUSED/.test(await text('#lvBadge')));
  const rp = await rng(), recP = await text('#lvRec'); await wait(1300);
  check('pausing a replay stops time: the charts hold still', (await rng()) === rp && (await text('#lvRec')) === recP, [rp, await rng()]);
  check('the badge says why', await p.evaluate(() => /replay stops in time/.test(document.getElementById('lvBadge').dataset.tip)));
  await p.evaluate(() => document.querySelector('.lvpop [data-spause]').click()); await wait(1300);
  check('resuming moves on from where it paused', (await rng()) !== rp);
  await p.click('#lvAddSrc2'); await wait(300);
  await p.click('#liveDlg [data-go]'); await wait(1500);
  check('second source is a second LIVE file', (await p.evaluate(() => document.querySelectorAll('.fbadge.live').length)) === 2);
  check('strip counts sources', /2 sources/.test(await text('#liveStrip')));

  // switch tabs and back
  await p.evaluate(() => [...document.querySelectorAll('#wsTabs .wstab')][0].click()); await wait(800);
  check('file tab hides the strip', await p.evaluate(() => document.getElementById('liveStrip').hidden));
  await p.evaluate(() => [...document.querySelectorAll('#wsTabs .wstab')][1].click()); await wait(800);
  check('back to live shows the strip', await p.evaluate(() => !document.getElementById('liveStrip').hidden));

  // the start page shows the recording
  await p.evaluate(() => document.getElementById('brandHome').click()); await wait(500);
  check('start page REC banner', /Recording to/.test(await text('#startPage')));
  await p.evaluate(() => document.getElementById('brandHome').click()); await wait(600);

  // closing the tab stops the session and removes its files
  await p.evaluate(() => { const t = [...document.querySelectorAll('#wsTabs .wstab')].find(e => e.getAttribute('aria-selected') === 'true'); t.querySelector('[data-tabx]').click(); }); await wait(800);
  check('closing the live tab removes its files', (await p.evaluate(() => document.querySelectorAll('.fbadge.live').length)) === 0);
  await wait(2200); await p.reload(); await wait(2500);
  check('a live session is not restored', (await tabs()).length === 1);
});
