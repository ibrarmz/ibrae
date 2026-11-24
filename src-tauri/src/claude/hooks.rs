//! Adds / removes the island's hooks in ~/.claude/settings.json.
//! Other hooks and settings are left untouched; a backup is written first.

use serde_json::{json, Value};
use std::{fs, path::PathBuf};

const EVENTS: [(&str, bool); 11] = [
    // (event, needs a tool matcher)
    ("PreToolUse", true),
    ("PostToolUse", true),
    ("UserPromptSubmit", false),
    ("Notification", false),
    ("Stop", false),
    // Instead of Stop when the turn dies on an API error (usage limit, auth…).
    ("StopFailure", false),
    ("SessionStart", false),
    ("SessionEnd", false),
    ("SubagentStart", false),
    ("SubagentStop", false),
    ("PermissionRequest", true),
];

/// The one event Claude Code waits on: the hook holds it until the user answers
/// in the island (or gives up just before this timeout, in seconds). In the
/// main session the terminal shows its own prompt meanwhile; first answer wins.
const WAITS: &str = "PermissionRequest";
const WAIT_TIMEOUT: u64 = 600;

/// The handler for one event.
fn entry(event: &str, cmd: &str) -> Value {
    if event == WAITS {
        json!({ "type": "command", "command": cmd, "timeout": WAIT_TIMEOUT })
    } else {
        // async: runs in the background, so Claude never waits on the island.
        json!({ "type": "command", "command": cmd, "async": true, "timeout": 5 })
    }
}

fn settings_path() -> Result<PathBuf, String> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .ok_or("no se encontró la carpeta del usuario")?;
    Ok(PathBuf::from(home).join(".claude").join("settings.json"))
}

/// Prefer the tiny `island-hook` next to the app (starts in a few ms);
/// fall back to the app binary's `hook` mode.
fn command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut fast = exe.with_file_name(if cfg!(windows) { "island-hook.exe" } else { "island-hook" });
    // An AppImage runs from a new mount point every time: hooks get a copy that stays put.
    if std::env::var_os("APPIMAGE").is_some() && fast.exists() {
        if let Some(copy) = stable_copy(&fast) {
            fast = copy;
        }
    }
    // Forward slashes: Claude Code runs hooks through a POSIX shell on Windows too.
    let quote = |p: &std::path::Path| format!("\"{}\"", p.to_string_lossy().replace('\\', "/"));
    Ok(if fast.exists() { quote(&fast) } else { format!("{} hook", quote(&exe)) })
}

/// `island-hook` copied to ~/.local/share/dev.dynamicisland.island/ (only
/// rewritten when it changed, through a rename so a running hook is unharmed).
fn stable_copy(src: &std::path::Path) -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dev.dynamicisland.island");
    let dest = dir.join("island-hook");
    let bytes = std::fs::read(src).ok()?;
    if std::fs::read(&dest).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::create_dir_all(&dir).ok()?;
        let tmp = dir.join("island-hook.new");
        std::fs::write(&tmp, &bytes).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).ok()?;
        }
        std::fs::rename(&tmp, &dest).ok()?;
    }
    Some(dest)
}

fn is_ours(hook: &Value) -> bool {
    hook["command"].as_str().is_some_and(|c| {
        // `ibrae hook` / the pre-rename `dynamic-island hook` fallback.
        c.contains("island-hook") || ((c.contains("ibrae") || c.contains("dynamic-island")) && c.trim_end().ends_with(" hook"))
    })
}

fn load() -> Result<(PathBuf, Value), String> {
    let path = settings_path()?;
    let value = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("settings.json no es JSON válido: {e}"))?,
        Err(_) => json!({}),
    };
    Ok((path, value))
}

fn save(path: &PathBuf, value: &Value) -> Result<(), String> {
    if path.exists() {
        fs::copy(path, path.with_extension("json.bak-dynamic-island")).map_err(|e| e.to_string())?;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, text + "\n").map_err(|e| e.to_string())
}

/// Removes our entries; when `add` is set, appends a fresh one per event.
fn rewrite(add: bool) -> Result<(), String> {
    let (path, mut settings) = load()?;
    let cmd = command()?;
    if !settings.is_object() {
        return Err("settings.json no es un objeto JSON".into());
    }
    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let Some(hooks) = hooks.as_object_mut() else {
        return Err("\"hooks\" en settings.json no es un objeto".into());
    };
    for (event, matcher) in EVENTS {
        let mut groups: Vec<Value> = hooks.get(event).and_then(Value::as_array).cloned().unwrap_or_default();
        for g in groups.iter_mut() {
            if let Some(list) = g.get_mut("hooks").and_then(Value::as_array_mut) {
                list.retain(|h| !is_ours(h));
            }
        }
        groups.retain(|g| g.get("hooks").and_then(Value::as_array).is_none_or(|l| !l.is_empty()));
        if add {
            let mut group = json!({ "hooks": [entry(event, &cmd)] });
            if matcher {
                group["matcher"] = json!("*");
            }
            groups.push(group);
        }
        if groups.is_empty() {
            hooks.remove(event);
        } else {
            hooks.insert(event.into(), Value::Array(groups));
        }
    }
    save(&path, &settings)
}

