//! What Claude Code is doing, fed by its hooks.
//!
//! Each hook event updates one session; the UI shows the most recently active
//! one. Tools map to phases: Read/Grep → LEER, Edit/Write → EDITAR,
//! Bash with test/build → PROBAR, Stop → LISTO.

pub mod hookclient;
pub mod hooks;
pub mod ipc;
pub mod permission;

use serde::Serialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State};

const MAX_LINES: usize = 6;
const MAX_LINE_LEN: usize = 160;
/// A subagent silent for this long no longer counts as running.
const AGENT_STALE_MS: u64 = 600_000;

#[derive(Serialize, Clone, Default)]
pub struct Diff {
    file: String,
    removed: Vec<String>,
    added: Vec<String>,
    add: usize,
    del: usize,
}

/// One subagent spawned by a session.
#[derive(Serialize, Clone)]
pub struct Agent {
    id: String,
    /// agent_type (Explore, Plan…)
    kind: String,
    task: String,
    label: &'static str,
    detail: String,
    /// working | waiting | done
    status: &'static str,
    started_at: u64,
    ended_at: Option<u64>,
    tools: u32,
    message: Option<String>,
    /// Arrival time of its latest event; long-silent agents are dropped on a new prompt.
    #[serde(skip)]
    last_seen: u64,
    /// Hook start time of its newest event applied; older arrivals are stale.
    #[serde(skip)]
    last_ts: u64,
}

impl Agent {
    fn new(id: String, kind: String, task: String, now: u64) -> Self {
        Self {
            id,
            kind,
            task,
            label: "TRABAJANDO",
            detail: String::new(),
            status: "working",
            started_at: now,
            ended_at: None,
            tools: 0,
            message: None,
            last_seen: now,
            last_ts: 0,
        }
    }

    fn tool_use(&mut self, tool: &str, input: &Value) {
        let (_, label, detail) = describe_tool(tool, input);
        self.status = "working";
        self.message = None;
        self.tools += 1;
        self.label = label;
        self.detail = detail;
    }
}

#[derive(Serialize, Clone)]
pub struct Session {
    id: String,
    project: String,
    prompt: String,
    /// idle | working | waiting | done
    status: &'static str,
    /// Spanish label shown in the UI (EDITANDO, PROBANDO…)
    label: &'static str,
    /// 0 read · 1 edit · 2 test · 3 done
    phase: usize,
    reached: [bool; 4],
    /// Current file / command / pattern.
    detail: String,
    tool: String,
    diff: Option<Diff>,
    started_at: u64,
    ended_at: Option<u64>,
    tools: u32,
    message: Option<String>,
    agents: Vec<Agent>,
    /// (subagent_type, description) of Agent calls whose SubagentStart hasn't arrived yet.
    #[serde(skip)]
    pending: Vec<(String, String)>,
    updated_at: u64,
    /// Hook start time of the newest event applied; older arrivals are stale.
    #[serde(skip)]
    last_ts: u64,
}

impl Session {
    fn new(id: String, now: u64) -> Self {
        Self {
            id,
            project: String::new(),
            prompt: String::new(),
            status: "idle",
            label: "LISTO",
            phase: 0,
            reached: [false; 4],
            detail: String::new(),
            tool: String::new(),
            diff: None,
            started_at: now,
            ended_at: None,
            tools: 0,
            message: None,
            agents: Vec::new(),
            pending: Vec::new(),
            updated_at: now,
            last_ts: 0,
        }
    }

    /// Injected prompts (background agent reports) continue the current task.
    fn resume(&mut self) {
        self.status = "working";
        self.label = "PENSANDO";
        self.ended_at = None;
        self.message = None;
    }

    fn start_task(&mut self, prompt: &str, now: u64) {
        // Agents still running (seen recently) outlive the new prompt.
        let mut agents = std::mem::take(&mut self.agents);
        agents.retain(|a| a.status != "done" && now.saturating_sub(a.last_seen) <= AGENT_STALE_MS);
        *self = Session {
            project: std::mem::take(&mut self.project),
            last_ts: self.last_ts,
            agents,
            ..Session::new(std::mem::take(&mut self.id), now)
        };
        self.prompt = prompt.trim().to_string();
        self.status = "working";
        self.label = "PENSANDO";
    }

