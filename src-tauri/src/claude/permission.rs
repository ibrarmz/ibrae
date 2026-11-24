//! Answering Claude Code's permission prompts from the island.
//!
//! The `PermissionRequest` hook connects, sends the request and waits on the
//! same connection (see `hookclient.rs`). We keep that connection here until
//! the user clicks Permitir / Siempre / Denegar, then write the answer and close
//! it. Closing without an answer leaves the decision to the terminal.
//!
//! A request goes away when it is answered, when its hook is gone (Claude Code
//! stops it once the user answers in the terminal), when the session moves on
//! (the tool ran, Claude stopped, a new prompt), or when it times out.

use super::{basename, first_line, ipc::Conn, make_diff, Diff};
use serde::Serialize;
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};

/// Main session: the terminal asks at the same time, so waiting costs nothing.
/// Just under the hook's own limit (`WAIT` in hookclient.rs).
const MAIN_WAIT_MS: u64 = 585_000;
/// Background subagents: Claude Code may hold its own prompt until the hook
/// finishes, so give up soon and let the terminal ask.
const AGENT_WAIT_MS: u64 = 30_000;

#[derive(Serialize, Clone)]
pub struct Request {
    id: u64,
    session: String,
    project: String,
    /// Subagent type when a subagent asks ("" = the main session).
    agent: String,
    /// Display name (Bash, Edit, MCP…).
    tool: String,
    /// EJECUTAR | EDITAR | LEER | ABRIR | BUSCAR | USAR
    action: &'static str,
    /// Command, file, URL… (may span lines).
    detail: String,
    diff: Option<Diff>,
    /// What "Siempre" would allow, when Claude Code offered rules.
    always: Option<String>,
    created_at: u64,
    expires_at: u64,
}

struct Pending {
    req: Request,
    conn: Conn,
    agent_id: Option<String>,
    tool: String,
}

#[derive(Default)]
pub struct PermState {
    list: Mutex<Vec<Pending>>,
    next: AtomicU64,
}

/// (action, display tool, detail) for the card.
fn describe(tool: &str, input: &Value) -> (&'static str, String, String) {
    let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let file = super::file_of(input);
    match tool {
        "Bash" | "PowerShell" => ("EJECUTAR", tool.into(), s("command").trim().to_string()),
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => ("EDITAR", tool.into(), file),
        "Read" | "Glob" | "Grep" | "LS" | "NotebookRead" => {
            ("LEER", tool.into(), if file.is_empty() { s("pattern") } else { file })
        }
        "WebFetch" => ("ABRIR", tool.into(), s("url")),
        "WebSearch" => ("BUSCAR", tool.into(), s("query")),
        t if t.starts_with("mcp__") => {
            let mut parts = t.trim_start_matches("mcp__").splitn(2, "__");
            let server = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("");
            ("USAR", "MCP".into(), format!("{server} · {name}"))
        }
        other => ("USAR", other.into(), first_line(&s("description"), 160)),
    }
}

/// Claude Code's suggested rules → "Bash(npm test:*)", "modo aceptar ediciones"…
fn describe_always(suggestions: &Value) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    for sug in suggestions.as_array().into_iter().flatten() {
        match sug["type"].as_str() {
            Some("addRules") | Some("replaceRules") => {
                for r in sug["rules"].as_array().into_iter().flatten() {
                    let tool = r["toolName"].as_str().unwrap_or("");
                    out.push(match r["ruleContent"].as_str() {
                        Some(c) if !c.is_empty() => format!("{tool}({c})"),
                        _ => tool.to_string(),
                    });
                }
            }
            Some("setMode") => out.push(match sug["mode"].as_str() {
                Some("acceptEdits") => "aceptar ediciones".into(),
                Some(m) => format!("modo {m}"),
                None => continue,
            }),
            Some("addDirectories") => {
                for d in sug["directories"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    out.push(format!("carpeta {}", basename(d.trim_end_matches(['/', '\\']))));
                }
            }
            _ => {}
        }
    }
    (!out.is_empty()).then(|| out.join(", "))
}

fn emit(app: &AppHandle, list: &[Pending]) {
    let reqs: Vec<Request> = list.iter().map(|p| p.req.clone()).collect();
    let _ = app.emit("claude-perm", reqs);
}

