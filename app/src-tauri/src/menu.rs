//! The native menu bar (File, Edit, View, Pane, Window, Help), built from the command table the window sends
//! (`app/ui/src/commands.ts`). A click emits `menu-action` with the command's id to the window in front; the page runs the
//! same action as the shortcut. The page sends each item's state (enabled, checked) as it changes.
//!
//! Accelerators are only ever combinations with ⌘ (Ctrl) or ⌥, so a text field keeps every plain key. Cut, Copy, Paste and
//! Select All are the system's own items, so text fields work as in any app.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Deserialize;
use tauri::menu::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

/// One command of the table.
#[derive(Deserialize)]
pub struct Entry {
    id: String,
    menu: String,
    label: String,
    accel: Option<String>,
    #[serde(default)]
    sep: bool,
    #[serde(default)]
    check: bool,
}

/// What a page says about an item right now.
#[derive(Deserialize)]
pub struct Update {
    id: String,
    enabled: bool,
    checked: bool,
}

type Setter = Box<dyn Fn(bool, bool) + Send + Sync>;

/// The items by command id, to change their state.
#[derive(Default)]
pub struct MenuState(Mutex<HashMap<String, Setter>>);

const ORDER: [&str; 6] = ["File", "Edit", "View", "Pane", "Window", "Help"];

/// The menu names in `entries` that the bar has no place for (their items would be dropped).
pub fn unknown_menus(entries: &[Entry]) -> Vec<String> {
    let mut bad: Vec<String> =
        entries.iter().filter(|e| !ORDER.contains(&e.menu.as_str())).map(|e| e.menu.clone()).collect();
    bad.dedup();
    bad
}

fn err(e: tauri::Error) -> String {
    e.to_string()
}

/// The menu for `entries`: the app menu on the Mac, then one submenu per name in `ORDER`.
pub fn build<R: Runtime>(app: &AppHandle<R>, entries: &[Entry]) -> Result<(Menu<R>, HashMap<String, Setter>), String> {
    if let Some(bad) = unknown_menus(entries).first() {
        return Err(format!("the menu bar has no menu called {bad:?}"));
    }
    let menu = Menu::new(app).map_err(err)?;
    let mut setters: HashMap<String, Setter> = HashMap::new();
    #[cfg(target_os = "macos")]
    {
        let name = app.package_info().name.clone();
        let sub = Submenu::new(app, &name, true).map_err(err)?;
        sub.append(&about(app)?).map_err(err)?;
        sub.append(&PredefinedMenuItem::separator(app).map_err(err)?).map_err(err)?;
        sub.append(&PredefinedMenuItem::services(app, None).map_err(err)?).map_err(err)?;
        sub.append(&PredefinedMenuItem::separator(app).map_err(err)?).map_err(err)?;
        sub.append(&PredefinedMenuItem::hide(app, None).map_err(err)?).map_err(err)?;
        sub.append(&PredefinedMenuItem::hide_others(app, None).map_err(err)?).map_err(err)?;
        sub.append(&PredefinedMenuItem::show_all(app, None).map_err(err)?).map_err(err)?;
        sub.append(&PredefinedMenuItem::separator(app).map_err(err)?).map_err(err)?;
        sub.append(&PredefinedMenuItem::quit(app, None).map_err(err)?).map_err(err)?;
        menu.append(&sub).map_err(err)?;
    }
    for name in ORDER {
        let sub = Submenu::new(app, name, true).map_err(err)?;
        let mine: Vec<&Entry> = entries.iter().filter(|e| e.menu == name).collect();
        for (n, e) in mine.iter().enumerate() {
            if e.sep && n > 0 {
                sub.append(&PredefinedMenuItem::separator(app).map_err(err)?).map_err(err)?;
            }
            let accel = e.accel.as_deref();
            if e.check {
                let item = CheckMenuItem::with_id(app, &e.id, &e.label, true, false, accel).map_err(err)?;
                sub.append(&item).map_err(err)?;
                setters.insert(
                    e.id.clone(),
                    Box::new(move |enabled, checked| {
                        let _ = item.set_enabled(enabled);
                        let _ = item.set_checked(checked);
                    }),
                );
            } else {
                let item = MenuItem::with_id(app, &e.id, &e.label, true, accel).map_err(err)?;
                sub.append(&item).map_err(err)?;
                setters.insert(
                    e.id.clone(),
                    Box::new(move |enabled, _| {
                        let _ = item.set_enabled(enabled);
                    }),
                );
            }
            // the system's own items sit with the ones they belong to
            if name == "Edit" && e.id == "redo" {
                sub.append(&PredefinedMenuItem::separator(app).map_err(err)?).map_err(err)?;
                sub.append(&PredefinedMenuItem::cut(app, None).map_err(err)?).map_err(err)?;
                sub.append(&PredefinedMenuItem::copy(app, None).map_err(err)?).map_err(err)?;
                sub.append(&PredefinedMenuItem::paste(app, None).map_err(err)?).map_err(err)?;
                sub.append(&PredefinedMenuItem::select_all(app, None).map_err(err)?).map_err(err)?;
            }
        }
        if name == "File" {
            sub.append(&PredefinedMenuItem::separator(app).map_err(err)?).map_err(err)?;
            sub.append(&PredefinedMenuItem::close_window(app, None).map_err(err)?).map_err(err)?;
        }
        if name == "Window" {
            sub.append(&PredefinedMenuItem::separator(app).map_err(err)?).map_err(err)?;
            sub.append(&PredefinedMenuItem::minimize(app, None).map_err(err)?).map_err(err)?;
            sub.append(&PredefinedMenuItem::maximize(app, None).map_err(err)?).map_err(err)?;
            sub.append(&PredefinedMenuItem::fullscreen(app, None).map_err(err)?).map_err(err)?;
        }
        // Windows and Linux have no app menu: About ends Help
        #[cfg(not(target_os = "macos"))]
        if name == "Help" {
            sub.append(&PredefinedMenuItem::separator(app).map_err(err)?).map_err(err)?;
            sub.append(&about(app)?).map_err(err)?;
        }
        menu.append(&sub).map_err(err)?;
        #[cfg(target_os = "macos")]
        match name {
            "Window" => sub.set_as_windows_menu_for_nsapp().map_err(err)?,
            "Help" => sub.set_as_help_menu_for_nsapp().map_err(err)?,
            _ => {}
        }
    }
    Ok((menu, setters))
}

