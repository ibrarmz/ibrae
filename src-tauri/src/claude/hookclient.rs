//! The Claude Code side of the hook: read the event from stdin, keep only what
//! the island uses, and hand it over the local channel. Shared verbatim by the
//! tiny `island-hook` binary (std + serde_json only, so it starts in a few ms)
//! and by `ibrae hook`. Never fails loudly: if the island isn't
//! running, Claude Code must not notice.
//!
//! Every event is fire-and-forget except `PermissionRequest`: that one waits on
//! the connection for the user's answer in the island ("allow" / "always" /
//! "deny") and prints Claude Code's decision. No answer (island closed, feature
//! off, timed out, answered in the terminal) prints nothing, which leaves
//! Claude Code's own prompt in charge.

use serde_json::{json, Map, Value};
use std::io::Read;

const MAX_FIELD: usize = 4000;

pub fn run() {
    // Taken first thing: hooks run async, so events can arrive out of order and
    // the island uses this to drop stale ones.
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        return;
    }
    if let Ok(ev) = serde_json::from_str::<Value>(&raw) {
        let mut msg = slim(&ev);
        msg["ts"] = json!(ts);
        if ev["hook_event_name"] == "PermissionRequest" {
            permission(&ev, msg);
        } else {
            let _ = send(msg.to_string().as_bytes());
        }
    }
}

/// Just under the hook's `timeout` in settings.json (600 s), so we print
/// nothing instead of being killed.
const WAIT: std::time::Duration = std::time::Duration::from_secs(590);

const DENY_MESSAGE: &str =
    "The user denied this from the IBRAE desktop island. Don't retry it; continue without it or ask the user how to proceed.";

fn permission(ev: &Value, mut msg: Value) {
    // These need an answer only the terminal can give (choices, plan review).
    if matches!(ev["tool_name"].as_str(), Some("AskUserQuestion" | "ExitPlanMode")) {
        return;
    }
    msg["suggestions"] = ev.get("permission_suggestions").cloned().unwrap_or(Value::Null);
    let Some(reply) = ask(msg.to_string()) else { return };
    let decision = match serde_json::from_str::<Value>(&reply).ok().and_then(|v| v["decision"].as_str().map(String::from)) {
        Some(d) => d,
        None => return,
    };
    if let Some(out) = decide(&decision, ev) {
        println!("{out}");
    }
}

/// The island's answer → what Claude Code reads on stdout.
pub fn decide(decision: &str, ev: &Value) -> Option<Value> {
    let d = match decision {
        "allow" => json!({ "behavior": "allow" }),
        "always" => {
            let mut d = json!({ "behavior": "allow" });
            // The same rules the terminal's "don't ask again" would add.
            if let Some(s) = ev.get("permission_suggestions").filter(|s| s.as_array().is_some_and(|a| !a.is_empty())) {
                d["updatedPermissions"] = s.clone();
            }
            d
        }
        "deny" => json!({ "behavior": "deny", "message": DENY_MESSAGE }),
        _ => return None,
    };
    Some(json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": d } }))
}

/// Send `msg` and wait for the island's answer (empty = no answer).
fn ask(msg: String) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(request(format!("{msg}\n").as_bytes()));
    });
    // On timeout the reader thread dies with the process.
    rx.recv_timeout(WAIT).ok().flatten().filter(|r| !r.trim().is_empty())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "…"
    }
}

/// Keep only the fields the island reads; cut long strings.
pub fn slim(ev: &Value) -> Value {
    let cut = |v: &Value| match v.as_str() {
        Some(s) if s.len() > MAX_FIELD => Value::String(truncate(s, MAX_FIELD)),
        _ => v.clone(),
    };
    let mut input = Map::new();
    if let Some(ti) = ev.get("tool_input").and_then(Value::as_object) {
        for k in [
            "file_path", "notebook_path", "path", "pattern", "url", "query", "command", "description",
            "old_string", "new_string", "content", "new_source", "subagent_type",
        ] {
            if let Some(v) = ti.get(k) {
                input.insert(k.into(), cut(v));
            }
        }
        if let Some(first) = ti.get("edits").and_then(|e| e.get(0)) {
            input.insert(
                "edits".into(),
                json!([{ "old_string": cut(&first["old_string"]), "new_string": cut(&first["new_string"]) }]),
            );
        }
    }
    json!({
        "session_id": ev.get("session_id"),
        "cwd": ev.get("cwd"),
        "hook_event_name": ev.get("hook_event_name"),
        "tool_name": ev.get("tool_name"),
        "agent_id": ev.get("agent_id"),
        "agent_type": ev.get("agent_type"),
        "prompt": ev.get("prompt").map(cut),
        "message": ev.get("message").map(cut),
        "notification_type": ev.get("notification_type"),
        "error": ev.get("error"),
        "error_details": ev.get("error_details").map(cut),
        "last_assistant_message": ev.get("last_assistant_message").map(cut),
        "tool_input": input,
    })
}

