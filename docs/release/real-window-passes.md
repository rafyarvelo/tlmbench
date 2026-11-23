# Real-window passes (R5)

One section per OS, run by the owner on the installed build, signed off at its end. Kept until R8, then summarised.
What headless tests can't see: Finder or Explorer drops, native dialogs, the system web view's text and fonts, drags
between windows, the GPU, high DPI, the installers. Each item names its ledger row; fill in `Result:` (Pass, or what
happened) and `Notes:` as on the test sheet. The agent ranks what you list and fixes it.

Before this: sheet sections Y2 to Y8 and Z1 to Z3 (`owner-test-sheet.md`) are still open; they come first and aren't
repeated here.

## macOS (WKWebView)

**Build:** the dry run's `.dmg` (run 45110269772) or a newer one; or, to check a fix, `cargo tauri build --bundles app`
then `open target/release/bundle/macos/TelemetryBench.app`. Installed in /Applications for M1 and M2; the rest can run
from either. A Retina screen, and an external non-Retina one if you have it.

### M1. First run from the installer (R4, D-ui-72)
Do: drag TelemetryBench from the `.dmg` to Applications, open it. Note which way macOS let you open it the first time:
right-click › Open, or System Settings › Privacy & Security › Open Anyway (the README and manual will say what you saw).
Then open a recording of yours from the start page.
Expect: the start page, then the recording's starter layout; no second warning on later opens.
Result:
Notes:

### M2. Opening files from the system (D-ui-72, X-win-01)
Do: double-click a `.tlm` in Finder with the app closed, then again while it runs. Right-click a `.csv` › Open With.
Drop a CSV, a `.tlm` and a folder on the window.
Expect: each opens in a tab of its own, the app in front; the folder opens as Open folder… does.
Result:
Notes:

### M3. Native dialogs (X-win-02)
Do: Open recording…, Open folder…, each export (data, image, HTML), Share….
Expect: each dialog opens in a sensible folder and what you pick is used.
Result:
Notes:

### M4. Drags inside the window (X-win-03)
Do: a channel from the Explorer to a pane and to a pane's edges; a chip between panes; a sidebar panel to the other
side; a tab within the strip; a pane by its number.
Expect: an outline shows where it lands; each lands there.
Result:
Notes:

### M5. Windows (X-win-04, D-arch-04)
Do: drag a tab out to a new window, then onto the other window's strip. With two windows open, quit and start again.
Then close the last window.
Expect: the tab moves both ways; both windows come back where they were; closing the last one quits.
Result:
Notes:

### M6. The menu bar and shortcuts (D-ui-32)
Do: look through each menu; press ⌘O, ⇧⌘O, ⌘S, ⌘Z and ⇧⌘Z (on the layout and in a text field), ⌘D, ⇧⌘N, ⌘. and ⌘,;
in a Table pane ⌘C, in the Explorer ⌘A.
Expect: every menu shows its shortcuts; each does its job; ⌘Z in a field undoes the typing, not the layout.
Result:
Notes:

### M7. How charts draw (S5.2-08, X-win-05, X-win-06, D-ui-30)
Do: a channel with warn and alarm limits; set a channel's line width, dashed, and step. Open the 100M-sample file (sheet B4) in
the app started from Terminal as `TLMBENCH_SLOW_BUILD=50 /Applications/TelemetryBench.app/Contents/MacOS/tlmbench-app`. Put the
prototype beside the app on the same data. Export a file as Light and open it beside the app.
Expect: dashed limit lines with yellow and red shading past them; each trace as set; hatching while the file builds,
then the min/max band on a spiky channel; text and fonts as crisp as the prototype; the viewer's band and line match the
app's at the same zoom.
Result:
Notes:

### M8. Motion (D-ui-04)
Do: Fit, box zoom and Back at a 2× and a 200× change of span; open a menu and a dialog. Then System Settings ›
Accessibility › Display › Reduce motion on, and again.
Expect: zooms glide and settle without overshoot, menus grow from their trigger, dialogs rise; with Reduce motion,
nothing animates.
Result:
Notes:

### M9. Menus and focus (A-02, the audit's real-window items)
Do: open a pane's view menu; click a menu item, then press Tab. Make the window 900 px tall and open a pane's ⋯. Look
at the start page with only a folder open, and at a narrow width. Open a file's right-click › Open with layout ▸.
Expect: the checked row highlighted across the full width; no focus ring after a click, one after Tab; the ⋯ menu fits
or scrolls; the greyed Layouts rows read as "open a file first"; Open with layout reads as a submenu.
Result:
Notes:

### M10. Code widgets (X-win-07)
Do: a Gauges pane with a code widget (Settings › Widgets › Edit…, or a shared one after trusting it).
Expect: it runs and updates with the data.
Result:
Notes:

### M11. Live (X-win-08)
Do: tail a growing `.tlm`; connect a TCP or a UDP source with a trigger and a live check; open the desktop window's
browser address in Safari.
Expect: the charts follow; the trigger fires; the check goes red and green; the browser tab shows the same session.
Result:
Notes:

### M12. Read-only (X-win-09)
Do: `just ro-test-mac`, then use the app on the mounted image; then on a read-only network share (SMB) if you have one.
Expect: everything works; the tab menu says "Folder is read-only" and offers Export file…; Share… opens as Export
configuration; nothing is written next to the data.
Result:
Notes:

### M13. Without WebGL2 (Canvas 2D)
Do: the app's window always has WebGL2, so this is the HTML export and browser-only mode in Safari: export a large file
as HTML, open it, and add `?renderer=2d` to the address (it forces the Canvas 2D renderer); zoom and pan. The same in
browser-only mode (`just browser-site`, served locally).
Expect: the same charts as with WebGL2; zoom and pan stay smooth.
Result:
Notes:

### M14. The look, last (apple-design audit on the installed build)
Do: walk the app in light and dark (start page, Explorer, a few panes, a dialog, Settings, the Report tab).
Expect: nothing clipped, blurry or off the type scale; list anything that looks wrong, however small. The agent
also audits screenshots of this build.
Result:
Notes:

**Signed off (macOS):** _date, build_

## Windows (WebView2)

Not started: the owner has no Windows machine or VM yet (Nov 23). Until one is found, everything here stays unverified
on Windows (X-win-10, the NSIS and MSI installers beyond CI's smoke test) and the release notes say so.

## Linux (WebKitGTK)

Not started: the owner has no Linux machine or VM yet (Nov 23). As for Windows: unverified (X-win-10, the AppImage and
`.deb` beyond CI's smoke test) and said in the release notes.
