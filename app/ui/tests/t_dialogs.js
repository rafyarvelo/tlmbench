// Every dialog opens centred across the window and inside it (sheet E2, F1: Rules and checks and Export to HTML opened
// stuck in the top-left corner because only some dialogs centred themselves). One shared rule in style.css centres `.dlg`;
// a dialog taller than the window scrolls its body, its close button is the shared one, and Escape closes each one.
import { run } from './lib.js';

const ch = (n) => `#explorer .row.chan[data-ch="${n}"]`;
// the Explorer panel, shown again after another rail replaced it
const explorer = async (p) => (await p.isVisible('#exMore')) || (await p.click('[data-rail="explorer"]'));
const OPENERS = [
  ['Export to HTML', '#htmlDlg', async (p) => (await p.click('#b-more'), await p.click('.moremenu [data-more="exporthtml"]'))],
  ['Export data', '#exportDlg', async (p) => (await p.click('#b-more'), await p.click('.moremenu [data-more="exportdata"]'))],
  ['Save the view as an image', '#imageDlg', async (p) => (await p.click('#b-more'), await p.click('.moremenu [data-more="exportimage"]'))],
  ['Rules and checks', '#recDlg', async (p) => (await p.click('[data-rail="checks"]'), await p.click('#checksBody [data-cnew]'))],
  ['Recipes', '#recipesDlg', async (p) => (await explorer(p), await p.click('#exMore'), await p.click('.exmenu [data-ex="recipes"]'))],
  ['New derived channel', '#deriveDlg', async (p) => (await explorer(p), await p.click(`${ch(0)} .nm`), await p.click('#selBar [data-sel="derive"]'))],
  ['Connect', '#liveDlg', async (p) => await p.click('[data-rail="live"]')],
  ['Channel editor', '#chEditor', async (p) => await p.click('#stack section.pane .chip[data-ch="0"]')],
];

run(
  'dialogs',
  async ({ p, check, until }) => {
    for (const vp of [{ width: 1440, height: 900 }, { width: 1024, height: 640 }]) {
      await p.setViewportSize(vp);
      for (const [name0, sel, openIt] of OPENERS) {
        const name = `${name0} (${vp.width}×${vp.height})`;
        await openIt(p);
        const ok = await until((s) => !!document.querySelector(s), sel);
        check(`${name} opens`, ok);
        if (!ok) continue;
        await p.waitForTimeout(260); // the rise animation
        const r = await p.$eval(sel, (e) => { const b = e.getBoundingClientRect(); return { l: b.left, r: b.right, t: b.top, b: b.bottom }; });
        const mid = (r.l + r.r) / 2;
        check(`${name} is centred across the window`, Math.abs(mid - vp.width / 2) <= 2, r);
        check(`${name} is inside the window`, r.l >= 0 && r.t >= 0 && r.r <= vp.width && r.b <= vp.height, r);
        check(`${name} is not in the top-left corner`, r.t > 8 && r.l > 8, r);
        const close = await p.$eval(sel, (e) => { const b = e.querySelector('.dclose'); return b ? Math.round(b.getBoundingClientRect().width) : null; });
        if (close !== null) check(`${name}: its close button is the shared 26 px one`, close === 26, close);
        const spill = await p.$eval(sel, (e) => e.scrollHeight - e.clientHeight);
        check(`${name}: nothing spills past its bottom (its body scrolls)`, spill <= 1, spill);
        await p.keyboard.press('Escape');
        await until((s) => !document.querySelector(s), sel);
        await p.keyboard.press('Escape');
      }
    }
  },
);
