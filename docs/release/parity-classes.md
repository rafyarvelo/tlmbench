# Parity classes: every difference from the prototype

Release step R3.0, Nov 15, 2026, on `main` at `0bce51c`. Classification only, no code changed. `just app-parity` gave
**213 differences in 23 scenes**; the screenshots it writes (`app/ui/tests/out/parity/<scene>_{proto,app}[_crop].png`,
not kept in the repo; re-run it to see them) were read side by side for the main view, start page, Connect, Import, tab
menu, ⋯ menu and table. The apple-design audit of the whole app ran in the same pass (`ui-audits.md`, Nov 15, R3.0).

Every difference is in one class:

- **Port**: the prototype's is better, or the app lacks it. Goes to R3.1 to R3.3, ranked by how visible it is.
- **App is better**: the app's is more mature. **Never ported back.** Every row is approved by the owner
  (B1 to B9 earlier, B10 to B16 on Nov 15) and is a ledger accepted difference (A-01 to A-11).
- **Noise**: the harness or the scene's data, not something a person sees. Listed so nobody chases it again.

## Port (ranked; the step and the ledger row each closes)

| # | Where | The prototype | The app | Step, ledger |
| --- | --- | --- | --- | --- |
| P1 | Start page (`start`) | A full page: three cards (Open folder, Open recording, Connect live source, each with what it opens), then two columns: recent sessions and recent recordings (with folder, channels and duration), live sources (saved and demo) and the folder's layouts | A small panel floating over the workspace with the Explorer and tab still showing behind; buttons, then short lists; no layouts | R3.2, D-ui-36, D-ui-01 (layouts on the start page). Keep the app's line "Nothing is written next to your data" |
| P2 | Explorer channel rows (`main`, `channel-hover`) | Quiet by default: name and tick; unit and pane on hover or selection | Every row shows its unit and "pane 3" all the time | R3.2, D-ui-10 |
| P3 | ⋯ menu (`more-menu`) | 8 items in three groups (All panes, Export, Window) | 19 items in five groups; at 900 px it is 600 px tall and scrolls. Since then the top bar has Export and the app a menu bar, so much of it is said twice | R3.1, D-ui-37. **Decided Nov 15 (owner): trim** to the prototype's groups plus what the app has nowhere else (recommendation: keep Open recording…, the view toggles that have no top-bar button, Window; drop Export (top bar) and the All-panes items the menu bar has) |
| P4 | Tab menu (`tab-menu`) | Says where the panes came from and that Save as… makes a layout; Save layout ⌘S and Revert changes always listed (greyed when there's nothing to save); the file chip numbered | No Save layout or Revert on a file tab; un-numbered chip | R3.1, S4.4-04 |
| P5 | Import dialog (`import`) | Sections When it opens (Panes, Channel prefix), Derived channels (pick recipes for this header), Categories by prefix with + New category and column selection; rates read as people say them ("daily · median interval 1 d"); a Detected line (jitter, gaps) | Time and rate, then the columns; rate as "0.04167 /hour (median step)" | R3.2, S5.1-10, S5.1-12, D-share-01; readable rates D-ui-38 |
| P5b | Explorer file right-click (owner, Nov 15) | — (not in the prototype) | Open with layout ▸, built with P5's profile starter layout | R3.2, D-ui-42 |
| P6 | Table (`table`) | An Event column; slower channels hold their last sample, dimmed when unchanged, and the status line says so; rate "1 kHz"; filter placeholders with an example ("e.g. > 1.5") | No Event column, nothing dimmed, "1000.000 Hz", "Filter" | R3.3, S5.2-13, S4.2-09; rate and placeholder D-ui-38 |
| P7 | Connect (`connect`, TCP) | Reconnect automatically; "Decodes as" with the descriptor's record size and fields; Save recordings to | The forms have no reconnect; the preview lists the descriptor's fields only | R3.2, S5.5-02, D-live-11 (core first) |
| P8 | — | — | — | Withdrawn: the overview's band is built and tested (S4.1-12, `t_overview`); the fake recording is smooth, so its band reads as a line. See Noise |
| P9 | Every menu opened with the mouse (`tab-menu`) | No focus ring on a click | The first item gets the keyboard focus ring when a menu opens from a click | R3.1, D-ui-39: ring only for keyboard (`:focus-visible`) |
| P10 | Time range in the top bar (`main`) | Fixed decimals (`00:00:00.000`), so it keeps its width | Decimals follow the zoom, so the text and what's beside it shift as you zoom | R3.1, D-ui-40. **Decided Nov 15 (owner):** the prototype's fixed width (tabular numbers), keeping the app's decimals inside it |
| P11 | The viewer's tab (README screenshot `app-viewer.png`) | — | A long layout name is cut at the left ("ltitude and …") | R3.1, D-ui-41 |
| P12 | Explorer ⋯ (`explorer-menu`) | "Recipes and profiles…" | "Recipes…" (the profiles tab isn't built) | R3.2, S5.1-12 |

Also Port, already ledger rows that R3.0 confirms (no screenshot shows them because the fake recording has no such data):
dragging a channel to a pane's edge or between rows (D-ui-17, D-ui-18; Nov 16 the scenes `drag-edge`, `drag-edge-right`, `drag-into`, `drag-gap`, `drag-pane-top` and `drag-pane-merge` hold a drag on both pages, and the app's outlines were matched: what is left is the panes' few pixels of size), event markers' tooltips (D-ui-11), Stop at several
categories (S5.2-15), Play's lost transport test (D-ui-13, D-ui-14), folder runs over sub-folders with a trends tab.

## App is better (never ported back)

| # | Where | What the app does | Why it's better | State |
| --- | --- | --- | --- | --- |
| B1 | Dialogs | One shared frame: centred, close button, scrim, form styles | Consistent placement (sheet E2, F1) | Approved Nov 14 |
| B2 | Top bar | Export menu (HTML, data, image) | One click to the frequent action (decision 6) | Approved Nov 14 |
| B3 | Explorer header | "Show only these" as an icon, with an empty state | Sheet A2 | Approved Nov 14 |
| B4 | Menu bar | File, Edit, View, Pane, Window, Help from one command table | Native, discoverable shortcuts (decision 8) | Approved Nov 14 |
| B5 | Pane header | Actions on hover, giving way by width; Chart and values' grip at the bottom | Calm, and works in narrow panes (decision 9, sheet B5) | Approved Nov 14 |
| B6 | Export to HTML | Folder or One file, raw when it fits, a size limit | Decision 7 | Approved Nov 14 |
| B7 | Viewer | Opens Locked, Unlock to rearrange | Sheet R12 | Approved Nov 15 |
| B8 | Settings | One Reset app… with a list | Sheet R3 | Approved Nov 15 |
| B9 | Tab strip, menus | ‹ › scroll buttons, full-width checked row, arrow keys in menus | Ledger A-01 to A-03 | Approved Nov 7 |
| B10 | Tab menu | Export file… and Move to new window | Features the prototype doesn't have (several windows, sharing a layout as a file) | Approved Nov 15 |
| B11 | Folder menu (`folder-menu`) | Refresh, with the folder watched, and more items (9 against 5) | A network share doesn't send changes; Refresh is the only way to see new files there (D-ui-02) | Approved Nov 15 |
| B12 | Alignment (`alignment`) | The file's name and slot number in the section | The design's numbered slots; you can tell which file you are aligning | Approved Nov 15 |
| B13 | Import dialog | A Category column per row, and names filled in | A category set per column without a prefix rule | Approved Nov 15 |
| B14 | Descriptor form (`descriptor`) | A Bits column | Bit fields, which the design accepted | Approved Nov 15 |
| B15 | Settings (`settings`) | Sections the prototype can't have: Plugins, Browser access, Cache, Shared configuration, Record live sessions to | Real features | Approved Nov 15 |
| B16 | Table bar | Go to A, Go to B and the table's height grip always in reach | Fewer clicks for the frequent move | Approved Nov 15 |

## Noise (not to chase)

- **Icon buttons** (rail, top bar, tooltip scene: 18 rows): the prototype sets `font-size: 0` on icon-only buttons; nothing
  shows.
- **Menus' padding** (every menu scene: "padding 0 → 4px"): the prototype pads an inner element, the app the menu itself;
  the items sit in the same place.
- **The selected pane**: a `box-shadow` in the prototype, a 2 px border in the app; they look the same. Pane widths ±10 px
  and heights (204 against 222 px) follow the fake recording's channel count and the stack's height.
- **Mapping artefacts**: the Explorer heading and the filter box (the harness maps the prototype's box to the app's inner
  input), the channel tick (the prototype's 24 px hit area against the app's 14 px box; both draw 14 px), header cells
  (the app's filter row sits inside the header cell), chips and labels whose width follows their text (`.bin` against
  `.tlm`).
- **Scene data**: the prototype's synthesized flight has events, limits and a 2 h envelope, and the fake core's recording
  has none; its folder has `.tlmbench/` and shared layouts (so the new-tab and Explorer menus list more), the app's fake
  folder doesn't; Connect opens on TCP in the prototype and File playback in the app; the descriptor's hex bytes and TOML
  come from the fake core; the import dialog's 970 px field is the app's one-row layout of a scene with fewer fields; table
  row counts follow the recording; the table filter strip shows because the scene typed a filter in the app.
- **The overview strip** draws a min/max band in both (S4.1-12); the fake recording's smooth sines make the band as thin as a line.
- **Item counts where the app has more features** (view menu Checks, pane menu, explorer menu separators): what the extra
  item does is in the ledger; the count isn't a difference to fix.
