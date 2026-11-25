# Installing TelemetryBench

Every release on the [Releases page](https://github.com/rafyarvelo/tlmbench/releases) has the app for macOS, Windows and
Linux, the `tlmbench` command line on its own, and a `SHA256SUMS` file. `<version>` below is the release's number, for
example `1.0.0`.

| You have | Download |
| --- | --- |
| A Mac (Apple silicon or Intel) | `TelemetryBench-<version>-macos-universal.dmg` |
| Windows 10 or 11 (x64) | `TelemetryBench-<version>-windows-x64-setup.exe` (or the `.msi`, or the `-portable.zip` to run without installing) |
| Linux x86_64, Debian or Ubuntu | `TelemetryBench-<version>-linux-amd64.deb` |
| Linux x86_64, any distribution | `TelemetryBench-<version>-linux-x86_64.AppImage` |
| Only the command line | `tlmbench-<version>-<os>-<arch>.tar.gz` (`.zip` on Windows): macos-arm64, macos-x86_64, windows-x64, linux-x86_64, linux-arm64 |
| Python | `pip install tlmbench` |
| No install at all | `tlmbench-<version>-browser-site.zip`: browser-only mode, served from any static host ([browser-only.md](browser-only.md)) |

The app carries the command line too: Help › Install command-line tool… puts `tlmbench` on your PATH.

## macOS

1. Open the `.dmg` and drag **TelemetryBench** to **Applications**.
2. Open it from Applications. The first time, macOS says it can't check the app for malicious software, because v1 is
   signed by the project and not notarised by Apple (that costs a yearly fee). Click **Done** (or **Cancel**), then:
   - **macOS 15 (Sequoia) and later:** System Settings › Privacy & Security, scroll down to "TelemetryBench was
     blocked", click **Open Anyway**, and confirm with your password.
   - **macOS 14 (Sonoma) and earlier:** right-click (or Control-click) TelemetryBench in Applications › **Open**, then
     **Open** in the dialog.
   - **Any version, in Terminal:** remove the flag macOS puts on downloaded apps, then open it as usual:
     ```
     xattr -dr com.apple.quarantine /Applications/TelemetryBench.app
     ```

   You do this once; afterwards it opens like any app. (Check the download first if you like: [Checking a
   download](#checking-a-download).)
3. To use `tlmbench` in Terminal: Help › Install command-line tool…. It goes to `~/.local/bin`; if that folder isn't on
   your PATH, the dialog shows the line to add to `~/.zprofile` (Copy the line).

`.tlm` files open in TelemetryBench when double-clicked; CSV, TSV and Parquet files offer it under right-click › Open With.

**The command line alone:** unpack the archive, then remove the download flag macOS puts on it and move it onto your
PATH:

```
tar xzf tlmbench-<version>-macos-arm64.tar.gz
xattr -d com.apple.quarantine tlmbench-<version>-macos-arm64/tlmbench
mv tlmbench-<version>-macos-arm64/tlmbench ~/.local/bin/
```

**Uninstall:** drag TelemetryBench from Applications to the Bin. Its cache and settings are in
`~/Library/Caches/tlmbench` and `~/Library/Application Support/io.github.rafyarvelo.tlmbench`; Settings › Reset app clears them first if you
want them gone.

## Windows

1. Run `TelemetryBench-<version>-windows-x64-setup.exe`. v1 isn't code-signed yet (it will be once SignPath
   Foundation's free signing for open-source projects is in place), so Windows SmartScreen may say "Windows protected
   your PC": click **More info**, then **Run anyway**.
2. The installer adds TelemetryBench to the Start menu. It needs Microsoft Edge WebView2, which Windows 10 and 11
   already have; on an older system the installer fetches it.
3. To use `tlmbench` in a terminal: Help › Install command-line tool… adds the app's folder to your user PATH. Open a
   new terminal afterwards.

The `.msi` installs the same app (for managed machines). The `-portable.zip` runs without installing: unpack it anywhere
and run `TelemetryBench.exe`; `tlmbench.exe` is beside it.

**Uninstall:** Settings › Apps › TelemetryBench › Uninstall.

## Linux

**Debian or Ubuntu (`.deb`):**

```
sudo apt install ./TelemetryBench-<version>-linux-amd64.deb
```

It appears in your applications menu; `tlmbench` is installed with it. Remove it with
`sudo apt remove "$(dpkg-deb -f TelemetryBench-<version>-linux-amd64.deb Package)"`.

**Any distribution (AppImage):**

```
chmod +x TelemetryBench-<version>-linux-x86_64.AppImage
./TelemetryBench-<version>-linux-x86_64.AppImage
```

An AppImage needs FUSE 2. If it says it can't mount itself, install `libfuse2` (Ubuntu 22.04: `sudo apt install
libfuse2`; 24.04 and later: `sudo apt install libfuse2t64`), or run it with `--appimage-extract-and-run`.

The app uses WebKitGTK; where WebGL2 isn't available (a virtual machine without 3D acceleration, a remote desktop), it
draws with Canvas 2D instead.

**The command line alone:** `tar xzf tlmbench-<version>-linux-x86_64.tar.gz` (or `linux-arm64`) and move `tlmbench` onto
your PATH. A container image is published too: `docker run --rm -v "$PWD:/data" ghcr.io/rafyarvelo/tlmbench:<version>
info /data/flight.tlm`.

## Checking a download

Each file is listed in `SHA256SUMS` with its checksum, and has a build-provenance attestation that ties it to the
workflow run that built it. To check that what you downloaded is what was built, download `SHA256SUMS` into the same
folder as the file, then in a terminal in that folder:

**macOS and Linux:**

```
cd ~/Downloads
shasum -a 256 -c SHA256SUMS --ignore-missing
```

It prints a line for each file of the list that's in the folder, such as
`TelemetryBench-<version>-macos-universal.dmg: OK` (`--ignore-missing` skips the files you didn't download). **FAILED**
means the file is damaged or not the one released: download it again. No output at all means `SHA256SUMS` isn't in
the same folder as the file.

**Windows (PowerShell):**

```
cd ~\Downloads
$f = "TelemetryBench-<version>-windows-x64-setup.exe"
(Get-FileHash $f).Hash.ToLower() -eq ((Select-String -Path SHA256SUMS -Pattern $f -SimpleMatch).Line -split '\s+')[0]
```

`True` means it matches; `False` means download it again.

**Any system, with the GitHub CLI:** `gh attestation verify <file> --repo rafyarvelo/tlmbench` also checks that the file
was built by this repository's release workflow.
