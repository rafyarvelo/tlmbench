//! The core as a daemon: the app's commands served to browser tabs over a local WebSocket (design: Daemon and clients).
//!
//! The commands are the ones the desktop UI calls, run by the same handlers: a request is a command name and its
//! arguments as JSON, and each client gets a hidden window of its own, so everything a window owns (holds on files,
//! its handoff, its view link) is per client. Replies, channel messages (build progress) and events come back as
//! binary frames: `[kind u8][id u32 LE][payload]`, kind 0 a JSON reply, 1 a raw reply (the viewport format),
//! 2 an error, 3 and 4 a JSON or raw channel message (payload: `[index u32 LE]` then the message), 5 an event
//! (`{"event", "payload"}`). The page and its scripts are served from a folder; the socket asks for the session's
//! token, and only listens on the loopback address.

use super::*;
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponse, InvokeResponseBody};
use tauri::webview::InvokeRequest;
use tauri::{Listener, Runtime, WebviewUrl, WebviewWindowBuilder};
use tungstenite::handshake::server::{ErrorResponse, Request, Response as Handshake};
use tungstenite::protocol::Role;
use tungstenite::{Message, WebSocket};

/// Browser access of the desktop app: the clients and the server while it is on.
pub struct Browser {
    pub clients: Arc<Clients>,
    server: Mutex<Option<Server>>,
}

impl Browser {
    pub fn new(clients: Arc<Clients>) -> Browser {
        Browser { clients, server: Mutex::default() }
    }
}

/// Browser access: turns it on or off (none: asks), and returns the page's address while it is on. The page is the
/// app's own bundle; the address carries a token made for the session.
#[tauri::command]
pub fn browser_access<R: Runtime>(
    on: Option<bool>,
    app: tauri::AppHandle<R>,
    browser: State<'_, Browser>,
) -> Result<Option<String>, String> {
    let mut server = browser.server.lock().unwrap();
    match on {
        Some(true) if server.is_none() => {
            let resolver = app.asset_resolver();
            let assets: Assets =
                Arc::new(move |rel| resolver.get(rel.to_string()).map(|a| (a.bytes.to_vec(), a.mime_type)));
            *server = Some(
                serve(app.clone(), browser.clients.clone(), 0, None, Some(assets), false).map_err(|e| e.to_string())?,
            );
        }
        Some(false) => {
            if let Some(s) = server.take() {
                s.stop(&browser.clients);
            }
        }
        _ => {}
    }
    Ok(server.as_ref().map(|s| s.url()))
}

/// What `tlmbench-app --daemon` was asked for.
pub struct Args {
    port: u16,
    ui: Option<PathBuf>,
    once: bool,
    token: Option<String>,
}

impl Args {
    /// The daemon's options, when the command line asks for it: `--daemon [--port N] [--ui DIR] [--token T] [--once]`.
    pub fn from_env() -> Option<Args> {
        let mut it = std::env::args().skip(1);
        let mut a = Args { port: 0, ui: None, once: false, token: None };
        let mut daemon = false;
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--daemon" => daemon = true,
                "--port" => a.port = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
                "--ui" => a.ui = it.next().map(PathBuf::from),
                "--token" => a.token = it.next(),
                "--once" => a.once = true,
                _ => {}
            }
        }
        daemon.then_some(a)
    }
}

type Socket = Arc<Mutex<WebSocket<TcpStream>>>;

/// Where the page comes from: a path inside the UI bundle to its bytes and type.
pub type Assets = Arc<dyn Fn(&str) -> Option<(Vec<u8>, String)> + Send + Sync>;

/// The page from a folder (the daemon on its own).
pub fn assets_from(dir: PathBuf) -> Assets {
    Arc::new(move |rel| {
        let f = dir.join(rel);
        std::fs::read(&f).ok().map(|b| (b, mime(&f).to_string()))
    })
}

/// The connected clients' sockets, by window name.
#[derive(Default)]
pub struct Clients(Mutex<HashMap<String, Socket>>);

