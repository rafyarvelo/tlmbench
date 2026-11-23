// A line of text asked for in a small dialog: the app's window.prompt. WKWebView has no prompt (wry answers null at once),
// so Range… and + New category… did nothing in the macOS app (R7 review, Nov 23).

export const asking = $state({ open: false, title: '', hint: '', value: '', done: null as null | ((v: string | null) => void) });

/** Asks for one line of text; null when cancelled. */
export function askText(title: string, value = '', hint = ''): Promise<string | null> {
  asking.done?.(null);
  return new Promise((done) => Object.assign(asking, { open: true, title, hint, value, done }));
}

/** Ends the question with `v` (null: cancelled). */
export function answer(v: string | null): void {
  const d = asking.done;
  Object.assign(asking, { open: false, done: null });
  d?.(v);
}
