// Widgets of your own: the editor (a list of your widgets and a form for the one picked, with a live preview, or the
// whole list as JSON), the list kept in settings, and the trust prompt for code from a shared folder.

import { setSetting, settings } from './settings.svelte';
import { CODE_WIDGET_EXAMPLE, type CustomWidget, WIDGET_EXAMPLE } from './widgets';

/** A widget in the editor: what is saved, with its id once it has one (a widget keeps its id when renamed, so the panes
 *  that show it keep it). */
export type Draft = Omit<CustomWidget, 'id'> & { id?: string };

export const widgetDlg = $state({ open: false, list: [] as Draft[], sel: 0, json: false, text: '', error: '' });
export const trust = $state({ id: null as string | null });

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));
const noId = (list: Draft[]) => list.map(({ id: _id, ...w }) => w);

/** The list as JSON (without ids): the editor's "Edit as JSON". */
export const widgetsText = (list: Draft[] = widgetDlg.list): string => JSON.stringify(noId(list), null, 2);

/** The list in a JSON text, or why it can't be read. */
export function parseWidgets(text: string): { list: Draft[] | null; error: string } {
  let arr: unknown;
  try {
    arr = JSON.parse(text);
  } catch (e) {
    return { list: null, error: `That isn't valid JSON: ${(e as Error).message}` };
  }
  const list = (Array.isArray(arr) ? arr : [arr]) as Draft[];
  if (list.some((w) => !w || typeof w !== 'object')) return { list: null, error: 'Each widget needs a name and an svg (or code).' };
  const bad = problems(list).find(Boolean);
  return bad ? { list: null, error: bad } : { list, error: '' };
}

/** What stops each widget from being saved ('' when nothing): a name, unique, and an SVG or code. */
export function problems(list: Draft[]): string[] {
  const seen = new Map<string, number>();
  for (const w of list) if (typeof w.name === 'string' && w.name.trim()) seen.set(w.name.trim(), (seen.get(w.name.trim()) ?? 0) + 1);
  return list.map((w) => {
    const name = typeof w.name === 'string' ? w.name.trim() : '';
    if (!name || (typeof w.svg !== 'string' && typeof w.code !== 'string')) return 'Each widget needs a name and an svg (or code).';
    if ((seen.get(name) ?? 0) > 1) return `Two widgets are named “${name}”.`;
    if (typeof w.code === 'string' ? !w.code.trim() : !w.svg?.trim()) return `“${name}” has no ${typeof w.code === 'string' ? 'code' : 'SVG'}.`;
    return '';
  });
}

/** Opens the editor on your widgets (and the folder's, read-only), on the one with id `id` when given. */
export function openWidgetEditor(id?: string): void {
  const list: Draft[] = clone(settings.customWidgets);
  Object.assign(widgetDlg, { open: true, list, sel: Math.max(0, list.findIndex((w) => w.id === id)), json: false, text: '', error: '' });
}
export const closeWidgetEditor = (): void => void (widgetDlg.open = false);

/** A name not yet in the list: `base`, else `base 2`, `base 3`… */
function freeName(base: string): string {
  const have = new Set(widgetDlg.list.map((w) => w.name));
  let name = base;
  for (let n = 2; have.has(name); n++) name = `${base} ${n}`;
  return name;
}

/** Adds a new widget filled in with a working example (D-ui-59), an SVG one or a code one, and picks it. */
export function addWidgetExample(kind: 'svg' | 'code'): void {
  const base = kind === 'code' ? CODE_WIDGET_EXAMPLE : WIDGET_EXAMPLE;
  widgetDlg.list.push({ ...clone(base), name: freeName(base.name) });
  widgetDlg.sel = widgetDlg.list.length - 1;
}

