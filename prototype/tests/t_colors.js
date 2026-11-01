// Channel colours: with more channels than colours they cycle evenly (no colour takes the rest), and channels in one pane differ first.
const T = require('./lib');
T.run('colors', async ({ p, check, wait }) => {
  const cols = await p.evaluate(() => [...document.querySelectorAll('#stack .tth[data-cid] .tsw, #stack .tth[data-cid] i')].map(e => getComputedStyle(e).backgroundColor).filter(c => c && c !== 'rgba(0, 0, 0, 0)'));
  const counts = {}; cols.forEach(c => { counts[c] = (counts[c] || 0) + 1; });
  const n = Object.values(counts);
  check('a table of every channel uses all eight colours', Object.keys(counts).length === 8, counts);
  check('...evenly: no colour has more than one extra', Math.max(...n) - Math.min(...n) <= 1, counts);
}, { settings: { fileOpenNew: 'table', fileOpen: 'table' }, wait: 5000 });