fn frame(kind: u8, id: u32, payload: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(5 + payload.len());
    f.push(kind);
    f.extend_from_slice(&id.to_le_bytes());
    f.extend_from_slice(payload);
    f
}

fn send(socket: &Socket, frame: Vec<u8>) {
    let _ = socket.lock().unwrap().send(Message::Binary(frame.into()));
}

impl Clients {
    fn broadcast(&self, frame: &[u8]) {
        for s in self.0.lock().unwrap().values() {
            send(s, frame.to_vec());
        }
    }

    pub fn count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

/// Channel messages (a file's build progress) go to the client whose window opened the channel.
pub fn interceptor<R: Runtime>(
    clients: Arc<Clients>,
) -> impl Fn(&tauri::Webview<R>, CallbackFn, usize, &InvokeResponseBody) -> bool {
    move |webview, callback, index, body| {
        let Some(socket) = clients.0.lock().unwrap().get(webview.label()).cloned() else { return false };
        let (kind, bytes) = match body {
            InvokeResponseBody::Json(s) => (3, s.as_bytes().to_vec()),
            InvokeResponseBody::Raw(b) => (4, b.clone()),
        };
        let mut payload = (index as u32).to_le_bytes().to_vec();
        payload.extend_from_slice(&bytes);
        send(&socket, frame(kind, callback.0, &payload));
        true
    }
}

/// The page's address and the session's token.
pub struct Server {
    pub addr: SocketAddr,
    pub token: String,
    /// While off, new connections are turned away.
    active: Arc<AtomicBool>,
}

impl Server {
    /// The page's address, with the token.
    pub fn url(&self) -> String {
        format!("http://{}/?token={}", self.addr, self.token)
    }

    /// Turns the clients away and closes the ones connected.
    pub fn stop(&self, clients: &Clients) {
        self.active.store(false, Ordering::Release);
        for s in clients.0.lock().unwrap().values() {
            let _ = s.lock().unwrap().close(None);
        }
        // the listener ends at its next connection: make one
        let _ = TcpStream::connect(self.addr);
    }
}

fn new_token() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Starts serving on `127.0.0.1:port` (0: any free port) from a thread of its own. `ui` is the folder the page comes
/// from; `once` ends the process when the last client leaves.
pub fn serve<R: Runtime>(
    app: tauri::AppHandle<R>,
    clients: Arc<Clients>,
    port: u16,
    token: Option<String>,
    ui: Option<Assets>,
    once: bool,
) -> std::io::Result<Server> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let addr = listener.local_addr()?;
    let token = token.unwrap_or_else(new_token);
    // events reach every client (the windows of the app take them from the runtime itself)
    for name in ["settings-changed", "view-linked", "table-progress"] {
        let clients = clients.clone();
        app.listen_any(name, move |e| {
            clients.broadcast(&frame(5, 0, format!("{{\"event\":\"{name}\",\"payload\":{}}}", e.payload()).as_bytes()));
        });
    }
    let t = token.clone();
    let active = Arc::new(AtomicBool::new(true));
    let on = active.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if !on.load(Ordering::Acquire) {
                break;
            }
            let (app, clients, token, ui) = (app.clone(), clients.clone(), t.clone(), ui.clone());
            std::thread::spawn(move || {
                let _ = connection(stream, app, clients, token, ui, once);
            });
        }
    });
    Ok(Server { addr, token, active })
}

fn mime(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        Some("json") => "application/json",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn respond(mut s: TcpStream, status: &str, kind: &str, body: &[u8]) {
    let _ = write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(body);
}

fn query_of<'a>(target: &'a str, key: &str) -> Option<&'a str> {
    target.split_once('?')?.1.split('&').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))
}

