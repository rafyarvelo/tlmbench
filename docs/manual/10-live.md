# Live data

TelemetryBench can follow a source while it records: a serial port, a TCP server, UDP datagrams, a CSV or `.tlm` file another program is still writing, or a recording replayed as if it were arriving now. A live tab is an ordinary file tab whose file grows, so every pane, cursor, event, rule and check works on it. Everything that arrives is recorded into a session folder in the app's data folder, never beside your data. A strip above the panes adds what only live data needs: status, a scrubber, the window, a trigger and the sources.

## Connect a source

1. Press **Connect live source…** on the start page, or the **Live** button in the rail.
2. Pick the kind of source on the left, then fill in the form.
3. Check the preview under the form (see below).
4. Press **Connect**. A live tab opens, following the source.

<!-- shot: connect-dialog — the Connect a live source dialog on UDP: the kinds on the left, host and port, descriptor, the Decodes as preview and Preview a frame -->

| Kind | What it does | You provide |
| --- | --- | --- |
| **File playback** | Replays a recording (a demo: no hardware needed). Good for trying rules, checks and layouts. | **Recording**, **Speed** (0.5× to 50×), **Start at (seconds)**, and whether to **Loop when the recording ends**. |
| **Serial port** | USB or RS-232. | **Port** (such as `/dev/tty.usbserial-1` or `COM3`), **Baud rate** (default 115200) and a **Format descriptor**. |
| **UDP** | Listens for datagrams on a port. | **Listen on** (an address such as `0.0.0.0`), **Port** and a **Format descriptor**. |
| **TCP** | Connects to a server. | **Host**, **Port** and a **Format descriptor**. |
| **Tail a file** | Follows a file another program is still writing, like `tail -f`. Recorded from its start; the file is only read. | **File** (a CSV or `.tlm`). It decodes the way opening it does. |

Serial, UDP and TCP need a **Format descriptor** (a TOML file; **Choose…** picks one). It is the same descriptor that decodes the logger's recorded files.

**Name** defaults from the source; type to change it.

### Preview how frames decode

Once the form says where frames come from, the dialog shows **Decodes as** with the channels and units. For serial, UDP and TCP, press **Preview a frame**: the app reads the stream for about a second and shows the **Latest frame**, with how many frames a second arrived. A UDP port is bound and a device opened for that second; nothing is kept.

### Save a connection

Tick **Save this connection on the start page** to list it under **Live sources** there. Open it to fill the dialog again; **Save changes** keeps edits without connecting. The start page's **New connection…** starts a fresh one, and a saved connection's × removes it.

## The live strip

The strip above the panes shows:

| Part | What it is |
| --- | --- |
| **LIVE** badge | The state of the source (below). |
| Session name | The source's name. Hover for the session folder. |
| **REC** and a time | How much has been recorded. |
| **Dropping samples · N lost** | Appears when records were dropped (the buffer was full) or missing by sequence number. |
| Scrubber | Drag back through the session; the right end is live. The thumb turns amber and a label like −02:10 says how far back you are. |
| **10 s**, **30 s**, **1 min**, **5 min** | How much time the view shows while following. |
| **Following** or **Jump to live** | Following means the view tracks now. |
| **Trigger…** | An oscilloscope-style trigger (below). |
| **+ Add source** | Adds another source to this session. |
| **N sources** | Opens the Sources panel. |

### States

| Badge | Meaning |
| --- | --- |
| **LIVE** | The source is streaming. |
| **PAUSED** | You paused it. A replay holds still and resumes where it left off. A device keeps sending, so time runs on and the pause leaves a gap. The badge's tooltip says which. |
| **RECONNECTING** | The link dropped and the app is connecting again (TCP and serial, with **Reconnect automatically if the link drops** on). Frames sent meanwhile are lost. |
| **STOPPED** | You stopped the source. |
| **ENDED** | The source ended by itself: the file stopped, the replay finished, or a UDP source went quiet. |

### Follow, and go back to live

Any view you choose holds the view: scrolling back, panning, zooming or jumping to an event leaves Following. Data keeps arriving and the view stays where you put it. To return to the live edge press the **End** key, or press **Jump to live** on the strip. While following, tables keep the newest rows in sight and value tiles show each channel's newest sample.