#[cfg(windows)]
// Kept from the "Dynamic Island" days so old hooks and islands still talk.
pub const PIPE_NAME: &str = r"\\.\pipe\dynamic-island";

#[cfg(windows)]
pub fn send(msg: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    // ERROR_PIPE_BUSY (231): the island is between clients; retry briefly.
    for _ in 0..10 {
        match std::fs::OpenOptions::new().write(true).open(PIPE_NAME) {
            Ok(mut pipe) => return pipe.write_all(msg),
            Err(e) if e.raw_os_error() == Some(231) => std::thread::sleep(std::time::Duration::from_millis(15)),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Write `msg`, then read until the island closes the connection.
#[cfg(windows)]
fn request(msg: &[u8]) -> Option<String> {
    use std::io::Write;
    let mut pipe = None;
    for _ in 0..10 {
        match std::fs::OpenOptions::new().read(true).write(true).open(PIPE_NAME) {
            Ok(p) => {
                pipe = Some(p);
                break;
            }
            Err(e) if e.raw_os_error() == Some(231) => std::thread::sleep(std::time::Duration::from_millis(15)),
            // Not running, or an island too old to answer (its pipe is write-only).
            Err(_) => return None,
        }
    }
    let mut pipe = pipe?;
    pipe.write_all(msg).ok()?;
    read_all(&mut pipe)
}

fn read_all(r: &mut impl std::io::Read) -> Option<String> {
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match r.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(unix)]
fn request(msg: &[u8]) -> Option<String> {
    use std::io::Write;
    let mut s = std::os::unix::net::UnixStream::connect(socket_path()).ok()?;
    s.write_all(msg).ok()?;
    read_all(&mut s)
}

#[cfg(unix)]
pub fn socket_path() -> std::path::PathBuf {
    use std::path::PathBuf;
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => PathBuf::from(dir).join("dynamic-island.sock"),
        None => {
            extern "C" {
                fn getuid() -> u32;
            }
            PathBuf::from(format!("/tmp/dynamic-island-{}.sock", unsafe { getuid() }))
        }
    }
}

#[cfg(unix)]
pub fn send(msg: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    std::os::unix::net::UnixStream::connect(socket_path())?.write_all(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slim_keeps_subagent_fields() {
        let ev = json!({"hook_event_name":"PreToolUse","session_id":"s","agent_id":"a1","agent_type":"Explore",
          "tool_name":"Agent","tool_input":{"description":"Buscar","subagent_type":"Explore","prompt":"x"}});
        let m = slim(&ev);
        assert_eq!(m["agent_id"], "a1");
        assert_eq!(m["agent_type"], "Explore");
        assert_eq!(m["tool_input"]["subagent_type"], "Explore");
        assert!(m["tool_input"].get("prompt").is_none()); // still slimmed
    }

    #[test]
    fn decisions() {
        let ev = json!({"permission_suggestions":[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"npm test"}],
            "behavior":"allow","destination":"localSettings"}]});
        let d = |x| decide(x, &ev).map(|v| v["hookSpecificOutput"]["decision"].clone());
        assert_eq!(d("allow").unwrap(), json!({"behavior":"allow"}));
        assert_eq!(d("always").unwrap()["updatedPermissions"][0]["rules"][0]["ruleContent"], "npm test");
        assert_eq!(d("deny").unwrap()["behavior"], "deny");
        assert!(d("whatever").is_none());
        // No suggestions: "always" is a plain allow.
        assert!(decide("always", &json!({})).unwrap()["hookSpecificOutput"]["decision"].get("updatedPermissions").is_none());
    }
}