    fn tool_use(&mut self, tool: &str, input: &Value, now: u64) {
        if self.status != "working" {
            // Tools without a prompt we saw (e.g. resumed session): start timing now.
            if self.status != "waiting" {
                self.started_at = now;
                self.ended_at = None;
                self.reached = [false; 4];
            }
            self.status = "working";
        }
        self.message = None;
        self.tools += 1;
        self.tool = tool.to_string();

        let (phase, label, detail) = describe_tool(tool, input);
        if phase == Some(1) {
            self.diff = Some(make_diff(tool, &file_of(input), input));
        }
        if let Some(p) = phase {
            self.phase = p;
            self.reached[p] = true;
        }
        self.label = label;
        self.detail = detail;
    }
}

#[derive(Default)]
pub struct ClaudeState {
    sessions: Mutex<HashMap<String, Session>>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

fn file_of(input: &Value) -> String {
    let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("");
    [s("file_path"), s("notebook_path"), s("path")].into_iter().find(|v| !v.is_empty()).unwrap_or("").to_string()
}

/// Tool call -> (phase it reaches, Spanish label, current detail).
fn describe_tool(tool: &str, input: &Value) -> (Option<usize>, &'static str, String) {
    let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("");
    let file = file_of(input);
    match tool {
        "Read" | "Grep" | "Glob" | "LS" | "WebFetch" | "WebSearch" | "NotebookRead" => {
            let d = [basename(&file), s("pattern").into(), s("url").into(), s("query").into()]
                .into_iter()
                .find(|v| !v.is_empty())
                .unwrap_or_default();
            (Some(0), "LEYENDO", d)
        }
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => (Some(1), "EDITANDO", basename(&file)),
        "Bash" | "PowerShell" => {
            let cmd = first_line(s("command"), 80);
            if is_check(&cmd) {
                (Some(2), "PROBANDO", cmd)
            } else {
                (None, "EJECUTANDO", cmd)
            }
        }
        "Task" | "Agent" => (None, "DELEGANDO", first_line(s("description"), 80)),
        "TodoWrite" => (None, "PLANEANDO", String::new()),
        other => (None, "TRABAJANDO", other.to_string()),
    }
}

fn basename(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "…"
    }
}

fn first_line(s: &str, n: usize) -> String {
    truncate(s.lines().next().unwrap_or("").trim(), n)
}

/// Commands that check the work: tests, builds, linters, type checks.
fn is_check(cmd: &str) -> bool {
    let c = cmd.to_lowercase();
    [
        "test", "jest", "vitest", "pytest", "mocha", "cargo check", "cargo build", "cargo clippy",
        "npm run build", "pnpm build", "yarn build", "tsc", "lint", "dotnet build", "go vet", "go build",
        "mvn ", "gradle", "phpunit", "rspec",
    ]
    .iter()
    .any(|k| c.contains(k))
}

fn sample(text: &str) -> (Vec<String>, usize) {
    if text.is_empty() {
        return (Vec::new(), 0);
    }
    let lines: Vec<&str> = text.lines().collect();
    (lines.iter().take(MAX_LINES).map(|l| truncate(l, MAX_LINE_LEN)).collect(), lines.len())
}

fn make_diff(tool: &str, file: &str, input: &Value) -> Diff {
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let (old, new) = match tool {
        "Write" => (String::new(), s(input, "content")),
        "MultiEdit" => input
            .get("edits")
            .and_then(|e| e.get(0))
            .map(|e| (s(e, "old_string"), s(e, "new_string")))
            .unwrap_or_default(),
        "NotebookEdit" => (String::new(), s(input, "new_source")),
        _ => (s(input, "old_string"), s(input, "new_string")),
    };
    let (removed, del) = sample(&old);
    let (added, add) = sample(&new);
    Diff { file: basename(file), removed, added, add, del }
}

