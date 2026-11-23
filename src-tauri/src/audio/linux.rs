//! Linux: volume, mute and outputs through `pactl` (PipeWire's PulseAudio
//! server or PulseAudio itself), brightness through /sys/class/backlight.
//!
//! `pactl subscribe` reports every change to outputs and the default output,
//! so audio is event driven. Without `pactl` we fall back to WirePlumber's
//! `wpctl`, which has no events: then the volume is read once a second and
//! there is no output list. Sysfs doesn't notify brightness changes, so the
//! backlight file (a few bytes) is read every 300 ms. Brightness is set through
//! logind (`SetBrightness`, no root needed), else by writing the file.

use super::{parse, update, AudioState, Output};
use std::{
    fs,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::Duration,
};
use tauri::{AppHandle, Manager};

pub enum Cmd {
    Volume(u32),
    Mute(bool),
    Brightness(u32),
    Output(String),
    /// Something changed in the audio server: read it again.
    Refresh,
}

static TX: OnceLock<Mutex<mpsc::Sender<Cmd>>> = OnceLock::new();

pub fn send(cmd: Cmd) -> Result<(), String> {
    let tx = TX.get().ok_or("El audio todavía no está listo")?;
    tx.lock().unwrap().send(cmd).map_err(|_| "El audio no responde".to_string())
}

pub fn start(app: AppHandle) {
    let (tx, rx) = mpsc::channel();
    let _ = TX.set(Mutex::new(tx.clone()));
    let tool = if exec("pactl", &["info"]).is_some() {
        Tool::Pactl
    } else if exec("wpctl", &["status"]).is_some() {
        Tool::Wpctl
    } else {
        Tool::None
    };
    match tool {
        Tool::Pactl => spawn_subscribe(tx.clone()),
        Tool::Wpctl => {
            let tx = tx.clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_secs(1));
                if tx.send(Cmd::Refresh).is_err() {
                    return;
                }
            });
        }
        Tool::None => {}
    }
    if let Some(panel) = Backlight::find() {
        watch_brightness(app.clone(), panel);
    }
    thread::spawn(move || run(app, tool, rx));
}

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Pactl,
    Wpctl,
    None,
}

/// Run a command and return its stdout when it succeeds.
fn exec(prog: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(prog);
    cmd.args(args).env("LC_ALL", "C").stdin(Stdio::null()).stderr(Stdio::null());
    // Inside an AppImage, don't hand the bundle's libraries to system tools.
    if std::env::var_os("APPDIR").is_some() {
        cmd.env_remove("LD_LIBRARY_PATH");
    }
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `pactl subscribe` prints a line per change; we only care about outputs
/// ("sink") and the server (default output). Restarts if the server restarts.
fn spawn_subscribe(tx: mpsc::Sender<Cmd>) {
    thread::spawn(move || loop {
        let mut cmd = Command::new("pactl");
        cmd.arg("subscribe").env("LC_ALL", "C").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        if std::env::var_os("APPDIR").is_some() {
            cmd.env_remove("LD_LIBRARY_PATH");
        }
        if let Ok(mut child) = cmd.spawn() {
            if let Some(out) = child.stdout.take() {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if (line.contains(" on sink ") || line.contains(" on server ")) && tx.send(Cmd::Refresh).is_err() {
                        let _ = child.kill();
                        return;
                    }
                }
            }
            let _ = child.wait();
        }
        // The audio server went away (restart, logout): read again and resubscribe.
        if tx.send(Cmd::Refresh).is_err() {
            return;
        }
        thread::sleep(Duration::from_secs(3));
    });
}

struct Snapshot {
    volume: u32,
    muted: bool,
    output: String,
    outputs: Vec<Output>,
}

