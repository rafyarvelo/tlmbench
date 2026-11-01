// Settings: the sidebar panel applies and saves changes, Basic/All, search, time labels, theme.
const T = require('./lib');
T.run('settings', async ({ p, check, wait, settingsLS, expandTree }) => {
  await p.click('#b-settings'); await wait(300);
  const rows = () => p.evaluate(() => [...document.querySelectorAll('#setPanel .row, #setPanel label.switch')].filter(r => r.offsetParent).length);
  const basic = await rows();
  check('Basic view is short', basic > 5 && basic < 20, basic);
  await p.click('#setPanel [data-sv="all"]'); await wait(200);
  check('All settings shows more', (await rows()) > basic + 10, await rows());
  await p.click('#setPanel [data-sv="basic"]'); await wait(200);

  // search covers every setting, by name, explanation and choices
  const hits = async q => { await p.fill('.setsearch input', q); await wait(150); return p.evaluate(() => [...document.querySelectorAll('#setPanel .row, #setPanel label.switch')].filter(r => r.offsetParent).map(r => (r.querySelector('span') || r).childNodes[0].textContent.trim())); };
  const h1 = await hits('number');
  check('"number" finds file labels via its choices', h1.includes('Channels from several files'), h1);
  const h2 = await hits('zoom speed');
  check('search finds an All-only setting', h2.length === 1 && h2[0] === 'Zoom speed', h2);
  await hits('xyzzy');
  check('no match message', await p.isVisible('.setnone'));
  await p.fill('.setsearch input', ''); await wait(150);
  check('clearing restores Basic', (await rows()) === basic, await rows());

  // changes apply and save (switch, select)
  await p.click('#setPanel label.switch:has([data-set="groupChannels"])'); await wait(300);
  check('switch saves', (await settingsLS()).groupChannels === false);
  check('switch applies', (await p.evaluate(() => document.querySelectorAll('.ghead[data-group]').length)) === 0);
  await p.click('#setPanel label.switch:has([data-set="groupChannels"])'); await wait(200);
  await p.click('#setPanel [data-sv="all"]'); await wait(200);
  await p.selectOption('#setPanel [data-set="timeAxis"]', 'every'); await wait(800);
  check('time labels setting saves', (await settingsLS()).timeAxis === 'every');
  check('every chart gets a time axis', await p.evaluate(() => { const h = [...document.querySelectorAll('#stack section.pane .plot')].map(e => Math.round(e.getBoundingClientRect().height)); return h.length > 1 && h.every(x => x === h[0]); }));
  await p.selectOption('#setPanel [data-set="timeAxis"]', 'auto'); await wait(500);
  check('desktop: only the bottom chart has one', await p.evaluate(() => { const h = [...document.querySelectorAll('#stack section.pane .plot')].map(e => Math.round(e.getBoundingClientRect().height)); return h[h.length - 1] > h[0]; }));
  const bg = () => p.evaluate(() => getComputedStyle(document.body).backgroundColor);
  const dark = await bg();
  await p.selectOption('#setPanel [data-set="theme"]', 'light'); await wait(300);
  check('theme applies', (await bg()) !== dark, [dark, await bg()]);
  await p.selectOption('#setPanel [data-set="theme"]', 'dark'); await wait(200);
});
