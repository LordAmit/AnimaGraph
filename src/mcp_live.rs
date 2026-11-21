//! Live mode: an assistant works in the Animagraph window that is open, so
//! the person sees each change. The window listens on a local socket that
//! only this user can open (no network). `animagraph --mcp` sends each tool
//! call there when a window is open and live access is on; otherwise it works
//! in the background as before (see mcp_server).

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use eframe::egui;
use serde_json::{Value, json};

use crate::app::StudioApp;
use crate::mcp_tools::{Reply, Workspace};
use crate::scene::{Edits, Scene};

/// Where the open window listens. ANIMAGRAPH_MCP_SOCKET points both sides
/// somewhere else (used to test live mode without touching a real window).
pub fn socket_path() -> PathBuf {
    std::env::var_os("ANIMAGRAPH_MCP_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::fonts::data_dir().join("mcp-live.sock"))
}

/// Whether a check screenshot should listen too (only with a test socket).
pub fn test_socket_set() -> bool {
    std::env::var_os("ANIMAGRAPH_MCP_SOCKET").is_some()
}

/// A tool call from an assistant, waiting for the window to answer.
pub struct LiveRequest {
    pub tool: String,
    pub args: Value,
    pub reply: Sender<Value>,
}

/// A tool's answer, as sent back over the socket.
pub fn reply_to_json(r: Reply) -> Value {
    use base64::Engine;
    json!({
        "text": r.text,
        "is_error": r.is_error,
        "png": r.png.map(|p| base64::engine::general_purpose::STANDARD.encode(p)),
    })
}

pub fn reply_from_json(v: &Value) -> Reply {
    use base64::Engine;
    Reply {
        text: v.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
        is_error: v.get("is_error").and_then(Value::as_bool).unwrap_or(true),
        png: v
            .get("png")
            .and_then(Value::as_str)
            .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok()),
    }
}

/// Start listening for assistants (window side). `None` when another
/// Animagraph window is already listening, or on systems without sockets.
#[cfg(unix)]
pub fn start_listener(ctx: egui::Context) -> Option<Receiver<LiveRequest>> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    let path = socket_path();
    if UnixStream::connect(&path).is_ok() {
        return None;
    }
    // Left over from a window that closed.
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let listener = UnixListener::bind(&path).ok()?;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (tx, ctx) = (tx.clone(), ctx.clone());
            std::thread::spawn(move || serve(stream, tx, ctx));
        }
    });
    Some(rx)
}

#[cfg(not(unix))]
pub fn start_listener(_ctx: egui::Context) -> Option<Receiver<LiveRequest>> {
    None
}

/// One assistant connection: a request per line, an answer per line.
#[cfg(unix)]
fn serve(stream: std::os::unix::net::UnixStream, tx: Sender<LiveRequest>, ctx: egui::Context) {
    use std::io::{BufRead, Write};
    let Ok(read_half) = stream.try_clone() else { return };
    let mut writer = stream;
    for line in std::io::BufReader::new(read_half).lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let (rtx, rrx) = std::sync::mpsc::channel();
        let req = LiveRequest {
            tool: msg.get("tool").and_then(Value::as_str).unwrap_or("").to_string(),
            args: msg.get("args").cloned().unwrap_or_else(|| json!({})),
            reply: rtx,
        };
        if tx.send(req).is_err() {
            break;
        }
        ctx.request_repaint();
        let answer = rrx
            .recv_timeout(std::time::Duration::from_secs(300))
            .unwrap_or_else(|_| json!({"text": "Animagraph did not answer in time.", "is_error": true}));
        if writeln!(writer, "{answer}").and_then(|_| writer.flush()).is_err() {
            break;
        }
    }
}

/// Answer waiting tool calls (window side, once per frame).
pub fn handle_requests(app: &mut StudioApp, ctx: &egui::Context) {
    let Some(rx) = app.live_rx.take() else { return };
    while let Ok(req) = rx.try_recv() {
        let answer = if !app.prefs.mcp_live {
            json!({"refused": true})
        } else {
            let r = {
                let mut ws = LiveWorkspace { app, ctx };
                crate::mcp_tools::call(&mut ws, &req.tool, &req.args)
            };
            app.status = format!("Assistant: {}{}", req.tool.replace('_', " "), if r.is_error { " (could not do it)" } else { "" });
            reply_to_json(r)
        };
        let _ = req.reply.send(answer);
        ctx.request_repaint();
    }
    app.live_rx = Some(rx);
}

/// The open window's files, for the tools. Ids are "file" + the file's
/// scene id; the photo being changed becomes the current file.
struct LiveWorkspace<'a> {
    app: &'a mut StudioApp,
    ctx: &'a egui::Context,
}

impl LiveWorkspace<'_> {
    fn id_of(scene: &Scene) -> String {
        format!("file{}", scene.id)
    }

    /// Index in the Files panel of the file with this id.
    fn index_of(&self, id: &str) -> Option<usize> {
        (0..self.app.files.len()).find(|&i| {
            let s = if Some(i) == self.app.active_file { self.app.scene.as_ref() } else { self.app.files[i].scene.as_ref() };
            s.is_some_and(|s| Self::id_of(s) == id)
        })
    }
}