## Sources and stream health

Press **N sources** to open the Sources panel. Each source has a card with its state and:

- **Pause** or **Resume**, **Stop** and **Remove**.
- **Source**: its endpoint. **Rate**: records a second. **Recorded**: records written.
- **Dropped**: records lost because the buffer was full. **Missing**: records missing by sequence number. Both turn red when above zero.
- **Buffer**: the most the ring buffer was filled.
- **Recording**: the file it is recorded to, or "not kept".
- **Error**, if the source had one.

### Several sources in one session

**+ Add source** on the strip opens the Connect dialog with **Open in** set to this session. Sources in one session share its recording folder and clock: every source starts at session time 0. Source 1 keeps plain channel names; source 2's channels read like `gps_lat·2`, the same rule files use. Each source is a file in Explorer. You can also pick **A new live tab** in **Open in** to keep sources apart, and separate live tabs run side by side. A recorded flight can be added to a live tab to compare a live run with a reference.

## Record, and where recordings go

**Record while streaming** is on by default. Each session is a folder `session-<start time>/` with a `.tlm` file for each source and a small `session.json`. It reopens like any recording.

- **Save recordings to** in the Connect dialog sets the folder for this connection. Empty uses the default.
- **Settings › Record live sessions to** sets the default. Without it, recordings go to a **Recordings** folder in the app's data folder.
- They are never written next to your data, and the cache's size limit never deletes them: they are your data. The start page lists them, and you delete them yourself.
- Turn **Record while streaming** off and the recording is not kept: it goes when the source closes. A source added to a session always records.

## Triggers

A trigger freezes a window around the moment a channel crosses a level, like an oscilloscope.

1. Press **Trigger…** on the strip.
2. In **When**, name a channel or an expression, choose **rises through**, **falls through**, **goes above** or **goes below**, and enter the level.
3. Set **Show** to the seconds **before** and **after** to keep in view.
4. In **Then**, choose **Stop after one** or **Repeat**.
5. Press **Arm** (it reads **Update** once armed).

The button reads **Armed** while it waits, **Triggered…** until the window after the crossing has been recorded, and **Triggered · N×** after. Each crossing adds an event in the **Trigger** category, and the view freezes on the window. **Jump to live** (or **End**) goes back to following. **Turn off** disarms. **Make a rule…** turns the same crossing into an event rule, so it marks every crossing in a recorded file the same way.

## Live rules and checks

A live tab's rules and checks run again every couple of seconds on what has been recorded, and once more when the source ends. A check that starts failing flashes the tab's badge, and its violations appear as events. See chapter 8.

## Record from the command line: `tlmbench record`

`tlmbench record` records a source into a new session folder until it ends, without opening the app. It prints the stream's health when it stops.

```
tlmbench record udp://0.0.0.0:9000 --descriptor frames.toml --out sessions --seconds 600
```

| Endpoint | What it does |
| --- | --- |
| `tcp://host:port` | Connects to it. |
| `udp://host:port` | Listens on it. |
| `serial://port?baud=rate` | A serial port (115200 baud if not given). |
| `file:///path` | Follows a growing CSV or `.tlm` from its start. The file is only read. |

| Option | What it does |
| --- | --- |
| `--descriptor FILE` | The frames' format descriptor. Sockets and serial ports need one; files bring their own. |
| `--out DIR` | Where the session folder goes. Default: `$TLMBENCH_RECORDINGS`, else the app's Recordings folder. |
| `--seconds N` | Stop after this many seconds. |
| `--idle-end N` | UDP: stop once no datagram has come for this many seconds. A file: once it hasn't grown for that long. |
| `--ring-mb N` | The ring buffer's size in MB (default: about 10 seconds of the descriptor's rate). |
| `--reconnect` | TCP and serial: connect again whenever the link drops, until stopped. Frames sent meanwhile are lost. |
| `--json` | The result as one JSON object. |

Open the recorded session folder in the app like any other recording, or check it with `tlmbench check` (chapter 8).
