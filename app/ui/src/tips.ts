// Tooltips: one floating tip for every element with `data-tip`, after 700 ms of hovering (at once on keyboard focus,
// and for 1.5 s after one has shown). Short (the default) shows the first sentence and the shortcut, and a tip
// retires after three showings; Full shows everything every time; Off shows none. Settings → Tooltips.

import { saveSettings, settings } from './settings.svelte';

const DELAY = 700;
const WARM = 1500;
const USES = 3;

let el: HTMLElement | null = null;
let timer = 0;
let warmUntil = 0;
let current: HTMLElement | null = null;

/** The text a tip shows in Short mode: the first sentence, and a trailing shortcut in brackets. */
export function shortTip(t: string): string {
  const key = / \(([^()]+)\)\s*$/.exec(t);
  const first = (/^.*?[.!?:](?=\s|$)/s.exec(t)?.[0] ?? t.replace(/ \([^()]+\)\s*$/, '')).replace(/:$/, '');
  return key && !first.includes(`(${key[1]})`) ? `${first} (${key[1]})` : first;
}

function hide(): void {
  clearTimeout(timer);
  current = null;
  if (el) el.hidden = true;
}

function show(target: HTMLElement): void {
  const full = target.dataset.tip ?? '';
  if (!full || settings.tooltips === 'off') return;
  let text = full;
  if (settings.tooltips === 'short') {
    if ((settings.tipsSeen[full] ?? 0) >= USES) return;
    text = shortTip(full);
    settings.tipsSeen[full] = (settings.tipsSeen[full] ?? 0) + 1;
    saveSettings();
  }
  el ??= Object.assign(document.createElement('div'), { className: 'tip', role: 'tooltip', hidden: true });
  if (!el.isConnected) document.body.append(el);
  el.textContent = text;
  el.hidden = false;
  const r = target.getBoundingClientRect();
  const w = el.offsetWidth;
  el.style.left = `${Math.max(8, Math.min(window.innerWidth - w - 8, r.left + r.width / 2 - w / 2))}px`;
  const below = r.bottom + 6 + el.offsetHeight < window.innerHeight;
  el.style.top = `${below ? r.bottom + 6 : r.top - 6 - el.offsetHeight}px`;
  current = target;
  warmUntil = Date.now() + WARM;
}

function schedule(target: HTMLElement, now: boolean): void {
  clearTimeout(timer);
  if (current === target) return;
  if (el) el.hidden = true;
  timer = window.setTimeout(() => show(target), now || Date.now() < warmUntil ? 0 : DELAY);
}

export function bindTips(): void {
  const tipOf = (e: Event) => (e.target as HTMLElement | null)?.closest?.<HTMLElement>('[data-tip]') ?? null;
  document.addEventListener('pointerover', (e) => {
    const t = tipOf(e);
    if (t) schedule(t, false);
    else hide();
  });
  document.addEventListener('focusin', (e) => {
    const t = tipOf(e);
    if (t && (e.target as HTMLElement).matches(':focus-visible')) schedule(t, true);
  });
  for (const type of ['pointerdown', 'keydown', 'focusout', 'wheel', 'dragstart']) document.addEventListener(type, hide, true);
}

/** Forgets which tips have retired. */
export function resetTips(): void {
  settings.tipsSeen = {};
  saveSettings();
}
