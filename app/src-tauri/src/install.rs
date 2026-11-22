//! What the installed app does for the system around it (R4): files the OS asks it to open, and the command-line tool
//! it ships beside itself.
//!
//! Opening: a double-click on a `.tlm` (or Open With) reaches the app as macOS's Opened event, or on Windows and Linux
//! as the path on the command line of a second launch, which hands it to the running app and quits (the single-instance
//! plugin). Paths wait in a queue until the main window has started and takes them (`opened_take`); after that each
//! batch goes to it as an `open-paths` event.
//!
//! The command-line tool: the bundle has `tlmbench` beside the app's own executable. "Install command-line tool…" links
//! it where a shell finds it, with no password and without running another program: `~/.local/bin/tlmbench` on macOS and
//! Linux (owner, Nov 22), the app's folder on the user's PATH on Windows.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

/// Files asked for before the window could take them, and whether it has.
#[derive(Default)]
pub struct Opening {
    queue: Mutex<(Vec<String>, bool)>,
}

/// The files among a launch's arguments (the first is the program): flags are left out, a relative path is read from
/// `cwd`, and only files that exist count.
pub fn file_args(args: impl IntoIterator<Item = String>, cwd: &Path) -> Vec<String> {
    args.into_iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .map(|a| {
            let p = PathBuf::from(&a);
            if p.is_relative() { cwd.join(p) } else { p }
        })
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Hands `paths` to the main window, or keeps them until it starts.
pub fn deliver<R: Runtime>(app: &AppHandle<R>, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    let opening = app.state::<Opening>();
    let mut q = opening.queue.lock().unwrap();
    if q.1 {
        let _ = app.emit_to("main", "open-paths", paths);
    } else {
        q.0.extend(paths);
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// The main window has started: the files asked for so far, and from now on `open-paths` events.
#[tauri::command]
pub fn opened_take(opening: State<'_, Opening>) -> Vec<String> {
    let mut q = opening.queue.lock().unwrap();
    q.1 = true;
    std::mem::take(&mut q.0)
}

/// What Install command-line tool… did.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliInstalled {
    /// Where the command is now (the link, or on Windows the folder added to PATH).
    pub at: String,
    /// A new terminal finds it: its folder is on PATH.
    pub on_path: bool,
    /// When it isn't: the line that puts it there, for the shell's profile.
    pub path_line: Option<String>,
    /// The profile file that line goes in.
    pub profile: Option<String>,
}

/// The `tlmbench` beside the app's executable, if this build has one (a development build doesn't).
fn bundled_cli() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = if cfg!(windows) { "tlmbench.exe" } else { "tlmbench" };
    let cli = exe.parent().map(|d| d.join(name)).filter(|p| p.is_file());
    cli.ok_or_else(|| "This build has no command-line tool beside the app (a development build): build it with `cargo build --release -p tlmbench`.".into())
}

fn on_path(dir: &Path) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d == dir))
}

/// Links `link` to `target`: a link already there (an older install) is replaced; anything else is left alone.
#[cfg(unix)]
pub fn link_cli(target: &Path, link: &Path) -> Result<(), String> {
    if let Some(dir) = link.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("can't make {}: {e}", dir.display()))?;
    }
    match std::fs::symlink_metadata(link) {
        Ok(m) if m.file_type().is_symlink() => {
            std::fs::remove_file(link).map_err(|e| format!("can't replace {}: {e}", link.display()))?
        }
        Ok(_) => return Err(format!("{} is another program: remove it first, then install again.", link.display())),
        Err(_) => {}
    }
    std::os::unix::fs::symlink(target, link).map_err(|e| format!("can't link {}: {e}", link.display()))
}

#[cfg(unix)]
#[tauri::command]
pub fn cli_install() -> Result<CliInstalled, String> {
    let cli = bundled_cli()?;
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or("no home folder")?;
    let dir = home.join(".local/bin");
    let link = dir.join("tlmbench");
    link_cli(&cli, &link)?;
    let found = on_path(&dir);
    let profile = if cfg!(target_os = "macos") { "~/.zprofile" } else { "~/.profile" };
    Ok(CliInstalled {
        at: link.to_string_lossy().into_owned(),
        on_path: found,
        path_line: (!found).then(|| r#"export PATH="$HOME/.local/bin:$PATH""#.to_owned()),
        profile: (!found).then(|| profile.to_owned()),
    })
}

/// Windows: the app's folder (where `tlmbench.exe` is) goes on the user's PATH, and running programs are told.
#[cfg(windows)]
#[tauri::command]
pub fn cli_install() -> Result<CliInstalled, String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, RegType};
    use winreg::{RegKey, RegValue};
    let cli = bundled_cli()?;
    let dir = cli.parent().ok_or("no folder")?.to_path_buf();
    let env = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|e| format!("can't open the user's environment: {e}"))?;
    let now: String = env.get_value("Path").unwrap_or_default();
    let has = std::env::split_paths(&now).any(|d| d == dir);
    if !has {
        let next = if now.is_empty() { dir.display().to_string() } else { format!("{now};{}", dir.display()) };
        // REG_EXPAND_SZ, as Windows keeps PATH (its %VARIABLES% stay variables)
        let bytes: Vec<u8> = next.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
        env.set_raw_value("Path", &RegValue { bytes: bytes.into(), vtype: RegType::REG_EXPAND_SZ })
            .map_err(|e| format!("can't change PATH: {e}"))?;
        broadcast_environment();
    }
    Ok(CliInstalled { at: dir.display().to_string(), on_path: true, path_line: None, profile: None })
}

/// Tells running programs (Explorer, so the terminals it starts) the environment changed.
#[cfg(windows)]
fn broadcast_environment() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };
    let what: Vec<u16> = "Environment\0".encode_utf16().collect();
    // SAFETY: a broadcast with a NUL-terminated string that outlives the call; the result isn't needed
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            what.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            2000,
            std::ptr::null_mut(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_launch_names_files_and_flags_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.tlm"), b"x").unwrap();
        let args = ["app", "--browser", "a.tlm", "missing.csv", "-psn_0_123"].map(String::from);
        assert_eq!(file_args(args, dir.path()), [dir.path().join("a.tlm").to_string_lossy().into_owned()]);
    }

    #[cfg(unix)]
    #[test]
    fn the_command_links_once_and_never_over_another_program() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, cli2) = (dir.path().join("app1/tlmbench"), dir.path().join("app2/tlmbench"));
        for c in [&cli, &cli2] {
            std::fs::create_dir_all(c.parent().unwrap()).unwrap();
            std::fs::write(c, b"#!").unwrap();
        }
        let link = dir.path().join("home/.local/bin/tlmbench");
        link_cli(&cli, &link).unwrap();
        assert_eq!(std::fs::read_link(&link).unwrap(), cli);
        // installing from another copy of the app moves the link
        link_cli(&cli2, &link).unwrap();
        assert_eq!(std::fs::read_link(&link).unwrap(), cli2);
        // a program of the same name that isn't our link stays
        std::fs::remove_file(&link).unwrap();
        std::fs::write(&link, b"other").unwrap();
        assert!(link_cli(&cli, &link).unwrap_err().contains("another program"));
        assert_eq!(std::fs::read(&link).unwrap(), b"other");
    }
}