/** A copy of widget `k` of your own (a shared one becomes yours), picked. */
export function duplicateWidget(k: number): void {
  const w = widgetDlg.list[k];
  if (!w) return;
  const { id: _id, shared: _s, by: _b, ...rest } = clone($state.snapshot(w) as Draft);
  widgetDlg.list.splice(k + 1, 0, { ...rest, name: freeName(`${w.name} copy`) });
  widgetDlg.sel = k + 1;
}

export function deleteWidget(k: number): void {
  if (!widgetDlg.list[k]) return;
  widgetDlg.list.splice(k, 1);
  widgetDlg.sel = Math.max(0, Math.min(widgetDlg.sel, widgetDlg.list.length - 1));
}

/** Switches the widget between an SVG and code; what it had for the other kind is kept until Save. */
export function setKind(k: number, kind: 'svg' | 'code'): void {
  const w = widgetDlg.list[k] as Draft & { _svg?: string; _code?: string };
  if (!w || (kind === 'code') === (typeof w.code === 'string')) return;
  if (kind === 'code') {
    w._svg = w.svg;
    delete w.svg;
    w.code = w._code ?? CODE_WIDGET_EXAMPLE.code;
  } else {
    w._code = w.code;
    delete w.code;
    w.svg = w._svg ?? WIDGET_EXAMPLE.svg;
  }
}

/** Between the form and "Edit as JSON". A JSON mistake keeps the JSON open, with the mistake named. */
export function toggleJson(): boolean {
  if (!widgetDlg.json) {
    Object.assign(widgetDlg, { json: true, text: widgetsText(), error: '' });
    return true;
  }
  const { list, error } = parseWidgets(widgetDlg.text);
  widgetDlg.error = error;
  if (!list) return false;
  // a widget keeps its id by name, so the panes showing it keep it
  const ids = new Map(widgetDlg.list.filter((w) => w.id).map((w) => [w.name, w.id!]));
  widgetDlg.list = list.map((w) => (ids.has(w.name) ? { ...w, id: ids.get(w.name) } : w));
  widgetDlg.sel = Math.min(widgetDlg.sel, Math.max(0, list.length - 1));
  widgetDlg.json = false;
  return true;
}

/** What Save stores: each widget tidied (only its kind's fields, empty rows dropped) with a stable id. */
function tidy(list: Draft[]): CustomWidget[] {
  const used = new Set<string>();
  return list.map((d, k) => {
    const { id: had, _svg: _a, _code: _b, ...w } = clone(d) as Draft & { _svg?: string; _code?: string };
    w.name = w.name.trim();
    if (typeof w.code === 'string') {
      delete w.svg;
      delete w.angle;
      w.inputs = (w.inputs ?? []).filter((i) => i.role?.trim()).map((i) => ({ role: i.role.trim(), ...(i.label?.trim() ? { label: i.label.trim() } : {}) }));
      w.params = (w.params ?? []).filter((p) => p.name?.trim()).map((p) => ({ ...p, name: p.name.trim() }));
      if (!w.inputs.length) delete w.inputs;
      if (!w.params.length) delete w.params;
    } else {
      delete w.code;
      delete w.inputs;
      delete w.params;
    }
    if (!w.icon?.trim()) delete w.icon;
    let id = had ?? (w.shared ? `sw_${w.name}` : `cw_${(w.name || String(k)).toLowerCase().replace(/[^a-z0-9]+/g, '_')}`);
    while (used.has(id)) id += '_';
    used.add(id);
    return { id, ...w };
  });
}

/** Saves the list (from the JSON when that is open) and closes the editor; false when something stops it. */
export function saveWidgets(): boolean {
  if (widgetDlg.json && !toggleJson()) return false;
  const bad = problems(widgetDlg.list).find(Boolean);
  widgetDlg.error = bad ?? '';
  if (bad) return false;
  setSetting('customWidgets', tidy(widgetDlg.list));
  widgetDlg.open = false;
  return true;
}

/** Runs the code widgets from `folder` from now on. */
export const trustSource = (folder: string): void => setSetting('trustedWidgetSources', [...new Set([...settings.trustedWidgetSources, folder])]);
