// The SVG sanitiser (R6's security review): shared widgets and icons come from other people's folders and reach exported
// HTML, which opens anywhere with no content policy of its own, so the sanitiser is their only guard. Each hostile SVG
// below must come out with nothing that runs or reaches outside the document; an ordinary widget must come out whole.
import { run } from './lib.js';

const HOSTILE = {
  'a handler': '<svg onload="alert(1)"><rect onclick="alert(1)"/></svg>',
  'a script element': '<svg><script>alert(1)</script></svg>',
  'a javascript: link': '<svg><a href="javascript:alert(1)"><text>x</text></a></svg>',
  'a tab inside "javascript:"': '<svg><a xlink:href="java&#9;script:alert(1)" xmlns:xlink="http://www.w3.org/1999/xlink"><text>x</text></a></svg>',
  'an animation that sets a link': '<svg><a href="#"><set attributeName="href" to="java&#10;script:alert(1)"/><text>x</text></a></svg>',
  'an animation of xlink:href': '<svg><a><animate attributeName="xlink:href" values="javascript:alert(1)"/><text>x</text></a></svg>',
  'an animation that adds a handler': '<svg><rect><set attributeName="onmouseover" to="alert(1)"/></rect></svg>',
  'an outside image': '<svg><image href="https://example.com/x.png"/><feImage href="//example.com/y.png"/></svg>',
  'a beacon in a style attribute': '<svg><rect style="fill: url(//example.com/beacon)"/></svg>',
  'a beacon in a style sheet': '<svg><style>rect { fill: url(https://example.com/b) }</style><rect/></svg>',
  'an @import': '<svg><style>@import "https://example.com/x.css";</style></svg>',
  'a data: URL': '<svg><rect fill="url(data:image/svg+xml;base64,AAAA)"/></svg>',
  'a foreignObject': '<svg><foreignObject><iframe src="https://example.com"></iframe></foreignObject></svg>',
};
const OK = '<svg viewBox="0 0 24 24"><defs><linearGradient id="g"><stop offset="0" stop-color="red"/></linearGradient></defs><style>.t { font-size: 12px }</style><rect fill="url(#g)" style="stroke: currentColor"/><use href="#g"/><text class="t">7</text><animate attributeName="opacity" values="0;1" dur="1s"/></svg>';

run('svgsafe', async ({ p, check, until }) => {
  check('the page boots', await until(() => !!window.__tlmApp?.sanitizeSvg, null, 15000));
  for (const [what, src] of Object.entries(HOSTILE)) {
    const out = await p.evaluate((s) => window.__tlmApp.sanitizeSvg(s), src);
    const bad = /\bon\w+=|<script|javascript|java\s*script|<image|<feimage|<foreignobject|<iframe|url\((?!["']?#)|@import|data:|attributename="(?:xlink:)?href"|attributename="on/i.exec(out);
    check(`${what}: removed`, !bad, out);
  }
  const kept = await p.evaluate((s) => window.__tlmApp.sanitizeSvg(s), OK);
  check('an ordinary widget keeps its gradient, style sheet, fragment links and animation', /url\(#g\)/.test(kept) && /<style>/.test(kept) && /href="#g"/.test(kept) && /attributeName="opacity"/.test(kept) && /currentColor/.test(kept), kept);
});
