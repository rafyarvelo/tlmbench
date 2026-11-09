// Widgets of your own (design: Gauges and widgets). Declarative ones are an SVG with bindings, made safe before use;
// code widgets run in a sandboxed iframe and are given values by postMessage. Both are plain data, kept in settings
// until Share (5.7) writes them to `.tlmbench/widgets/`.

import type { Ctx } from './gauges';
import { gNum } from './gauges';

export interface CustomWidget {
  id: string;
  name: string;
  /** A small SVG (currentColor) for menus. */
  icon?: string;
  /** Declarative: an SVG with {{value}} {{unit}} {{label}} {{pct}} {{angle}} {{color}} {{level}} {{min}} {{max}} {{name}}. */
  svg?: string;
  /** {{angle}} runs from the first to the second as the value runs across the range (default -120 to 120). */
  angle?: [number, number];
  /** A code widget: `{ build(el, info) { … }, update(el, x) { … } }` as text. */
  code?: string;
  /** Where a shared widget came from (a folder's `.tlmbench/widgets/<name>`) and who shared it; code from there asks first. */
  shared?: string;
  by?: string;
}

export const WIDGET_EXAMPLE: Omit<CustomWidget, 'id'> = {
  name: 'Throttle lever',
  icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M6 20h12M12 20V6"/><circle cx="12" cy="6" r="3"/></svg>',
  angle: [-60, 60],
  svg: '<svg viewBox="0 0 120 96"><path d="M30 86h60" stroke="currentColor" stroke-width="2" opacity=".4"/><g transform="rotate({{angle}} 60 86)"><path d="M60 86V30" stroke="{{color}}" stroke-width="6" stroke-linecap="round"/><circle cx="60" cy="26" r="9" fill="{{color}}"/></g><text x="60" y="14" text-anchor="middle" font-size="12" fill="currentColor">{{value}} {{unit}}</text></svg>',
};

export const esc = (s: unknown): string => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);

const INVALID = '<svg viewBox="0 0 120 96"><text x="60" y="50" text-anchor="middle" font-size="9" fill="currentColor">Not a valid SVG</text></svg>';

/** An SVG made safe: scripts, foreign objects, images, media and outside links are removed, and so are event handlers
 * and anything that points outside the document. */
export function sanitizeSvg(src: string): string {
  const doc = new DOMParser().parseFromString(String(src), 'image/svg+xml');
  const root = doc.documentElement;
  if (!root || root.nodeName.toLowerCase() !== 'svg' || doc.querySelector('parsererror')) return INVALID;
  root.querySelectorAll('script, foreignObject, iframe, object, embed, audio, video, image, use[href^="http"]').forEach((n) => n.remove());
  for (const n of [root, ...root.querySelectorAll('*')])
    for (const a of [...n.attributes]) {
      const k = a.name.toLowerCase();
      const v = a.value.trim().toLowerCase();
      if (k.startsWith('on') || ((k === 'href' || k === 'xlink:href') && !v.startsWith('#')) || v.includes('javascript:') || v.includes('url(http') || v.includes('url("http') || v.includes("url('http")) n.removeAttribute(a.name);
    }
  return new XMLSerializer().serializeToString(root);
}

/** A declarative widget's SVG with its bindings filled in from `x`, safe to put in the page. */
export function fillTemplate(spec: CustomWidget, x: Pick<Ctx, 'txt' | 'unit' | 'label' | 'ok' | 'p' | 'col' | 'lvl' | 'lo' | 'hi' | 'dec' | 'name'>): string {
  const [a0, a1] = spec.angle ? [Number(spec.angle[0]), Number(spec.angle[1])] : [-120, 120];
  const vars: Record<string, string> = {
    value: x.txt,
    unit: esc(x.unit),
    label: esc(x.label ?? x.txt),
    pct: x.ok ? (x.p * 100).toFixed(1) : '0',
    angle: x.ok ? (a0 + (a1 - a0) * x.p).toFixed(1) : String(a0),
    color: x.col,
    level: x.lvl || 'none',
    min: gNum(x.lo, x.dec),
    max: gNum(x.hi, x.dec),
    name: esc(x.name),
  };
  return sanitizeSvg(String(spec.svg ?? '').replace(/\{\{\s*(\w+)\s*\}\}/g, (m, k: string) => vars[k] ?? m)).replace('<svg', '<svg class="gsvg"');
}

/** What the sandboxed frame runs: it takes the widget's code once, then values. No same-origin access, no network. */
export const CODE_HOST = `<!doctype html><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline' 'unsafe-eval'"><style>html,body{margin:0;background:transparent;overflow:hidden}</style><div id="w"></div><script>
let api=null;const el=document.getElementById('w');
addEventListener('message',e=>{const m=e.data||{};if(m.code!=null){document.body.style.color=m.info.fg;try{api=(new Function('return ('+m.code+')'))();if(api&&api.build)api.build(el,m.info)}catch(err){el.textContent='Widget error: '+err.message}parent.postMessage({tlmReady:1},'*')}else if(api&&api.update){try{api.update(el,m)}catch(err){el.textContent='Widget error: '+err.message;api=null}}});
<\/script>`;

/** Yours run without asking; a shared one asks once per folder. */
export const codeTrusted = (spec: CustomWidget, trusted: string[]): boolean => !spec.shared || trusted.includes(spec.shared);
