// A line of text asked for in a small dialog: the app's window.prompt. WKWebView has no prompt (wry answers null at once),
// so Range… and + New category… did nothing in the macOS app (R7 review, Nov 23).

export const asking = $state({ open: false, title: '', hint: '', value: '', show: false, copied: false, done: null as null | ((v: string | null) => void) });

/** Asks for one line of text; null when cancelled. */
export function askText(title: string, value = '', hint = ''): Promise<string | null> {
  asking.done?.(null);
  return new Promise((done) => Object.assign(asking, { open: true, title, hint, value, show: false, copied: false, done }));
}

/** Shows a line to copy (read-only, with Copy) until closed: a note went away before it could be copied (rc.1 test
 *  plan 1.9). */
export function showLine(title: string, value: string, hint = ''): Promise<void> {
  asking.done?.(null);
  return new Promise((done) => Object.assign(asking, { open: true, title, hint, value, show: true, copied: false, done: () => done() }));
}

/** Copies the line shown. */
export async function copyLine(): Promise<void> {
  try {
    await navigator.clipboard.writeText(asking.value);
    asking.copied = true;
  } catch {
    asking.copied = false;
  }
}

/** Ends the question with `v` (null: cancelled). */
export function answer(v: string | null): void {
  const d = asking.done;
  Object.assign(asking, { open: false, done: null });
  d?.(v);
}