/// A `PermissionRequest` arrived; hold its connection until the user answers.
pub fn request(app: &AppHandle, ev: Value, conn: Conn) {
    if !crate::settings::get(app).claude_approve {
        return; // dropping `conn` answers "no decision"
    }
    let now = super::now_ms();
    let tool = ev["tool_name"].as_str().unwrap_or("").to_string();
    let input = &ev["tool_input"];
    let (action, shown, detail) = describe(&tool, input);
    let diff = (action == "EDITAR").then(|| make_diff(&tool, &super::file_of(input), input));
    let agent_id = ev["agent_id"].as_str().filter(|a| !a.is_empty()).map(String::from);
    let state = app.state::<PermState>();
    let req = Request {
        id: state.next.fetch_add(1, Ordering::Relaxed) + 1,
        session: ev["session_id"].as_str().unwrap_or("").to_string(),
        project: ev["cwd"].as_str().map(|c| basename(c.trim_end_matches(['/', '\\']))).unwrap_or_default(),
        agent: if agent_id.is_some() { ev["agent_type"].as_str().unwrap_or("Agente").to_string() } else { String::new() },
        tool: shown,
        action,
        detail: if action == "EDITAR" { basename(&detail) } else { detail },
        diff,
        always: describe_always(&ev["suggestions"]),
        created_at: now,
        expires_at: now + if agent_id.is_some() { AGENT_WAIT_MS } else { MAIN_WAIT_MS },
    };
    let mut list = state.list.lock().unwrap();
    list.push(Pending { req, conn, agent_id, tool });
    emit(app, &list);
    drop(list);
    let _ = app.emit("peek", "claude");
}

/// Other hook events: drop requests the session has moved past.
pub fn on_event(app: &AppHandle, ev: &Value) {
    let name = ev["hook_event_name"].as_str().unwrap_or("");
    let session = ev["session_id"].as_str().unwrap_or("");
    let agent = ev["agent_id"].as_str().filter(|a| !a.is_empty());
    let state = app.state::<PermState>();
    let mut list = state.list.lock().unwrap();
    let before = list.len();
    let mine = |p: &Pending| p.req.session == session;
    match name {
        "SessionEnd" => list.retain(|p| !mine(p)),
        // The main thread moved on; subagents may still be waiting.
        "Stop" | "StopFailure" => list.retain(|p| !(mine(p) && p.agent_id.is_none())),
        "UserPromptSubmit" if !super::is_injected(ev["prompt"].as_str().unwrap_or("")) => {
            list.retain(|p| !(mine(p) && p.agent_id.is_none()))
        }
        "SubagentStop" => list.retain(|p| !(mine(p) && p.agent_id.as_deref() == agent)),
        // The tool ran: it was answered in the terminal.
        "PostToolUse" => {
            let tool = ev["tool_name"].as_str().unwrap_or("");
            if let Some(i) = list.iter().position(|p| mine(p) && p.agent_id.as_deref() == agent && p.tool == tool) {
                list.remove(i);
            }
        }
        _ => {}
    }
    if list.len() != before {
        emit(app, &list);
    }
}

/// Drops requests whose hook is gone or that timed out. Cheap when idle.
pub fn start(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        let state = app.state::<PermState>();
        let mut list = state.list.lock().unwrap();
        if list.is_empty() {
            continue;
        }
        let now = super::now_ms();
        let before = list.len();
        list.retain(|p| now < p.req.expires_at && p.conn.alive());
        if list.len() != before {
            emit(&app, &list);
        }
    });
}

#[tauri::command]
pub fn claude_permissions(state: State<PermState>) -> Vec<Request> {
    state.list.lock().unwrap().iter().map(|p| p.req.clone()).collect()
}

/// decision: "allow" | "always" | "deny".
#[tauri::command]
pub fn claude_permission(app: AppHandle, id: u64, decision: String) -> Result<(), String> {
    if !matches!(decision.as_str(), "allow" | "always" | "deny") {
        return Err("Respuesta no válida".into());
    }
    let state = app.state::<PermState>();
    let pending = {
        let mut list = state.list.lock().unwrap();
        let i = list.iter().position(|p| p.req.id == id).ok_or("Claude ya no espera esta respuesta")?;
        let p = list.remove(i);
        emit(&app, &list);
        p
    };
    let alive = pending.conn.alive();
    // Writing waits for the hook to read it; keep that off the UI thread.
    thread::spawn(move || pending.conn.reply(serde_json::json!({ "decision": decision }).to_string().as_bytes()));
    if alive {
        Ok(())
    } else {
        Err("Claude ya no espera esta respuesta".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn describes_requests() {
        let (a, t, d) = describe("Bash", &json!({"command":"npm test\n"}));
        assert_eq!((a, t.as_str(), d.as_str()), ("EJECUTAR", "Bash", "npm test"));
        let (a, t, d) = describe("mcp__github__create_issue", &json!({}));
        assert_eq!((a, t.as_str(), d.as_str()), ("USAR", "MCP", "github · create_issue"));
        assert_eq!(describe("Edit", &json!({"file_path":"/a/b.rs"})).2, "/a/b.rs");
    }

    #[test]
    fn describes_always() {
        let s = json!([
            {"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"npm test:*"},{"toolName":"WebSearch"}]},
            {"type":"setMode","mode":"acceptEdits"},
            {"type":"addDirectories","directories":["C:\\dev\\otro\\"]}
        ]);
        assert_eq!(describe_always(&s).unwrap(), "Bash(npm test:*), WebSearch, aceptar ediciones, carpeta otro");
        assert!(describe_always(&Value::Null).is_none());
    }
}