fn connection<R: Runtime>(
    stream: TcpStream,
    app: tauri::AppHandle<R>,
    clients: Arc<Clients>,
    token: String,
    ui: Option<Assets>,
    once: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // the head of the request, without taking it from the stream (the WebSocket handshake reads it again)
    let mut buf = vec![0u8; 8192];
    let mut head = String::new();
    for _ in 0..200 {
        let n = stream.peek(&mut buf)?;
        head = String::from_utf8_lossy(&buf[..n]).into_owned();
        if head.contains("\r\n\r\n") {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let target = head.lines().next().and_then(|l| l.split(' ').nth(1)).unwrap_or("/").to_string();
    if !head.to_ascii_lowercase().contains("upgrade: websocket") {
        // the page: the folder's files, nothing outside it (the request is read first, or closing resets the connection)
        let _ = std::io::Read::read(&mut &stream, &mut buf);
        let path = target.split('?').next().unwrap_or("/");
        let rel = if path == "/" { "index.html" } else { path.trim_start_matches('/') };
        let found = ui.filter(|_| !rel.split('/').any(|p| p == ".." || p.is_empty())).and_then(|f| f(rel));
        match found {
            Some((bytes, kind)) => respond(stream, "200 OK", &kind, &bytes),
            None => respond(stream, "404 Not Found", "text/plain", b"not found"),
        };
        return Ok(());
    }
    let want = query_of(&target, "window").map(str::to_string);
    #[allow(clippy::result_large_err)] // the handshake's own error type
    let ok = move |req: &Request, resp: Handshake| -> Result<Handshake, ErrorResponse> {
        let t = req.uri().to_string();
        let local = |o: &str| {
            ["http://127.0.0.1", "http://localhost"].iter().any(|h| o == *h || o.starts_with(&format!("{h}:")))
        };
        let origin_ok = req.headers().get("origin").and_then(|o| o.to_str().ok()).is_none_or(local);
        if query_of(&t, "token") == Some(token.as_str()) && origin_ok {
            Ok(resp)
        } else {
            Err(tungstenite::http::Response::builder().status(403).body(Some("no".into())).unwrap())
        }
    };
    let reader_stream = stream.try_clone()?;
    let mut reader = tungstenite::accept_hdr(stream, ok)?;
    let writer: Socket = Arc::new(Mutex::new(WebSocket::from_raw_socket(reader_stream, Role::Server, None)));

    // this client's window: the one a "new window" made for it, or a hidden window of its own
    let files = app.state::<Files>();
    let (label, made) = match want.filter(|l| app.get_webview_window(l).is_some()) {
        Some(l) => (l, false),
        None => {
            let l = if app.get_webview_window("main").is_none() {
                "main".to_string()
            } else {
                format!("win-{}", files.next_window.fetch_add(1, Ordering::Relaxed))
            };
            (l, true)
        }
    };
    let window = match app.get_webview_window(&label) {
        Some(w) => w,
        None => WebviewWindowBuilder::new(&app, &label, WebviewUrl::External("about:blank".parse().unwrap()))
            .visible(false)
            .build()?,
    };
    clients.0.lock().unwrap().insert(label.clone(), writer.clone());
    writer.lock().unwrap().send(Message::text(format!("{{\"hello\":{{\"label\":\"{label}\"}}}}")))?;

    loop {
        match reader.read() {
            Ok(Message::Text(t)) => {
                let Ok(req) = serde_json::from_str::<serde_json::Value>(&t) else { continue };
                let id = req["id"].as_u64().unwrap_or(0) as u32;
                let request = InvokeRequest {
                    cmd: req["cmd"].as_str().unwrap_or_default().to_string(),
                    callback: CallbackFn(0),
                    error: CallbackFn(1),
                    url: "tauri://localhost/".parse().unwrap(),
                    body: InvokeBody::Json(req["args"].clone()),
                    headers: tauri::http::HeaderMap::new(),
                    invoke_key: app.invoke_key().to_string(),
                };
                let (webview, out) = (window.as_ref().clone(), writer.clone());
                std::thread::spawn(move || {
                    webview.on_message(
                        request,
                        Box::new(move |_w, _cmd, response, _cb, _err| {
                            send(
                                &out,
                                match response {
                                    InvokeResponse::Ok(InvokeResponseBody::Json(s)) => frame(0, id, s.as_bytes()),
                                    InvokeResponse::Ok(InvokeResponseBody::Raw(b)) => frame(1, id, &b),
                                    InvokeResponse::Err(e) => frame(2, id, e.0.to_string().as_bytes()),
                                },
                            );
                        }),
                    );
                });
            }
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {}
        }
    }
    clients.0.lock().unwrap().remove(&label);
    if made {
        // (the runtime may not announce the window going; what it held is let go here)
        release_window(&label, &files);
        let _ = window.destroy();
    }
    if once && clients.count() == 0 {
        std::process::exit(0);
    }
    Ok(())
}

/// The core with no window system: the app's commands on the mock runtime, a window per client.
pub fn core(clients: Arc<Clients>) -> tauri::App<tauri::test::MockRuntime> {
    // under the app's identifier: its settings file and cache are the app's
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().identifier = "io.github.rafyarvelo.tlmbench".into();
    tauri::test::mock_builder()
        .manage(new_files())
        .manage(Browser::new(clients.clone()))
        .on_window_event(|w, ev| {
            if let tauri::WindowEvent::Destroyed = ev
                && let Some(files) = w.try_state::<Files>()
            {
                release_window(w.label(), &files);
            }
        })
        .invoke_handler(commands!())
        .channel_interceptor(interceptor(clients))
        .build(context)
        .expect("the daemon failed to start")
}

/// `tlmbench-app --daemon`: the core without a window, until it is stopped (or, with `--once`, until its last client
/// leaves).
pub fn run(args: Args) {
    let clients = Arc::new(Clients::default());
    let app = core(clients.clone());
    let ui = args
        .ui
        .or_else(|| std::env::var_os("TLMBENCH_UI").map(PathBuf::from))
        .or_else(|| Some(PathBuf::from("app/ui/dist")).filter(|p| p.exists()));
    let server = serve(app.handle().clone(), clients, args.port, args.token, ui.map(assets_from), args.once)
        .expect("the daemon couldn't listen");
    println!("{}", server.url());
    loop {
        std::thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A daemon on a free port in this process, with the commands of the app.
    fn start(ui: Option<PathBuf>) -> (Server, tauri::App<tauri::test::MockRuntime>, Arc<Clients>) {
        let clients = Arc::new(Clients::default());
        let app = core(clients.clone());
        let server =
            serve(app.handle().clone(), clients.clone(), 0, Some("tok".into()), ui.map(assets_from), false).unwrap();
        (server, app, clients)
    }

    type Client = tungstenite::WebSocket<TcpStream>;

    fn connect(server: &Server, query: &str) -> Result<Client, tungstenite::Error> {
        let stream = TcpStream::connect(server.addr).unwrap();
        let url = format!("ws://{}/ws?{query}", server.addr);
        tungstenite::client(url, stream).map(|(c, _)| c).map_err(|e| match e {
            tungstenite::HandshakeError::Failure(e) => e,
            _ => unreachable!(),
        })
    }

    fn ask(c: &mut Client, id: u32, cmd: &str, args: serde_json::Value) -> (u8, Vec<u8>) {
        c.send(Message::text(serde_json::json!({ "id": id, "cmd": cmd, "args": args }).to_string())).unwrap();
        loop {
            if let Message::Binary(b) = c.read().unwrap() {
                let (kind, rid) = (b[0], u32::from_le_bytes(b[1..5].try_into().unwrap()));
                if rid == id && kind <= 2 {
                    return (kind, b[5..].to_vec());
                }
            }
        }
    }

    #[test]
    fn a_client_needs_the_token_and_gets_a_window_of_its_own() {
        let (server, _app, clients) = start(None);
        assert!(connect(&server, "token=nope").is_err(), "a wrong token is refused");
        assert!(connect(&server, "").is_err(), "so is none");
        let mut a = connect(&server, "token=tok").unwrap();
        let hello = a.read().unwrap().into_text().unwrap();
        assert!(hello.contains("\"label\":\"main\""), "{hello}");
        let mut b = connect(&server, "token=tok").unwrap();
        let hello = b.read().unwrap().into_text().unwrap();
        assert!(hello.contains("win-2"), "{hello}");
        assert_eq!(clients.count(), 2);
    }

    #[test]
    fn commands_run_as_in_the_app() {
        let (server, _app, _clients) = start(None);
        let mut c = connect(&server, "token=tok").unwrap();
        c.read().unwrap();
        // a raw reply (the viewport's format) comes back as bytes
        let (kind, body) = ask(&mut c, 1, "echo", serde_json::json!({ "bytes": 64 }));
        assert_eq!((kind, body.len()), (1, 64));
        // a JSON reply, and an error for a command that is not there
        let (kind, body) = ask(&mut c, 2, "holders", serde_json::json!({ "file": 7 }));
        assert_eq!((kind, String::from_utf8(body).unwrap()), (0, "[]".to_string()));
        let (kind, _) = ask(&mut c, 3, "no_such_command", serde_json::json!({}));
        assert_eq!(kind, 2);
    }

    #[test]
    fn files_are_held_by_the_client_that_opened_them() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("a.csv");
        let body: String = (0..2000).map(|i| format!("{i},{}\n", i % 7)).collect();
        std::fs::write(&csv, format!("time,x\n{body}")).unwrap();
        let (server, _app, _clients) = start(None);
        let mut a = connect(&server, "token=tok").unwrap();
        a.read().unwrap();
        let mut b = connect(&server, "token=tok").unwrap();
        b.read().unwrap();
        // a file opened by one client: its reply, and its build progress through the channel it passed
        let args = serde_json::json!({ "path": csv.to_string_lossy(), "progress": "__CHANNEL__:7" });
        a.send(Message::text(serde_json::json!({ "id": 1, "cmd": "open", "args": args }).to_string())).unwrap();
        let (mut reply, mut channel) = (None, false);
        while reply.is_none() || !channel {
            if let Message::Binary(f) = a.read().unwrap() {
                let id = u32::from_le_bytes(f[1..5].try_into().unwrap());
                match (f[0], id) {
                    (0, 1) => reply = Some(serde_json::from_slice::<serde_json::Value>(&f[5..]).unwrap()),
                    (2, 1) => panic!("{}", String::from_utf8_lossy(&f[5..])),
                    (3, 7) => channel = true,
                    _ => {}
                }
            }
        }
        let id = reply.unwrap()["id"].as_u64().unwrap();
        let (_, others) = ask(&mut b, 2, "holders", serde_json::json!({ "file": id }));
        assert_eq!(others, b"[\"main\"]".to_vec(), "the other window holds it");
        // the other client takes a hold too; the first leaving doesn't close the file
        let (kind, _) = ask(&mut b, 3, "hold", serde_json::json!({ "file": id }));
        assert_eq!(kind, 0);
        drop(a);
        std::thread::sleep(Duration::from_millis(300));
        let (kind, view) = ask(&mut b, 4, "values_at", serde_json::json!({ "file": id, "channels": [0], "t": "5" }));
        assert_eq!(kind, 0, "{}", String::from_utf8_lossy(&view));
        let (_, others) = ask(&mut b, 5, "holders", serde_json::json!({ "file": id }));
        assert_eq!(others, b"[]".to_vec(), "the closed client let go of its hold");
    }

    #[test]
    fn the_page_is_served_from_its_folder_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<p>hi</p>").unwrap();
        std::fs::write(dir.path().join("app.js"), "1").unwrap();
        let (server, _app, _clients) = start(Some(dir.path().to_path_buf()));
        let get = |path: &str| {
            let mut s = TcpStream::connect(server.addr).unwrap();
            write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut out = String::new();
            std::io::Read::read_to_string(&mut s, &mut out).unwrap();
            out
        };
        assert!(get("/?token=tok").starts_with("HTTP/1.1 200") && get("/").contains("<p>hi</p>"));
        assert!(get("/app.js").contains("text/javascript"));
        assert!(get("/../Cargo.toml").starts_with("HTTP/1.1 404"));
        assert!(get("/nothing").starts_with("HTTP/1.1 404"));
    }
}