fn latest(map: &HashMap<String, Session>) -> Option<Session> {
    map.values().max_by_key(|s| s.updated_at).cloned()
}

pub(crate) struct Applied {
    pub changed: bool,
    pub peek: bool,
}

fn agent_entry<'a>(s: &'a mut Session, id: &str, kind: &str, now: u64) -> &'a mut Agent {
    let i = match s.agents.iter().position(|a| a.id == id) {
        Some(i) => i,
        None => {
            s.agents.push(Agent::new(id.to_string(), kind.to_string(), String::new(), now));
            s.agents.len() - 1
        }
    };
    let a = &mut s.agents[i];
    if a.kind.is_empty() && !kind.is_empty() {
        a.kind = kind.to_string();
    }
    a.last_seen = now;
    a
}

/// Prompts Claude Code injects itself (background agent reports), not typed by the user.
fn is_injected(prompt: &str) -> bool {
    let p = prompt.trim_start();
    p.starts_with("<task-notification") || p.starts_with("<agent-message")
}

/// Apply one hook event to the session map (pure: no I/O).
pub(crate) fn apply(map: &mut HashMap<String, Session>, ev: &Value, now: u64) -> Applied {
    let name = ev["hook_event_name"].as_str().unwrap_or("");
    let id = ev["session_id"].as_str().unwrap_or("default").to_string();

    if name == "SessionEnd" {
        map.remove(&id);
        return Applied { changed: true, peek: false };
    }

    let s = map.entry(id.clone()).or_insert_with(|| Session::new(id, now));
    // Async hooks may arrive out of order: ignore anything older than what we applied
    // on the same stream (main thread, or one subagent). Start/Stop are never stale.
    let ts = ev["ts"].as_u64().unwrap_or(now);
    let agent_id = ev["agent_id"].as_str().filter(|a| !a.is_empty());
    let agent_stream = agent_id.is_some() && matches!(name, "PreToolUse" | "PostToolUse" | "Notification");
    let lifecycle = matches!(name, "SubagentStart" | "SubagentStop");
    if agent_stream {
        let seen = s.agents.iter().find(|a| Some(a.id.as_str()) == agent_id).map(|a| a.last_ts);
        if seen.is_some_and(|l| ts < l) {
            return Applied { changed: false, peek: false };
        }
    } else if !lifecycle {
        if ts < s.last_ts {
            return Applied { changed: false, peek: false };
        }
        s.last_ts = ts;
    }
    if let Some(cwd) = ev["cwd"].as_str() {
        s.project = basename(cwd.trim_end_matches(['/', '\\']));
    }
    s.updated_at = now;

    let agent_kind = ev["agent_type"].as_str().unwrap_or("");
    let mut peek = false;

    // Events fired from inside a subagent only touch that agent.
    if let (Some(aid), "PreToolUse" | "PostToolUse" | "Notification") = (agent_id, name) {
        let a = agent_entry(s, aid, agent_kind, now);
        a.last_ts = ts;
        match name {
            "PreToolUse" => a.tool_use(ev["tool_name"].as_str().unwrap_or(""), &ev["tool_input"]),
            "PostToolUse" => {
                if a.status == "waiting" {
                    a.status = "working";
                    a.label = "TRABAJANDO";
                    a.message = None;
                }
            }
            _ => {
                let msg = ev["message"].as_str().unwrap_or("");
                if is_permission(ev, msg) {
                    a.status = "waiting";
                    a.label = "ESPERA";
                    a.message = Some(msg.to_string());
                    peek = true;
                }
            }
        }
        return Applied { changed: true, peek };
    }

    match name {
        "UserPromptSubmit" => {
            let prompt = ev["prompt"].as_str().unwrap_or("");
            if is_injected(prompt) {
                s.resume();
            } else {
                s.start_task(prompt, now);
            }
        }
        "PreToolUse" => {
            let tool = ev["tool_name"].as_str().unwrap_or("");
            let input = &ev["tool_input"];
            if matches!(tool, "Agent" | "Task") {
                let g = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                s.pending.push((g("subagent_type"), first_line(&g("description"), 80)));
            }
            s.tool_use(tool, input, now)
        }
        "PostToolUse" => {
            if s.status == "waiting" {
                s.status = "working";
                s.label = "TRABAJANDO";
                s.message = None;
            }
        }
        "Notification" => {
            let msg = ev["message"].as_str().unwrap_or("");
            if is_permission(ev, msg) {
                s.status = "waiting";
                s.label = "ESPERA";
                s.message = Some(msg.to_string());
                peek = true;
            }
        }
        "SubagentStart" => {
            if let Some(aid) = agent_id {
                let i = s.pending.iter().position(|(k, _)| k == agent_kind).or(if s.pending.is_empty() { None } else { Some(0) });
                let task = i.map(|i| s.pending.remove(i).1).unwrap_or_default();
                let a = agent_entry(s, aid, agent_kind, now);
                a.last_ts = a.last_ts.max(ts);
                if a.task.is_empty() {
                    a.task = task;
                }
            }
        }
        "SubagentStop" => {
            if let Some(aid) = agent_id {
                let a = agent_entry(s, aid, agent_kind, now);
                a.last_ts = a.last_ts.max(ts);
                a.status = "done";
                a.label = "LISTO";
                a.ended_at = Some(now);
                a.message = None;
            }
        }
        "Stop" => {
            s.status = "done";
            s.label = "LISTO";
            s.phase = 3;
            s.reached = [true; 4];
            s.ended_at = Some(now);
            s.message = None;
        }
        // The turn died on an API error (no Stop follows): stop the clock and say why.
        "StopFailure" => {
            let kind = ev["error"].as_str().unwrap_or("unknown");
            s.status = "error";
            s.label = error_label(kind);
            s.ended_at = Some(now);
            s.message = Some(error_message(ev, kind));
            // Subagents die with the turn.
            for a in s.agents.iter_mut().filter(|a| a.status != "done") {
                a.status = "done";
                a.label = "DETENIDO";
                a.ended_at = Some(now);
                a.message = None;
            }
            peek = true;
        }
        _ => {}
    }
    Applied { changed: true, peek }
}