pub fn install() -> Result<(), String> {
    rewrite(true)
}

pub fn uninstall() -> Result<(), String> {
    rewrite(false)
}

/// True when the island is connected (one of our hooks exists somewhere) but some
/// event in `EVENTS` has none of ours, e.g. a set installed before SubagentStart existed.
fn missing_events(settings: &Value) -> bool {
    let Some(hooks) = settings["hooks"].as_object() else { return false };
    let ours_in = |groups: &Value| {
        groups
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|g| g["hooks"].as_array().into_iter().flatten())
            .any(is_ours)
    };
    let connected = hooks.values().any(ours_in);
    connected && EVENTS.iter().any(|&(event, _)| !hooks.get(event).is_some_and(ours_in))
}

/// At startup: if the user connected Claude Code earlier but the hook now points
/// at a file that no longer exists (moved, reinstalled, dev build deleted), predates
/// async hooks, or lacks events this build uses (e.g. SubagentStart/SubagentStop),
/// rewrite it for this copy. Any island listens on the same channel, so a hook
/// pointing at another existing copy is left alone.
pub fn repair() {
    let Ok((_, settings)) = load() else { return };
    // AppImage: refresh the stable copy of island-hook to this version's.
    if std::env::var_os("APPIMAGE").is_some() && installed().unwrap_or(false) {
        let _ = command();
    }
    if needs_repair(&settings, |p| std::path::Path::new(p).exists()) {
        let _ = install();
    }
}

fn needs_repair(settings: &Value, exists: impl Fn(&str) -> bool) -> bool {
    let hooks: Vec<(&String, &Value)> = settings["hooks"]
        .as_object()
        .into_iter()
        .flatten()
        .flat_map(|(event, groups)| groups.as_array().into_iter().flatten().map(move |g| (event, g)))
        .flat_map(|(event, g)| g["hooks"].as_array().into_iter().flatten().map(move |h| (event, h)))
        .filter(|(_, h)| is_ours(h))
        .collect();
    let stale = hooks.iter().any(|(event, h)| {
        let cmd = h["command"].as_str().unwrap_or("");
        let path = cmd.split('"').nth(1).unwrap_or("");
        let should_wait = event.as_str() == WAITS;
        (h["async"] == Value::Bool(true)) == should_wait || path.is_empty() || !exists(path)
    });
    !hooks.is_empty() && (stale || missing_events(settings))
}

pub fn installed() -> Result<bool, String> {
    let (_, settings) = load()?;
    Ok(settings["hooks"]
        .as_object()
        .is_some_and(|h| h.values().flat_map(|g| g.as_array().into_iter().flatten()).any(|g| {
            g["hooks"].as_array().is_some_and(|l| l.iter().any(is_ours))
        })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ours() -> Value {
        json!({ "type": "command", "command": "\"/opt/island-hook\"", "async": true, "timeout": 5 })
    }

    #[test]
    fn missing_events_detects_new_events() {
        let settings = json!({ "hooks": { "PreToolUse": [{ "matcher": "*", "hooks": [ours()] }] } });
        assert!(missing_events(&settings));
    }

    #[test]
    fn missing_events_false_when_complete() {
        let mut hooks = serde_json::Map::new();
        for (event, _) in EVENTS {
            hooks.insert(event.into(), json!([{ "hooks": [ours()] }]));
        }
        assert!(!missing_events(&json!({ "hooks": hooks })));
    }

    #[test]
    fn repair_wants_the_permission_hook_synchronous() {
        let mut hooks = serde_json::Map::new();
        for (event, _) in EVENTS {
            hooks.insert(event.into(), json!([{ "hooks": [entry(event, "\"/opt/island-hook\"")] }]));
        }
        let ok = json!({ "hooks": hooks.clone() });
        assert!(!needs_repair(&ok, |_| true));
        assert!(needs_repair(&ok, |_| false)); // moved
        hooks.insert(WAITS.into(), json!([{ "hooks": [ours()] }])); // async: can't answer
        assert!(needs_repair(&json!({ "hooks": hooks }), |_| true));
    }

    #[test]
    fn missing_events_false_when_not_connected() {
        assert!(!missing_events(&json!({})));
        let foreign = json!({ "hooks": { "PreToolUse": [{ "hooks": [{ "type": "command", "command": "echo hi" }] }] } });
        assert!(!missing_events(&foreign));
    }
}
