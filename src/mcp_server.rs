//! Background mode for assistants: `animagraph --mcp` runs a local MCP
//! server on standard input and output (one JSON-RPC message per line), with
//! no window. Nothing is sent over the network. Messages for people go to
//! standard error, so standard output carries only the protocol.

use std::io::{BufRead, Write};

use base64::Engine;
use serde_json::{Value, json};

use crate::mcp_tools::{Reply, Session};

const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Animagraph edits photos the way its window does. When the Animagraph window is \
open (live mode), the tools work in that window and the person sees each change; list_open_images shows its \
files. Otherwise they work in the background. Open a photo with open_image \
(only from the allowed folders), then use apply_style or set_adjustments for colour, set_framing for rotation, \
straightening, crop, border, rounded corners, corner mounts and caption, preview to look at the result, and \
save_image to save it as HEIC, JPEG or PNG.";

/// Run until standard input closes. Returns the process exit code.
pub fn run() -> i32 {
    let prefs = crate::settings::Prefs::load();
    let folders = crate::mcp_access::allowed_folders(&prefs);
    eprintln!(
        "Animagraph MCP server ready. Allowed folders: {}",
        folders.iter().map(|f| f.display().to_string()).collect::<Vec<_>>().join(", ")
    );
    let mut session = Session::new(folders);
    let mut live = crate::mcp_live::LiveClient::new();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&mut session, &mut live, &msg),
            Err(e) => Some(error(Value::Null, -32700, &format!("Parse error: {e}"))),
        };
        if let Some(r) = reply {
            if writeln!(out, "{r}").and_then(|_| out.flush()).is_err() {
                break;
            }
        }
    }
    0
}

/// Answer one message; notifications (no id) get no answer.
pub fn handle(session: &mut Session, live: &mut crate::mcp_live::LiveClient, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let id = id?;
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOL_VERSIONS[0]);
            json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "animagraph", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools": crate::mcp_tools::tool_list()}),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(error(id, -32602, "tools/call needs a tool name"));
            };
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            // The open window first (live mode), otherwise the background.
            let reply = live.call(name, &args).unwrap_or_else(|| session.call(name, &args));
            tool_result(reply)
        }
        _ => return Some(error(id, -32601, &format!("Unknown method {method}"))),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn tool_result(r: Reply) -> Value {
    let mut content = vec![json!({"type": "text", "text": r.text})];
    if let Some(png) = r.png {
        content.push(json!({
            "type": "image",
            "mimeType": "image/png",
            "data": base64::engine::general_purpose::STANDARD.encode(png),
        }));
    }
    json!({"content": content, "isError": r.is_error})
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_tool_list_and_unknown_method() {
        let mut s = Session::new(Vec::new());
        let mut live = crate::mcp_live::LiveClient::off();
        let init = handle(&mut s, &mut live, &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}})).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
        assert!(handle(&mut s, &mut live, &json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
        let list = handle(&mut s, &mut live, &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == "save_image"));
        let bad = handle(&mut s, &mut live, &json!({"jsonrpc": "2.0", "id": 3, "method": "nope"})).unwrap();
        assert_eq!(bad["error"]["code"], -32601);
        let call = handle(&mut s, &mut live, &json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "list_styles", "arguments": {}}})).unwrap();
        assert_eq!(call["result"]["isError"], false);
    }
}