impl Workspace for LiveWorkspace<'_> {
    fn mode(&self) -> &'static str {
        "live"
    }

    fn folders(&self) -> Vec<PathBuf> {
        crate::mcp_access::allowed_folders(&self.app.prefs)
    }

    fn add_image(&mut self, _path: PathBuf, name: String, img: image::RgbaImage) -> String {
        self.app.close_layout();
        self.app.set_scene(self.ctx, Scene::new(name, img));
        self.app.scene.as_ref().map(Self::id_of).unwrap_or_default()
    }

    fn list(&self) -> Value {
        let list: Vec<Value> = (0..self.app.files.len())
            .filter_map(|i| {
                let active = Some(i) == self.app.active_file;
                let s = if active { self.app.scene.as_ref() } else { self.app.files[i].scene.as_ref() }?;
                Some(json!({"image_id": Self::id_of(s), "name": s.name, "current": active, "width": s.full.width(), "height": s.full.height()}))
            })
            .collect();
        json!({"mode": "live", "images": list})
    }

    fn edit(&mut self, id: &str, f: &mut dyn FnMut(&mut Scene, &mut Edits) -> Result<Value, String>) -> Result<Value, String> {
        let i = self.index_of(id).ok_or_else(|| format!("No open file with id {id}. Use list_open_images."))?;
        if self.app.active_file != Some(i) {
            self.app.close_layout();
            self.app.select_file(self.ctx, i);
        }
        // The window keeps the current file's edits in its own fields.
        self.app.sync_edits_to_scene();
        let Some(scene) = self.app.scene.as_mut() else { return Err("No current file.".into()) };
        let before = (scene.full.dimensions(), scene.angle, scene.crop);
        let mut edits = scene.edits.clone();
        let result = f(scene, &mut edits);
        scene.edits = edits.clone();
        let framing_changed = before != (scene.full.dimensions(), scene.angle, scene.crop);
        self.app.show_edits(self.ctx, &edits, framing_changed);
        result
    }

    fn close(&mut self, id: &str) -> Result<Value, String> {
        let i = self.index_of(id).ok_or_else(|| format!("No open file with id {id}."))?;
        self.app.remove_file(self.ctx, i);
        Ok(json!({"closed": id}))
    }
}

/// Assistant side: the connection to an open window, if there is one.
pub struct LiveClient {
    enabled: bool,
    #[cfg(unix)]
    conn: Option<(std::io::BufReader<std::os::unix::net::UnixStream>, std::os::unix::net::UnixStream)>,
}

impl LiveClient {
    pub fn new() -> Self {
        Self {
            enabled: true,
            #[cfg(unix)]
            conn: None,
        }
    }

    /// A client that never looks for a window (background only).
    #[cfg(test)]
    pub fn off() -> Self {
        Self {
            enabled: false,
            #[cfg(unix)]
            conn: None,
        }
    }

    /// Send a tool call to the open window. `None` when no window is open,
    /// or it has live access switched off: then the caller works in the
    /// background.
    #[cfg(unix)]
    pub fn call(&mut self, tool: &str, args: &Value) -> Option<Reply> {
        use std::io::{BufRead, Write};
        if !self.enabled {
            return None;
        }
        for _attempt in 0..2 {
            if self.conn.is_none() {
                let stream = std::os::unix::net::UnixStream::connect(socket_path()).ok()?;
                let reader = std::io::BufReader::new(stream.try_clone().ok()?);
                self.conn = Some((reader, stream));
            }
            let (reader, writer) = self.conn.as_mut()?;
            let line = json!({"tool": tool, "args": args}).to_string();
            let mut answer = String::new();
            let ok = writeln!(writer, "{line}").and_then(|_| writer.flush()).is_ok()
                && reader.read_line(&mut answer).map(|n| n > 0).unwrap_or(false);
            if !ok {
                // The window closed; try a fresh connection once.
                self.conn = None;
                continue;
            }
            let v: Value = serde_json::from_str(&answer).ok()?;
            if v.get("refused").and_then(Value::as_bool) == Some(true) {
                self.conn = None;
                return None;
            }
            return Some(reply_from_json(&v));
        }
        None
    }

    #[cfg(not(unix))]
    pub fn call(&mut self, _tool: &str, _args: &Value) -> Option<Reply> {
        let _ = self.enabled;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_survive_the_trip_over_the_socket() {
        let r = Reply { text: "hi".into(), png: Some(vec![1, 2, 3]), is_error: false };
        let back = reply_from_json(&reply_to_json(r));
        assert_eq!(back.text, "hi");
        assert_eq!(back.png, Some(vec![1, 2, 3]));
        assert!(!back.is_error);
        assert!(reply_from_json(&json!({"text": "x"})).is_error);
    }
}