fn error_label(kind: &str) -> &'static str {
    match kind {
        "rate_limit" => "LÍMITE",
        "authentication_failed" | "oauth_org_not_allowed" | "cloud_credential_error" => "SIN SESIÓN",
        "billing_error" | "account_on_hold" => "CUENTA",
        "server_error" => "SIN SERVICIO",
        _ => "ERROR",
    }
}

/// Claude Code's own error text ("You've hit your 5-hour limit · resets 1pm…")
/// when it sends one; a Spanish fallback otherwise.
fn error_message(ev: &Value, kind: &str) -> String {
    let text = |k: &str| ev[k].as_str().map(str::trim).filter(|t| !t.is_empty()).map(|t| first_line(t, MAX_LINE_LEN));
    text("last_assistant_message").or_else(|| text("error_details")).unwrap_or_else(|| {
        match kind {
            "rate_limit" => "Llegaste al límite de uso de Claude.",
            "server_error" => "La API de Claude no responde.",
            _ => "El turno terminó por un error de la API.",
        }
        .to_string()
    })
}

/// Only permission prompts matter; idle_prompt fires long after Stop.
/// Older Claude Code versions send no notification_type: fall back to the text.
fn is_permission(ev: &Value, msg: &str) -> bool {
    match ev["notification_type"].as_str() {
        Some(kind) => kind == "permission_prompt",
        None => !msg.to_lowercase().contains("waiting for your input"),
    }
}

/// A message from a hook client. Permission requests keep their connection to
/// answer later; anything else closes it right away.
pub fn receive(app: &AppHandle, ev: Value, conn: ipc::Conn) {
    if ev["hook_event_name"] == "PermissionRequest" {
        return permission::request(app, ev, conn);
    }
    drop(conn);
    permission::on_event(app, &ev);
    handle(app, ev);
}

