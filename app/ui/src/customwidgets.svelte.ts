// Widgets of your own: the editor (a JSON list with a live preview), the list kept in settings, and the trust prompt
// for code from a shared folder.

import { setSetting, settings } from './settings.svelte';
import { type CustomWidget, WIDGET_EXAMPLE } from './widgets';

export const widgetDlg = $state({ open: false, text: '', error: '' });
export const trust = $state({ id: null as string | null });

/** The widgets of your own, as the editor's text (without ids; the example when there are none). */
export const widgetsText = (): string => JSON.stringify(settings.customWidgets.length ? settings.customWidgets.map(({ id: _id, ...w }) => w) : [WIDGET_EXAMPLE], null, 2);

/** The list in the editor's text, or why it can't be read. */
export function parseWidgets(text: string): { list: Omit<CustomWidget, 'id'>[] | null; error: string } {
  let arr: unknown;
  try {
    arr = JSON.parse(text);
  } catch (e) {
    return { list: null, error: `That isn't valid JSON: ${(e as Error).message}` };
  }
  const list = (Array.isArray(arr) ? arr : [arr]) as Omit<CustomWidget, 'id'>[];
  if (list.some((w) => !w || typeof w.name !== 'string' || !w.name || (typeof w.svg !== 'string' && typeof w.code !== 'string'))) return { list: null, error: 'Each widget needs a name and an svg (or code).' };
  return { list, error: '' };
}

export const openWidgetEditor = (): void => void Object.assign(widgetDlg, { open: true, text: widgetsText(), error: '' });
export const closeWidgetEditor = (): void => void (widgetDlg.open = false);

export function saveWidgets(text: string): boolean {
  const { list, error } = parseWidgets(text);
  widgetDlg.error = error;
  if (!list) return false;
  const used = new Set<string>();
  setSetting(
    'customWidgets',
    list.map((w, k) => {
      let id = w.shared ? `sw_${w.name}` : `cw_${(w.name || String(k)).toLowerCase().replace(/[^a-z0-9]+/g, '_')}`;
      while (used.has(id)) id += '_';
      used.add(id);
      return { id, ...w };
    }),
  );
  widgetDlg.open = false;
  return true;
}

/** Runs the code widgets from `folder` from now on. */
export const trustSource = (folder: string): void => setSetting('trustedWidgetSources', [...new Set([...settings.trustedWidgetSources, folder])]);
