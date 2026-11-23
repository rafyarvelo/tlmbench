# Changing the UI

The UI (`app/ui/src`) is Svelte 5 and TypeScript, built by Vite. It is the same code in the desktop window, a browser tab
against the daemon, an export's viewer and browser-only mode, so a control must not assume the desktop (`ui.browserOnly`
and the viewer's build flags say what is missing). The v1 UI is **frozen**: only bugs are fixed, and a new idea goes to
[`docs/post-v1.md`](../post-v1.md). These are the conventions a fix or a later change follows; the owner cares about them.

## Behaviour

- **Production-like.** Everything configurable is saved in settings (`settings.svelte.ts`: add the key to `Settings` and
  `DEFAULTS`; [the settings reference](../reference/settings.md) is generated from them). The **reset** lists in
  `settings.svelte.ts` say what Reset UI layout, Reset tips and Reset app clear.
- **Calm, with few clicks.** Controls appear on selection, hover or focus, not all at once. A frequent action is one
  click; a rarer one goes in a ⋯ or right-click menu.
- **Shortcuts** live in one table, `commands.ts`: a combination with ⌘/Ctrl or ⌥ is bound by the native menu and shown beside
  the item; a single key (E, L, T, F, Esc) is handled by the keyboard handler only, because the menu would stop that key typing in
  every field. A test checks that every command has a menu item and an action, and
  [the shortcuts reference](../reference/shortcuts.md) is generated from the table.
- **No `window.prompt`, no synchronous `confirm`:** the app's webview has neither. Use `askText` (`ask.svelte.ts`) and `await`
  the confirm.
- **Never spread a large array into arguments** (`push(...bytes)`): past about 100 KB the stack overflows.
- **Read-only first.** A control that would write next to the data is wrong: only Share and exports write there.

## Look

- **Tokens.** Font sizes 11, 11.5, 12, 13, 15 and 26 px (20 only inside widget SVGs); radii 4, 8, 12 and round; monospace only for
  numbers, times and units. `style.css` holds the colours as variables for light and dark.
- **Tooltips** use `data-tip` (a `title` is converted); short, and they retire after three uses.
- **Copy** is plain and short. Say what a control does, in sentence case.
- **Icons** come from `icons.ts`.

## Adding or changing a control

1. Find where it lives (`app/ui/src/shell/*.svelte`; the rail, top bar and panels are there) and reuse a neighbour's pattern.
2. If the core must answer something new, add the command in `app/src-tauri/src/main.rs`, its fake in
   `app/ui/tests/fakecore.js` and, for the viewer or browser-only mode, the call in `crates/web` and `wasmcore.ts`.
3. Write the test in the area's `t_*.js` (a new file if there is none) and add a line to the table in
   [`app/ui/tests/README.md`](../../app/ui/tests/README.md). Tests check behaviour and state; check a visual change with a
   screenshot by eye.
4. If the control is visible in the manual, update the chapter, and retake its screenshot (`node app/ui/tests/shots.js OUTDIR
   --manual id`). Run `just manual`: it fails on a broken link or a missing image.
5. If it changes what the design says, add a note to [`docs/follow-ups.md`](../follow-ups.md).
6. `just app-test`, `npm run typecheck` (in `app/ui`) and `just lint` pass.
