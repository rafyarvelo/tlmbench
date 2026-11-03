// The top bar on a narrow window: Clear, Back, Fit, Play and Lock move into ⋯ (under 900 px); tabs that don't fit
// scroll, with ‹ › beside them and the tab in front kept in sight.
import { run } from './lib.js';

const names = ['alpha_flight', 'bravo_flight', 'charlie_flight', 'delta_flight', 'echo_flight', 'foxtrot_flight'];
const files = Object.fromEntries(names.map((n) => [`/data/${n}.tlm`, {}]));

run('narrow', async ({ p, check, fake, until, wait, text }) => {
  const exists = (sel) => p.evaluate((s) => !!document.querySelector(s), sel);
  check('wide: the bar has Fit, Back, Play, Lock', (await exists('#b-fit')) && (await exists('#b-back')) && (await exists('#b-play')) && (await exists('#b-lock')) && !(await exists('#tabsRight')));

  for (const n of names.slice(1)) {
    await fake((d) => (window.__tlmFake.pick = d), `/data/${n}.tlm`);
    await p.click('#b-more');
    await p.click('.moremenu .mi:has-text("Open recording")');
    await wait(250);
  }
  await p.setViewportSize({ width: 760, height: 800 });
  await wait(400);
  check('narrow: those icons leave the bar', !(await exists('#b-fit')) && !(await exists('#b-back')) && !(await exists('#b-play')) && !(await exists('#b-lock')) && (await exists('#b-more')));
  await p.click('#b-more');
  check('and ⋯ has them', /Back/.test(await text('.moremenu')) && /Fit/.test(await text('.moremenu')) && /Play back/.test(await text('.moremenu')) && /Lock the time view/.test(await text('.moremenu')), await text('.moremenu'));
  await p.click('.moremenu [data-more="play"]');
  check('Play back works from ⋯', await until(() => document.body.innerText.includes('Loop A–B') || document.body.innerText.includes('Following')));
  await p.keyboard.press('Escape');

  check('tabs that do not fit scroll, with ‹ ›', (await exists('#tabsRight')) && (await exists('#tabsLeft')));
  check('the front tab is at least partly in sight', await p.evaluate(() => { const t = document.querySelector('#wsTabs [aria-selected=true]').getBoundingClientRect(), s = document.getElementById('wsTabs').getBoundingClientRect(); return t.left < s.right && t.right > s.left; }));
  await p.click('#tabsLeft');
  await wait(500);
  check('‹ scrolls the strip back', await p.evaluate(() => document.getElementById('wsTabs').scrollLeft) < (await p.evaluate(() => document.getElementById('wsTabs').scrollWidth)) );
  await p.setViewportSize({ width: 1440, height: 900 });
  await wait(400);
  check('wide again: the icons return', (await exists('#b-fit')) && (await exists('#b-lock')));
}, { files, launch: '/data/alpha_flight.tlm' });
