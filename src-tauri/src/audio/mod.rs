//! Volume, mute, audio output and screen brightness: what the pill shows when
//! they change (the "OSD") and the controls in Sistema → Sonido y pantalla.
//!
//! Event-driven like the rest of the core: Windows calls us when the volume,
//! the default output or the brightness change; on Linux `pactl subscribe`
//! does (see `linux.rs` for the exceptions). Each change is emitted as `audio`
//! with its `cause` ("volume" | "mute" | "output" | "brightness"), and the UI
//! decides whether to show it. Other platforms report `available: false`.

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "linux", test))]
mod parse;
#[cfg(windows)]
mod win;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Output {
    pub id: String,
    pub name: String,
    pub default: bool,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct AudioStatus {
    /// There is a default output whose volume we can read.
    pub available: bool,
    /// 0–100.
    pub volume: u32,
    pub muted: bool,
    /// Name of the default output.
    pub output: String,
    pub outputs: Vec<Output>,
    /// 0–100, only for a screen whose brightness the PC controls (laptop panel).
    pub brightness: Option<u32>,
}

#[derive(Serialize, Clone)]
struct AudioEvent {
    status: AudioStatus,
    /// What changed, for the pill; None = no notice (startup, device list).
    cause: Option<&'static str>,
}

#[derive(Default)]
pub struct AudioState {
    status: Mutex<AudioStatus>,
}

/// Apply a change and tell the UI, only if something really changed.
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
fn update(app: &AppHandle, cause: Option<&'static str>, f: impl FnOnce(&mut AudioStatus)) {
    let state = app.state::<AudioState>();
    let snap = {
        let mut s = state.status.lock().unwrap();
        let before = s.clone();
        f(&mut s);
        if *s == before {
            return;
        }
        s.clone()
    };
    let _ = app.emit("audio", AudioEvent { status: snap, cause });
}

pub fn start(app: AppHandle) {
    #[cfg(windows)]
    win::start(app);
    #[cfg(target_os = "linux")]
    linux::start(app);
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = app;
}

#[tauri::command]
pub fn audio_snapshot(state: State<AudioState>) -> AudioStatus {
    state.status.lock().unwrap().clone()
}

/// What: "volume" (0–100) | "mute" (0/1) | "brightness" (0–100). Output: `audio_output`.
#[tauri::command]
pub fn audio_set(what: String, value: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let cmd = match what.as_str() {
            "volume" => win::Cmd::Volume(value.min(100)),
            "mute" => win::Cmd::Mute(value != 0),
            "brightness" => win::Cmd::Brightness(value.min(100)),
            _ => return Err("Ajuste desconocido".into()),
        };
        win::send(cmd)
    }
    #[cfg(target_os = "linux")]
    {
        let cmd = match what.as_str() {
            "volume" => linux::Cmd::Volume(value.min(100)),
            "mute" => linux::Cmd::Mute(value != 0),
            "brightness" => linux::Cmd::Brightness(value.min(100)),
            _ => return Err("Ajuste desconocido".into()),
        };
        linux::send(cmd)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (what, value);
        Err("No disponible en este sistema".into())
    }
}

/// Make `id` (from `outputs`) the default output.
#[tauri::command]
pub fn audio_output(id: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        win::send(win::Cmd::Output(id))
    }
    #[cfg(target_os = "linux")]
    {
        linux::send(linux::Cmd::Output(id))
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = id;
        Err("No disponible en este sistema".into())
    }
}