/// The island side: apply one hook event and tell the UI.
pub fn handle(app: &AppHandle, ev: Value) {
    let state = app.state::<ClaudeState>();
    let mut map = state.sessions.lock().unwrap();
    let r = apply(&mut map, &ev, now_ms());
    if !r.changed {
        return;
    }
    let _ = app.emit("claude", latest(&map));
    drop(map);
    if r.peek {
        let _ = app.emit("peek", "claude");
    }
}

#[tauri::command]
pub fn claude_snapshot(state: State<ClaudeState>) -> Option<Session> {
    latest(&state.sessions.lock().unwrap())
}

/// action: "status" | "install" | "uninstall". Returns whether hooks are installed.
#[tauri::command]
pub fn claude_hooks(action: String) -> Result<bool, String> {
    match action.as_str() {
        "install" => hooks::install()?,
        "uninstall" => hooks::uninstall()?,
        _ => {}
    }
    hooks::installed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: u64 = 1_000;

    fn run(evs: Vec<Value>) -> (HashMap<String, Session>, Vec<Applied>) {
        let mut map = HashMap::new();
        let mut out = Vec::new();
        for (i, mut ev) in evs.into_iter().enumerate() {
            ev["session_id"] = json!("s1");
            ev["ts"] = json!(i as u64 + 1);
            out.push(apply(&mut map, &ev, NOW));
        }
        (map, out)
    }

    fn agent_call(desc: &str) -> Value {
        json!({"hook_event_name":"PreToolUse","tool_name":"Agent",
               "tool_input":{"description":desc,"subagent_type":"Explore"}})
    }

    fn start(id: &str) -> Value {
        json!({"hook_event_name":"SubagentStart","agent_id":id,"agent_type":"Explore"})
    }

    #[test]
    fn main_flow_unchanged() {
        let (map, _) = run(vec![
            json!({"hook_event_name":"UserPromptSubmit","prompt":"hola"}),
            json!({"hook_event_name":"PreToolUse","tool_name":"Read","tool_input":{"file_path":"/a/b.rs"}}),
            json!({"hook_event_name":"Stop"}),
        ]);
        let s = &map["s1"];
        assert_eq!(s.status, "done");
        assert_eq!(s.label, "LISTO");
        assert!(s.agents.is_empty());
    }

    #[test]
    fn agent_tool_queues_task_and_start_takes_it() {
        let (map, _) = run(vec![agent_call("Buscar RAM"), start("a1")]);
        let s = &map["s1"];
        assert_eq!(s.agents.len(), 1);
        assert_eq!(s.agents[0].task, "Buscar RAM");
        assert_eq!(s.agents[0].status, "working");
        assert!(s.pending.is_empty());
    }

    #[test]
    fn parallel_same_type_fifo() {
        let (map, _) = run(vec![agent_call("A"), agent_call("B"), start("a1"), start("a2")]);
        let s = &map["s1"];
        assert_eq!(s.agents[0].task, "A");
        assert_eq!(s.agents[1].task, "B");
    }

    #[test]
    fn agent_events_do_not_touch_session() {
        let (map, _) = run(vec![
            agent_call("X"),
            start("a1"),
            json!({"hook_event_name":"PreToolUse","agent_id":"a1","agent_type":"Explore",
                   "tool_name":"Edit","tool_input":{"file_path":"/a/b.rs","old_string":"x","new_string":"y"}}),
        ]);
        let s = &map["s1"];
        assert_eq!(s.agents[0].label, "EDITANDO");
        assert_eq!(s.agents[0].tools, 1);
        assert!(s.diff.is_none());
        assert_eq!(s.label, "DELEGANDO");
    }

    #[test]
    fn unknown_agent_created() {
        let (map, _) = run(vec![json!({"hook_event_name":"PreToolUse","agent_id":"zz","agent_type":"Plan",
            "tool_name":"Read","tool_input":{"file_path":"/a/b.rs"}})]);
        let a = &map["s1"].agents[0];
        assert_eq!(a.id, "zz");
        assert_eq!(a.kind, "Plan");
        assert_eq!(a.task, "");
    }

    #[test]
    fn agent_permission_peeks() {
        let (map, r) = run(vec![
            start("a1"),
            json!({"hook_event_name":"Notification","agent_id":"a1","notification_type":"permission_prompt",
                   "message":"Claude needs your permission to use Bash"}),
        ]);
        let a = &map["s1"].agents[0];
        assert_eq!(a.status, "waiting");
        assert_eq!(a.label, "ESPERA");
        assert!(r[1].peek);

        let (map, _) = run(vec![
            start("a1"),
            json!({"hook_event_name":"Notification","agent_id":"a1","notification_type":"permission_prompt","message":"m"}),
            json!({"hook_event_name":"PostToolUse","agent_id":"a1","tool_name":"Bash"}),
        ]);
        assert_eq!(map["s1"].agents[0].status, "working");
    }

    #[test]
    fn stop_failure_stops_the_clock() {
        let mut map = HashMap::new();
        ap(&mut map, json!({"hook_event_name":"UserPromptSubmit","prompt":"hola"}), 1, 1_000);
        ap(&mut map, json!({"hook_event_name":"SubagentStart","agent_id":"a1","agent_type":"Explore"}), 2, 1_000);
        let r = ap(
            &mut map,
            json!({"hook_event_name":"StopFailure","error":"rate_limit",
                   "last_assistant_message":"You've hit your 5-hour limit · resets 1pm"}),
            3,
            5_000,
        );
        assert!(r.peek);
        let s = map.values().next().unwrap();
        assert_eq!((s.status, s.label, s.ended_at), ("error", "LÍMITE", Some(5_000)));
        assert_eq!(s.message.as_deref(), Some("You've hit your 5-hour limit · resets 1pm"));
        assert!(s.agents.iter().all(|a| a.status == "done"));
        // A new prompt after the reset starts over.
        ap(&mut map, json!({"hook_event_name":"UserPromptSubmit","prompt":"sigue"}), 4, 9_000);
        assert_eq!(map.values().next().unwrap().status, "working");
    }

    #[test]
    fn subagent_stop_marks_done() {
        let (map, _) = run(vec![
            start("a1"),
            json!({"hook_event_name":"SubagentStop","agent_id":"a1","agent_type":"Explore"}),
        ]);
        let a = &map["s1"].agents[0];
        assert_eq!(a.status, "done");
        assert_eq!(a.label, "LISTO");
        assert_eq!(a.ended_at, Some(NOW));
    }

    /// Apply with explicit hook timestamp and arrival time.
    fn ap(map: &mut HashMap<String, Session>, mut ev: Value, ts: u64, now: u64) -> Applied {
        ev["session_id"] = json!("s1");
        ev["ts"] = json!(ts);
        apply(map, &ev, now)
    }

    #[test]
    fn new_prompt_keeps_active_agents() {
        let mut map = HashMap::new();
        ap(&mut map, agent_call("A"), 1, 1_000);
        ap(&mut map, start("a1"), 2, 1_000);
        ap(&mut map, start("a2"), 3, 1_000);
        ap(&mut map, json!({"hook_event_name":"SubagentStop","agent_id":"a2","agent_type":"Explore"}), 4, 1_000);
        ap(&mut map, start("a3"), 5, 1_000);
        // a1 stays active, a3 goes silent for more than 10 minutes.
        ap(&mut map, json!({"hook_event_name":"PreToolUse","agent_id":"a1","agent_type":"Explore",
            "tool_name":"Read","tool_input":{"file_path":"/a"}}), 6, 700_000);
        ap(&mut map, agent_call("B"), 7, 700_000);
        ap(&mut map, json!({"hook_event_name":"UserPromptSubmit","prompt":"otra"}), 8, 700_000 + 1_000);
        let s = &map["s1"];
        let ids: Vec<&str> = s.agents.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["a1"]);
        assert!(s.pending.is_empty());
        assert_eq!(s.prompt, "otra");
    }

    #[test]
    fn injected_prompt_keeps_agents_and_prompt() {
        let (map, _) = run(vec![
            json!({"hook_event_name":"UserPromptSubmit","prompt":"haz algo"}),
            agent_call("A"),
            start("a1"),
            json!({"hook_event_name":"SubagentStop","agent_id":"a1","agent_type":"Explore"}),
            json!({"hook_event_name":"Stop"}),
            json!({"hook_event_name":"UserPromptSubmit","prompt":"  <task-notification>\nx</task-notification>"}),
        ]);
        let s = &map["s1"];
        assert_eq!(s.prompt, "haz algo");
        assert_eq!(s.agents.len(), 1);
        assert_eq!(s.status, "working");
        assert_eq!(s.label, "PENSANDO");
        assert!(s.ended_at.is_none());
        assert!(s.reached.iter().all(|r| *r));
        assert_eq!(s.phase, 3);

        let (map, _) = run(vec![
            json!({"hook_event_name":"UserPromptSubmit","prompt":"haz algo"}),
            start("a1"),
            json!({"hook_event_name":"UserPromptSubmit","prompt":"<agent-message from=\"x\">hi"}),
        ]);
        assert_eq!(map["s1"].prompt, "haz algo");
        assert_eq!(map["s1"].agents.len(), 1);

        // A generic leading "<" is a real prompt.
        let (map, _) = run(vec![
            json!({"hook_event_name":"UserPromptSubmit","prompt":"uno"}),
            json!({"hook_event_name":"UserPromptSubmit","prompt":"<div> que es esto"}),
        ]);
        assert_eq!(map["s1"].prompt, "<div> que es esto");
    }

    #[test]
    fn out_of_order_across_agents_not_dropped() {
        let mut map = HashMap::new();
        let tool = |id: &str| json!({"hook_event_name":"PreToolUse","agent_id":id,"agent_type":"Explore",
            "tool_name":"Read","tool_input":{"file_path":"/a"}});
        ap(&mut map, start("a1"), 1, NOW);
        ap(&mut map, start("a2"), 2, NOW);
        assert!(ap(&mut map, tool("a1"), 50, NOW).changed);
        // Older than a1's last event but a different stream.
        assert!(ap(&mut map, tool("a2"), 40, NOW).changed);
        assert_eq!(map["s1"].agents[1].tools, 1);
        // Same stream, older: dropped.
        assert!(!ap(&mut map, tool("a1"), 45, NOW).changed);
        assert_eq!(map["s1"].agents[0].tools, 1);
        // Main thread has its own clock.
        assert!(ap(&mut map, json!({"hook_event_name":"Stop"}), 10, NOW).changed);
        // Stop of a subagent is never stale.
        assert!(ap(&mut map, json!({"hook_event_name":"SubagentStop","agent_id":"a1","agent_type":"Explore"}), 3, NOW).changed);
        assert_eq!(map["s1"].agents[0].status, "done");
    }

    #[test]
    fn kind_filled_in_and_main_wait_label_restored() {
        let (map, _) = run(vec![
            json!({"hook_event_name":"PreToolUse","agent_id":"a1","tool_name":"Read","tool_input":{}}),
            json!({"hook_event_name":"PreToolUse","agent_id":"a1","agent_type":"Plan","tool_name":"Read","tool_input":{}}),
        ]);
        assert_eq!(map["s1"].agents[0].kind, "Plan");

        let (map, _) = run(vec![
            json!({"hook_event_name":"PreToolUse","tool_name":"Read","tool_input":{}}),
            json!({"hook_event_name":"Notification","notification_type":"permission_prompt","message":"m"}),
            json!({"hook_event_name":"PostToolUse","tool_name":"Read"}),
        ]);
        assert_eq!(map["s1"].status, "working");
        assert_eq!(map["s1"].label, "TRABAJANDO");
    }

    #[test]
    fn stale_event_is_dropped() {
        let mut map = HashMap::new();
        let ev = |ts: u64| json!({"session_id":"s1","ts":ts,"hook_event_name":"Stop"});
        assert!(apply(&mut map, &ev(5), NOW).changed);
        assert!(!apply(&mut map, &ev(4), NOW).changed);
    }
}
