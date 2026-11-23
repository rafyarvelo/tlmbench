// Text from a file is data, never markup (R6's security review): a CSV's text column becomes a labelled channel, and its
// texts reach the Measure panel, which is built as HTML. Browser-only mode and an exported HTML have no content policy,
// so a label like `<img src=x onerror=…>` must show as text there and run nothing.
import { run } from './lib.js';

const F = '/data/hostile.tlm';
const EVIL = '<img src=x onerror="window.__pwned=1">';

run(
  'hostilefile',
  async ({ p, check, until, plotAt }) => {
    check('the file opens and draws', await until(() => window.__tlmApp?.drawn?.().length > 0, null, 15000));
    await p.evaluate(() => window.__tlmApp.setPanes([{ channels: [0] }]));
    await until(() => window.__tlmApp.drawn().length > 0);
    await p.click('[data-rail="measure"]');
    // A on the first sample (the fake's value there is 0: the code of the first label), B placed as a person does,
    // with the pointer over the chart, which also brings the panel up to date
    await p.evaluate(() => window.__tlmApp.setCursor('A', window.__tlmApp.view().t0));
    const at = await plotAt(0, 0.6);
    await p.mouse.move(at.x, at.y);
    await p.keyboard.press('b');
    check("a label's markup stays text in the cursor table", await until((e) => document.querySelector('#mpCur [data-mrow]')?.textContent.includes(e), EVIL, 6000), await p.evaluate(() => document.querySelector('#mpCur')?.innerText.slice(0, 300)));
    check('and nothing ran or was added', !(await p.$('#mpCur img, #mpStats img')) && !(await p.evaluate(() => window.__pwned)));
  },
  { files: { [F]: { channels: ['mode', 'rpm'], labels: { mode: [EVIL, EVIL, EVIL] } } }, settings: { fileOpenNew: 'starter' } },
);