/// Where releases are published, and what About says of the licences (R7: the version, the licence, the notices).
const RELEASES: &str = "https://github.com/rafyarvelo/tlmbench/releases";
const NOTICE: &str = "Licensed under the MIT License or the Apache License 2.0, at your choice. The open-source software it is built with, and their \
licences, are listed in THIRD_PARTY_LICENSES.md beside the app.";

/// About TelemetryBench: the system's own window, with the version, the licence and where the notices are.
fn about<R: Runtime>(app: &AppHandle<R>) -> Result<PredefinedMenuItem<R>, String> {
    let info = app.package_info();
    let meta = AboutMetadata {
        name: Some(info.name.clone()),
        version: Some(info.version.to_string()),
        copyright: Some("© TelemetryBench contributors".into()),
        license: Some("MIT OR Apache-2.0".into()),
        website: Some(RELEASES.into()),
        website_label: Some("Releases".into()),
        comments: Some(NOTICE.into()),
        credits: Some(format!("{NOTICE}\n\nReleases: {RELEASES}")),
        ..Default::default()
    };
    PredefinedMenuItem::about(app, None, Some(meta)).map_err(err)
}

/// The window sends the command table once; the menu bar is built from it.
#[tauri::command]
pub fn menu_set<R: Runtime>(app: AppHandle<R>, entries: Vec<Entry>, state: State<'_, MenuState>) -> Result<(), String> {
    let (menu, setters) = build(&app, &entries)?;
    app.set_menu(menu).map_err(err)?;
    *state.0.lock().map_err(|e| e.to_string())? = setters;
    Ok(())
}

/// Items enabled or checked, as the page has them now.
#[tauri::command]
pub fn menu_state(updates: Vec<Update>, state: State<'_, MenuState>) -> Result<(), String> {
    let set = state.0.lock().map_err(|e| e.to_string())?;
    for u in updates {
        if let Some(f) = set.get(&u.id) {
            f(u.enabled, u.checked);
        }
    }
    Ok(())
}

/// A click: the command goes to the window in front (the first one if none says it is).
pub fn clicked<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let Some(state) = app.try_state::<MenuState>() else { return };
    if !state.0.lock().is_ok_and(|s| s.contains_key(id)) {
        return;
    }
    let windows = app.webview_windows();
    let target = windows.values().find(|w| w.is_focused().unwrap_or(false)).or_else(|| windows.values().next());
    if let Some(w) = target {
        let _ = w.emit("menu-action", id);
    }
}