fn read(tool: Tool) -> Option<Snapshot> {
    match tool {
        Tool::Pactl => {
            let default = exec("pactl", &["get-default-sink"])?.trim().to_string();
            let volume = parse::volume(&exec("pactl", &["get-sink-volume", "@DEFAULT_SINK@"])?)?;
            let muted = parse::mute(&exec("pactl", &["get-sink-mute", "@DEFAULT_SINK@"])?).unwrap_or(false);
            let sinks = parse::sinks(&exec("pactl", &["list", "sinks"]).unwrap_or_default());
            let output = sinks.iter().find(|(n, _)| *n == default).map_or_else(|| default.clone(), |(_, d)| d.clone());
            let outputs = sinks
                .into_iter()
                .map(|(id, name)| Output { default: id == default, id, name })
                .collect();
            Some(Snapshot { volume, muted, output, outputs })
        }
        Tool::Wpctl => {
            let (volume, muted) = parse::wpctl_volume(&exec("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])?)?;
            let output = exec("wpctl", &["inspect", "@DEFAULT_AUDIO_SINK@"])
                .and_then(|o| parse::wpctl_description(&o))
                .unwrap_or_default();
            Some(Snapshot { volume, muted, output, outputs: Vec::new() })
        }
        Tool::None => None,
    }
}

/// Read the audio server and tell the UI what changed (no notice the first time).
fn refresh(app: &AppHandle, tool: Tool, first: bool) {
    let Some(new) = read(tool) else {
        update(app, None, |s| s.available = false);
        return;
    };
    let cause = {
        let s = app.state::<AudioState>().status.lock().unwrap().clone();
        if first || !s.available {
            None
        } else if s.output != new.output {
            Some("output")
        } else if s.muted != new.muted {
            Some("mute")
        } else if s.volume != new.volume {
            Some("volume")
        } else {
            None
        }
    };
    update(app, cause, |s| {
        s.available = true;
        s.volume = new.volume;
        s.muted = new.muted;
        s.output = new.output;
        s.outputs = new.outputs;
    });
}

fn run(app: AppHandle, tool: Tool, rx: mpsc::Receiver<Cmd>) {
    refresh(&app, tool, true);
    let panel = Backlight::find();
    while let Ok(first) = rx.recv() {
        // Let a burst of events (or a slider drag) arrive, then act on the last values.
        thread::sleep(Duration::from_millis(40));
        let mut cmds = vec![first];
        cmds.extend(rx.try_iter());
        let (mut volume, mut brightness, mut stale) = (None, None, false);
        for cmd in cmds {
            match cmd {
                Cmd::Volume(v) => volume = Some(v),
                Cmd::Brightness(v) => brightness = Some(v),
                Cmd::Mute(m) => set_mute(tool, m),
                Cmd::Output(id) => {
                    // Only ids we listed, so nothing odd reaches pactl.
                    let known = app.state::<AudioState>().status.lock().unwrap().outputs.iter().any(|o| o.id == id);
                    if known && tool == Tool::Pactl {
                        let _ = exec("pactl", &["set-default-sink", &id]);
                    }
                }
                Cmd::Refresh => stale = true,
            }
        }
        if let Some(v) = volume {
            set_volume(tool, v);
        }
        if let (Some(v), Some(p)) = (brightness, &panel) {
            p.set(v);
        }
        // pactl reports our own changes through subscribe; wpctl doesn't.
        if stale || tool == Tool::Wpctl {
            refresh(&app, tool, false);
        }
    }
}

fn set_volume(tool: Tool, v: u32) {
    let pct = format!("{}%", v.min(100));
    match tool {
        Tool::Pactl => {
            let _ = exec("pactl", &["set-sink-volume", "@DEFAULT_SINK@", &pct]);
        }
        Tool::Wpctl => {
            let _ = exec("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &pct]);
        }
        Tool::None => {}
    }
}

fn set_mute(tool: Tool, m: bool) {
    let v = if m { "1" } else { "0" };
    match tool {
        Tool::Pactl => {
            let _ = exec("pactl", &["set-sink-mute", "@DEFAULT_SINK@", v]);
        }
        Tool::Wpctl => {
            let _ = exec("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", v]);
        }
        Tool::None => {}
    }
}

// ---- Brightness ---------------------------------------------------------

struct Backlight {
    name: String,
    max: u32,
}

impl Backlight {
    const DIR: &'static str = "/sys/class/backlight";

    /// The laptop panel, if there is one (desktops and external monitors: none).
    fn find() -> Option<Backlight> {
        let devices: Vec<(String, String)> = fs::read_dir(Self::DIR)
            .ok()?
            .flatten()
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let kind = fs::read_to_string(e.path().join("type")).unwrap_or_default();
                (name, kind)
            })
            .collect();
        let name = parse::pick_backlight(&devices)?;
        let max = Self::read_u32(&name, "max_brightness").filter(|m| *m > 0)?;
        Some(Backlight { name, max })
    }

    fn read_u32(name: &str, file: &str) -> Option<u32> {
        fs::read_to_string(format!("{}/{name}/{file}", Self::DIR)).ok()?.trim().parse().ok()
    }

    fn get(&self) -> Option<u32> {
        Self::read_u32(&self.name, "brightness").map(|raw| parse::percent(raw, self.max))
    }

    fn set(&self, percent: u32) {
        let raw = parse::raw(percent, self.max);
        let via_logind = zbus::blocking::Connection::system().and_then(|conn| {
            let p = zbus::blocking::Proxy::new(
                &conn,
                "org.freedesktop.login1",
                "/org/freedesktop/login1/session/auto",
                "org.freedesktop.login1.Session",
            )?;
            p.call_method("SetBrightness", &("backlight", self.name.as_str(), raw)).map(|_| ())
        });
        if via_logind.is_err() {
            let _ = fs::write(format!("{}/{}/brightness", Self::DIR, self.name), raw.to_string());
        }
    }
}

fn watch_brightness(app: AppHandle, panel: Backlight) {
    thread::spawn(move || {
        let mut last = panel.get();
        update(&app, None, |s| s.brightness = last);
        loop {
            thread::sleep(Duration::from_millis(300));
            let now = panel.get();
            if now != last && now.is_some() {
                last = now;
                update(&app, Some("brightness"), |s| s.brightness = now);
            }
        }
    });
}
