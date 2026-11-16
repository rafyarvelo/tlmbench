//! Plugins in the app: the folders they are read from, what Settings lists, and the trust rules (see
//! `tlmbench_plugin`). Plugin files are found in `plugins/` in the app's data folder (yours) and in the open folder's
//! `.tlmbench/plugins/` (shared; only sandboxed WASM plugins load from there). Both are watched: a plugin built again,
//! added or removed is read again by itself (hot reload), and the windows are told (`plugins-changed`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tlmbench_plugin::Found;

#[derive(Default)]
pub struct State {
    found: Mutex<Vec<Found>>,
    /// The open folder's `.tlmbench/plugins/`, when it has one.
    shared: Mutex<Option<PathBuf>>,
    /// File names of the native plugins turned on.
    native_on: Mutex<Vec<String>>,
    /// The shared plugins folders (`.tlmbench/plugins`, by path) the person trusts.
    trusted: Mutex<Vec<String>>,
    /// What watches the plugins folders, and the folders it watches.
    watcher: Mutex<Option<(notify::RecommendedWatcher, Vec<PathBuf>)>>,
}

/// Whether a changed path is a plugin's file (a WASM component or a native library): the only changes that reload.
pub fn is_plugin_file(p: &std::path::Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["wasm", "dylib", "so", "dll"].contains(&e.to_ascii_lowercase().as_str()))
}

impl State {
    pub fn list(&self) -> Vec<Found> {
        self.found.lock().unwrap().clone()
    }

    /// Points at the open folder's shared plugins; true when that changed.
    pub fn set_shared(&self, dir: Option<PathBuf>) -> bool {
        let mut s = self.shared.lock().unwrap();
        let dir = dir.filter(|d| d.is_dir());
        let changed = *s != dir;
        *s = dir;
        changed
    }

    pub fn set_native_on(&self, on: Vec<String>) {
        *self.native_on.lock().unwrap() = on;
    }

    pub fn set_trusted(&self, folders: Vec<String>) {
        *self.trusted.lock().unwrap() = folders;
    }

    /// Watches `own` and the shared folder (again when either changed): a plugin file that changes there is read again
    /// half a second after the last change, and every window hears `plugins-changed`.
    pub fn watch<R: Runtime>(self: &Arc<Self>, app: &AppHandle<R>, own: Option<PathBuf>) {
        let dirs: Vec<PathBuf> =
            own.into_iter().chain(self.shared.lock().unwrap().iter().cloned()).filter(|d| d.is_dir()).collect();
        let mut slot = self.watcher.lock().unwrap();
        if slot.as_ref().is_some_and(|(_, d)| *d == dirs) {
            return;
        }
        *slot = None;
        if dirs.is_empty() {
            return;
        }
        let pending = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (me, app2, own2) = (Arc::downgrade(self), app.clone(), own_dir(app));
        let Ok(mut w) = notify::recommended_watcher(move |ev: notify::Result<notify::Event>| {
            let Ok(ev) = ev else { return };
            if matches!(ev.kind, notify::EventKind::Access(_)) || !ev.paths.iter().any(|p| is_plugin_file(p)) {
                return;
            }
            // a build writes a file in several steps: one reload for the burst
            if pending.swap(true, std::sync::atomic::Ordering::AcqRel) {
                return;
            }
            let (me, app, own, pending) = (me.clone(), app2.clone(), own2.clone(), pending.clone());
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));
                pending.store(false, std::sync::atomic::Ordering::Release);
                if let Some(state) = me.upgrade() {
                    let found = state.scan(own);
                    let _ = app.emit("plugins-changed", found.len());
                }
            });
        }) else {
            return;
        };
        for d in &dirs {
            let _ = w.watch(d, RecursiveMode::NonRecursive);
        }
        *slot = Some((w, dirs));
    }

    pub fn native_on(&self) -> Vec<String> {
        self.native_on.lock().unwrap().clone()
    }

    /// Reads the folders again and loads what is allowed. Compiling WASM takes a moment: run this off the UI thread.
    pub fn scan(&self, own: Option<PathBuf>) -> Vec<Found> {
        let shared: Vec<PathBuf> = self.shared.lock().unwrap().iter().cloned().collect();
        let found =
            tlmbench_plugin::install(own.as_deref(), &shared, &self.trusted.lock().unwrap(), &self.native_on()).found;
        *self.found.lock().unwrap() = found.clone();
        found
    }
}

/// Your plugins folder.
pub fn own_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("plugins"))
}

/// A list of strings under `key` in the shared settings file (`nativePlugins`, `trustedPluginFolders`).
fn list_from_settings(text: &str, key: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v[key].as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()))
        .unwrap_or_default()
}

/// At start: reads which native plugins are on from the saved settings and loads the plugins, off the UI thread.
pub fn start<R: Runtime>(app: &AppHandle<R>, settings: Option<String>, state: Arc<State>) {
    let list = |key: &str| settings.as_deref().map(|t| list_from_settings(t, key)).unwrap_or_default();
    state.set_native_on(list("nativePlugins"));
    state.set_trusted(list("trustedPluginFolders"));
    let own = own_dir(app);
    if let Some(d) = &own {
        // yours is there to watch (and for plugins to be dropped into) from the first start
        let _ = std::fs::create_dir_all(d);
    }
    state.watch(app, own.clone());
    std::thread::spawn(move || {
        state.scan(own);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plugin_files_reload() {
        for (p, yes) in [
            ("a/dec.wasm", true),
            ("a/LIB.DYLIB", true),
            ("x.so", true),
            ("x.dll", true),
            ("a/notes.md", false),
            ("a/dec.wasm.tmp", false),
            ("a/plugins", false),
        ] {
            assert_eq!(is_plugin_file(std::path::Path::new(p)), yes, "{p}");
        }
    }

    #[test]
    fn native_plugins_turned_on_come_from_the_saved_settings() {
        let text = r#"{"a":1,"nativePlugins":["x.dylib","y.so"],"trustedPluginFolders":["/d/.tlmbench/plugins"]}"#;
        assert_eq!(list_from_settings(text, "nativePlugins"), ["x.dylib", "y.so"]);
        assert_eq!(list_from_settings(text, "trustedPluginFolders"), ["/d/.tlmbench/plugins"]);
        assert!(list_from_settings("{}", "nativePlugins").is_empty());
        assert!(list_from_settings("not json", "nativePlugins").is_empty());
    }
}
