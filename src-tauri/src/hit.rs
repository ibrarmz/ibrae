//! Window placement, click-through and "stay out of the way" behaviour.
//!
//! The window is a fixed transparent rectangle at the top of the screen; only
//! the island inside it may catch the mouse, and even then only on purpose:
//!
//! - away   → cursor elsewhere. Click-through on.
//! - ghost  → cursor passed over the island. The UI fades it out and clicks
//!            still go to whatever is underneath (e.g. browser tabs).
//! - active → the cursor stayed still on it for DWELL, or Ctrl is held, or the
//!            island was already open. Click-through off, the UI expands.
//!
//! A click while in ghost means the user was aiming at something underneath,
//! so the island won't auto-open until the cursor leaves (Ctrl still works).
//!
//! It also reports `covered` when the foreground window on this monitor is
//! maximized or fullscreen, so the UI can tuck the island away.
//!
//! Which monitor: `settings.monitor` (see monitors.rs). The same loops notice
//! displays being plugged / unplugged, and in "cursor" mode move the island to
//! the monitor the cursor has rested on for a second.
//!
//! The loop stays cheap: direct syscalls against a cached window origin, and
//! it only round-trips to the UI thread when a state actually changes.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use crate::monitors::{self, Mon};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Must match WIN_W in src/App.svelte.
pub const WIN_W: f64 = 760.0;
/// Window height in logical px (tauri.conf.json).
const WIN_H: f64 = 330.0;
/// Gap between the screen edge and the island when it sits left or right.
const SIDE_MARGIN: i32 = 12;

#[derive(Default)]
struct Hit {
    /// Island hit rectangle in logical px relative to the window: x, y, w, h.
    rect: (f64, f64, f64, f64),
    /// The island is expanded: entering it is always intentional.
    open: bool,
    /// Something is active (music, focus): use the longer dwell.
    busy: bool,
}

#[derive(Default)]
pub struct HitState {
    hit: Mutex<Hit>,
    /// Window origin in physical px and its scale factor.
    origin: Mutex<(f64, f64, f64)>,
    /// Displays as of the last check, the one the island is on, the
    /// `settings.monitor` value it was placed for, and in "cursor" mode the
    /// monitor being followed.
    mons: Mutex<Vec<Mon>>,
    current: Mutex<String>,
    mode: Mutex<String>,
    following: Mutex<Option<String>>,
}

/// How long the cursor must stay on another monitor before the island follows it.
const FOLLOW_AFTER: Duration = Duration::from_secs(1);

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn set_hit_rect(
    window: WebviewWindow,
    state: tauri::State<HitState>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    open: bool,
    busy: bool,
) {
    *state.hit.lock().unwrap() = Hit { rect: (x, y, w, h), open, busy };
    // Linux: the island's rectangle *is* the window's input region; the rest lets clicks through.
    #[cfg(target_os = "linux")]
    platform::set_input_region(&window, (x, y, w, h));
    #[cfg(not(target_os = "linux"))]
    let _ = window;
}

/// Last `covered` value, for a UI that starts listening after it was sent.
static COVERED: AtomicBool = AtomicBool::new(false);

pub fn set_covered(win: &WebviewWindow, covered: bool) {
    COVERED.store(covered, Ordering::Relaxed);
    let _ = win.emit("covered", covered);
}

#[tauri::command]
pub fn covered() -> bool {
    COVERED.load(Ordering::Relaxed)
}

/// Called when the island collapses after the user clicked or typed in it:
/// give the keyboard back to the app they were using.
#[tauri::command]
pub fn release_focus() {
    platform::restore_foreground();
}

/// true: the core tracks the cursor and emits `hover` (Windows).
/// false: the UI must detect hover itself with DOM events (Linux, where
/// Wayland/XWayland don't expose the global cursor reliably).
#[tauri::command]
pub fn hover_native() -> bool {
    cfg!(windows)
}

/// Place the window at the top of the chosen monitor (left, centre or right,
/// per settings) and cache its origin. The UI aligns the island inside it to match.
pub fn place_window(win: &WebviewWindow) {
    let mons = monitors::snapshot(win);
    let state = win.state::<HitState>();
    let settings = crate::settings::get(win.app_handle());
    let following = state.following.lock().unwrap().clone();
    let Some(m) = monitors::choose(&mons, &settings.monitor, following.as_deref()).cloned() else { return };
    let sf = m.scale;
    let width = (WIN_W * sf).round() as i32;
    let margin = (SIDE_MARGIN as f64 * sf).round() as i32;
    let (mx, my, mw, _) = m.rect;
    let x = match settings.position.as_str() {
        "left" => mx + margin,
        "right" => mx + mw - width - margin,
        _ => mx + (mw - width) / 2,
    };
    let _ = win.set_position(PhysicalPosition::new(x, my));
    // Size it for *this* monitor's scale: the window is created at the scale of
    // whichever monitor Windows picked first, and moving it to one with another
    // DPI doesn't always rescale it, which left the island cut off on the right.
    let _ = win.set_size(PhysicalSize::new(width as u32, (WIN_H * sf).round() as u32));
    *state.origin.lock().unwrap() = (x as f64, my as f64, sf);
    *state.current.lock().unwrap() = m.key;
    *state.mode.lock().unwrap() = settings.monitor;
    *state.mons.lock().unwrap() = mons;
}

/// Run every few hundred ms by the cursor / covered loops. Every ~2 s: re-place
/// if displays changed. In "cursor" mode: follow the cursor to another monitor
/// once it has stayed there FOLLOW_AFTER, but never while the island is open
/// or the cursor is on it.
fn monitor_tick(win: &WebviewWindow, tick: u32, every_2s: u32, cursor: Option<(f64, f64)>, pending: &mut Option<(String, Instant)>) {
    let state = win.state::<HitState>();
    if tick.is_multiple_of(every_2s) && (monitors::snapshot(win) != *state.mons.lock().unwrap() || moved_by_os(win)) {
        place_window(win);
        return;
    }
    if *state.mode.lock().unwrap() != monitors::CURSOR || state.hit.lock().unwrap().open {
        *pending = None;
        return;
    }
    let Some(p) = cursor else { return };
    let under = state.mons.lock().unwrap().iter().find(|m| m.contains(p)).map(|m| m.key.clone());
    let current = state.current.lock().unwrap().clone();
    match under {
        Some(key) if key != current => match pending {
            Some((k, since)) if *k == key => {
                if since.elapsed() >= FOLLOW_AFTER {
                    *state.following.lock().unwrap() = Some(key);
                    *pending = None;
                    place_window(win);
                }
            }
            _ => *pending = Some((key, Instant::now())),
        },
        _ => *pending = None,
    }
}

/// Windows 11 puts windows back on a monitor when it is plugged in again
/// ("remember window locations"), a few seconds after we placed the island.
/// Hover is tested against the cached origin, so a window moved under us
/// opens on one monitor while it shows on the other: re-place it.
#[cfg(windows)]
fn moved_by_os(win: &WebviewWindow) -> bool {
    let (ox, oy, _) = *win.state::<HitState>().origin.lock().unwrap();
    win.outer_position()
        .map(|p| (p.x as f64 - ox).abs() > 2.0 || (p.y as f64 - oy).abs() > 2.0)
        .unwrap_or(false)
}

/// Window managers on Linux may keep the window off our exact position on
/// purpose (KWin pins it over a top panel), so this check is Windows-only.
#[cfg(not(windows))]
fn moved_by_os(_: &WebviewWindow) -> bool {
    false
}

#[derive(Clone, Copy, PartialEq)]
enum Hover {
    Away,
    Ghost { since: Instant, clicked: bool },
    Active,
}

impl Hover {
    fn name(self) -> &'static str {
        match self {
            Hover::Away => "away",
            Hover::Ghost { .. } => "ghost",
            Hover::Active => "active",
        }
    }
}

pub fn start(win: WebviewWindow) {
    // Re-place the window if displays change (resolution, scale, monitor plugged in).
    let w2 = win.clone();
    win.on_window_event(move |e| {
        if let tauri::WindowEvent::ScaleFactorChanged { .. } = e {
            place_window(&w2);
        }
    });

    if hover_native() {
        let _ = win.set_ignore_cursor_events(true);
        track_cursor(win);
        return;
    }
    #[cfg(target_os = "linux")]
    {
        platform::watch_leave(&win);
        // Plasma Wayland: KWin sees native Wayland windows, X11 doesn't.
        let kwin = crate::kwin::wanted() && crate::kwin::start(win.clone());
        watch_covered(win, !kwin);
        return;
    }
    #[allow(unreachable_code)]
    watch_covered(win, true);
}

/// Linux: the "maximized window in front" check (unless KWin does it) and the
/// monitor tick; hover lives in the UI.
fn watch_covered(win: WebviewWindow, check_covered: bool) {
    thread::spawn(move || {
        let mut covered = false;
        let mut pending = None;
        let mut tick: u32 = 0;
        loop {
            thread::sleep(Duration::from_millis(300));
            tick = tick.wrapping_add(1);
            // X11 sees the pointer only over X11 windows on a Wayland session.
            monitor_tick(&win, tick, 7, platform::cursor(&win), &mut pending);
            if !check_covered || !tick.is_multiple_of(2) {
                continue;
            }
            let (ox, oy, _) = *win.state::<HitState>().origin.lock().unwrap();
            let now = platform::foreground_covers((ox + 10.0, oy + 5.0));
            if now != covered {
                covered = now;
                set_covered(&win, covered);
            }
        }
    });
}

/// Windows: poll the cursor and run the away / ghost / active state machine.
fn track_cursor(win: WebviewWindow) {
    thread::spawn(move || {
        let mut hover = Hover::Away;
        let mut covered = false;
        let mut tick: u32 = 0;
        let mut pending = None;
        loop {
            thread::sleep(Duration::from_millis(match hover {
                Hover::Away => 70,
                _ => 30,
            }));
            tick = tick.wrapping_add(1);

            let state = win.state::<HitState>();
            let (ox, oy, sf) = *state.origin.lock().unwrap();

            // Every ~0.5 s: is a maximized / fullscreen window in front?
            // And are we still above normal windows? (Windows can demote it.)
            if tick % 7 == 0 {
                platform::keep_on_top(&win);
                let now_covered = platform::foreground_covers((ox + 10.0, oy + 5.0));
                if now_covered != covered {
                    covered = now_covered;
                    set_covered(&win, covered);
                }
            }

            let cursor = platform::cursor(&win);
            if hover == Hover::Away {
                // ~2 s at the idle 70 ms pace.
                monitor_tick(&win, tick, 30, cursor, &mut pending);
            }
            let Some((cx, cy)) = cursor else { continue };
            let (inside, open, busy) = {
                let hit = state.hit.lock().unwrap();
                let (x, y, w, h) = hit.rect;
                let lx = (cx - ox) / sf;
                let ly = (cy - oy) / sf;
                (w > 0.0 && lx >= x && lx <= x + w && ly >= y && ly <= y + h, hit.open, hit.busy)
            };
            // Read only while deciding whether a hover becomes an open.
            let dwell = || {
                let s = crate::settings::get(win.app_handle());
                Duration::from_millis(if busy { s.dwell_busy } else { s.dwell_idle } as u64)
            };

            let next = match (hover, inside) {
                (_, false) => Hover::Away,
                (Hover::Away, true) if open => Hover::Active,
                (Hover::Away, true) => Hover::Ghost { since: Instant::now(), clicked: platform::mouse_down() },
                (Hover::Ghost { since, clicked }, true) => {
                    let clicked = clicked || platform::mouse_down();
                    if open || platform::ctrl_down() || (!clicked && since.elapsed() >= dwell()) {
                        Hover::Active
                    } else {
                        Hover::Ghost { since, clicked }
                    }
                }
                (Hover::Active, true) => Hover::Active,
            };

            if next.name() != hover.name() {
                if next == Hover::Active {
                    platform::remember_foreground();
                }
                let _ = win.set_ignore_cursor_events(next != Hover::Active);
                let _ = win.emit("hover", next.name());
            }
            hover = next;
        }
    });
}

#[cfg(windows)]
mod platform {
    use tauri::WebviewWindow;
    use windows::Win32::{
        Foundation::{POINT, RECT},
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO,
            MONITOR_DEFAULTTONEAREST,
        },
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_LBUTTON, VK_RBUTTON},
            WindowsAndMessaging::{
                GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowRect,
                GetWindowThreadProcessId, IsZoomed,
            },
        },
    };

    pub fn cursor(_win: &WebviewWindow) -> Option<(f64, f64)> {
        let mut p = POINT::default();
        unsafe { GetCursorPos(&mut p).ok()? };
        Some((p.x as f64, p.y as f64))
    }

    /// The app that had the keyboard before the island opened.
    static PREVIOUS: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

    pub fn remember_foreground() {
        unsafe {
            let fg = GetForegroundWindow();
            let mut pid = 0u32;
            GetWindowThreadProcessId(fg, Some(&mut pid));
            if !fg.is_invalid() && pid != std::process::id() {
                PREVIOUS.store(fg.0 as isize, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    pub fn restore_foreground() {
        use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::{IsWindow, SetForegroundWindow}};
        let prev = HWND(PREVIOUS.load(std::sync::atomic::Ordering::Relaxed) as *mut _);
        unsafe {
            // Only while we are the foreground app (Windows allows it then); skip dead windows.
            let mut pid = 0u32;
            GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid));
            if pid == std::process::id() && !prev.is_invalid() && IsWindow(Some(prev)).as_bool() {
                let _ = SetForegroundWindow(prev);
            }
        }
    }

    fn key(vk: u16) -> bool {
        unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
    }

    pub fn mouse_down() -> bool {
        key(VK_LBUTTON.0) || key(VK_RBUTTON.0)
    }

    pub fn ctrl_down() -> bool {
        key(VK_CONTROL.0)
    }

    /// True when the foreground window sits on our monitor and is maximized
    /// or fullscreen. The desktop, the taskbar and our own window don't count.
    /// Windows sometimes drops an always-on-top window below normal windows
    /// while keeping its WS_EX_TOPMOST flag (seen behind a maximized Windows
    /// Terminal). Only topmost windows may sit above ours; if a visible normal
    /// window does, put the island back on top, without taking focus.
    pub fn keep_on_top(win: &WebviewWindow) {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindow, GetWindowLongW, IsWindowVisible, SetWindowPos, GWL_EXSTYLE, GW_HWNDPREV, HWND_TOPMOST,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, WS_EX_TOPMOST,
        };
        let Ok(hwnd) = win.hwnd() else { return };
        unsafe {
            let mut above = GetWindow(hwnd, GW_HWNDPREV);
            // Normally only a handful of topmost windows are above us.
            for _ in 0..64 {
                let Ok(h) = above else { return };
                if h.is_invalid() {
                    return;
                }
                let topmost = GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0 != 0;
                if !topmost && IsWindowVisible(h).as_bool() {
                    let _ = SetWindowPos(
                        hwnd,
                        Some(HWND_TOPMOST),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
                    );
                    return;
                }
                above = GetWindow(h, GW_HWNDPREV);
            }
        }
    }

    pub fn foreground_covers(island_point: (f64, f64)) -> bool {
        unsafe {
            let fg = GetForegroundWindow();
            if fg.is_invalid() {
                return false;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(fg, Some(&mut pid));
            if pid == std::process::id() {
                return false;
            }
            let mut class = [0u16; 64];
            let n = GetClassNameW(fg, &mut class) as usize;
            let class = String::from_utf16_lossy(&class[..n]);
            if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
                return false;
            }
            let ours = MonitorFromPoint(
                POINT { x: island_point.0 as i32, y: island_point.1 as i32 },
                MONITOR_DEFAULTTONEAREST,
            );
            let theirs = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
            if ours != theirs {
                return false;
            }
            if IsZoomed(fg).as_bool() {
                return true;
            }
            // Fullscreen (games, video, presentations): window covers the whole monitor.
            let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            let mut r = RECT::default();
            GetMonitorInfoW(theirs, &mut info).as_bool()
                && GetWindowRect(fg, &mut r).is_ok()
                && r.left <= info.rcMonitor.left
                && r.top <= info.rcMonitor.top
                && r.right >= info.rcMonitor.right
                && r.bottom >= info.rcMonitor.bottom
        }
    }
}

/// Linux. The island runs on X11 (native or XWayland, see main.rs), so EWMH
/// tells us about maximized / fullscreen windows. Caveat: on a Wayland session
/// only X11 apps are visible this way; native Wayland windows never "cover".
#[cfg(target_os = "linux")]
#[allow(dead_code)] // cursor/mouse/ctrl are only used by the Windows-style loop
mod platform {
    use std::sync::OnceLock;
    use tauri::WebviewWindow;
    use x11rb::{
        connection::Connection,
        protocol::xproto::{AtomEnum, ConnectionExt, KeyButMask, Window},
        rust_connection::RustConnection,
    };

    x11rb::atom_manager! {
        Atoms: AtomsCookie {
            _NET_ACTIVE_WINDOW,
            _NET_WM_STATE,
            _NET_WM_STATE_FULLSCREEN,
            _NET_WM_STATE_MAXIMIZED_VERT,
            _NET_WM_STATE_MAXIMIZED_HORZ,
            _NET_WM_PID,
        }
    }

    struct X {
        conn: RustConnection,
        root: Window,
        atoms: Atoms,
    }

    fn x() -> Option<&'static X> {
        static X11: OnceLock<Option<X>> = OnceLock::new();
        X11.get_or_init(|| {
            let (conn, screen) = RustConnection::connect(None).ok()?;
            let root = conn.setup().roots.get(screen)?.root;
            let atoms = Atoms::new(&conn).ok()?.reply().ok()?;
            Some(X { conn, root, atoms })
        })
        .as_ref()
    }

    /// The island's rectangle (logical px) becomes the only part of the window
    /// that receives input; everything else passes through to what's below.
    pub fn set_input_region(win: &WebviewWindow, (x, y, w, h): (f64, f64, f64, f64)) {
        let win2 = win.clone();
        let _ = win.run_on_main_thread(move || {
            use gtk::prelude::*;
            let Ok(gw) = win2.gtk_window() else { return };
            let Some(gdk) = gw.window() else { return };
            let rect = gtk::cairo::RectangleInt::new(x.floor() as i32, y.floor() as i32, w.ceil() as i32, h.ceil() as i32);
            gdk.input_shape_combine_region(&gtk::cairo::Region::create_rectangle(&rect), 0, 0);
        });
    }

    /// The UI can't tell on its own when the cursor leaves: WebKitGTK follows
    /// every leave with a synthetic `pointermove` at the exit point, which is
    /// still inside the island, so the page thinks the cursor came back. GTK
    /// sees the real crossings; forward them as `pointer-left` / `pointer-entered`
    /// so the UI can ignore pointer events while the cursor is outside.
    pub fn watch_leave(win: &WebviewWindow) {
        let w = win.clone();
        let _ = win.run_on_main_thread(move || {
            use gtk::{gdk, glib, prelude::*};
            let Ok(gw) = w.gtk_window() else { return };
            gw.add_events(gdk::EventMask::LEAVE_NOTIFY_MASK | gdk::EventMask::ENTER_NOTIFY_MASK);
            // Moving between our own widgets (the webview) isn't a crossing.
            let w2 = w.clone();
            gw.connect_enter_notify_event(move |_, ev| {
                if ev.detail() != gdk::NotifyType::Inferior {
                    let _ = tauri::Emitter::emit(&w2, "pointer-entered", ());
                }
                glib::Propagation::Proceed
            });
            gw.connect_leave_notify_event(move |_, ev| {
                if ev.detail() != gdk::NotifyType::Inferior {
                    let _ = tauri::Emitter::emit(&w, "pointer-left", ());
                }
                glib::Propagation::Proceed
            });
        });
    }

    fn pointer() -> Option<(f64, f64, u16)> {
        let x = x()?;
        let r = x.conn.query_pointer(x.root).ok()?.reply().ok()?;
        Some((r.root_x as f64, r.root_y as f64, u16::from(r.mask)))
    }

    pub fn cursor(_: &WebviewWindow) -> Option<(f64, f64)> {
        pointer().map(|p| (p.0, p.1))
    }

    // On X11 the window manager hands focus back when the island stops taking input.
    pub fn remember_foreground() {}
    pub fn restore_foreground() {}

    pub fn mouse_down() -> bool {
        let buttons = u16::from(KeyButMask::BUTTON1) | u16::from(KeyButMask::BUTTON3);
        pointer().is_some_and(|p| p.2 & buttons != 0)
    }

    pub fn ctrl_down() -> bool {
        pointer().is_some_and(|p| p.2 & u16::from(KeyButMask::CONTROL) != 0)
    }

    fn prop32(x: &X, win: Window, atom: u32, ty: AtomEnum) -> Vec<u32> {
        x.conn
            .get_property(false, win, atom, ty, 0, 64)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|v| v.collect()))
            .unwrap_or_default()
    }

    pub fn keep_on_top(_: &WebviewWindow) {}

    pub fn foreground_covers(island: (f64, f64)) -> bool {
        let Some(x) = x() else { return false };
        let Some(&active) = prop32(x, x.root, x.atoms._NET_ACTIVE_WINDOW, AtomEnum::WINDOW).first() else {
            return false;
        };
        if active == 0 || prop32(x, active, x.atoms._NET_WM_PID, AtomEnum::CARDINAL).first() == Some(&std::process::id()) {
            return false;
        }
        let state = prop32(x, active, x.atoms._NET_WM_STATE, AtomEnum::ATOM);
        let full = state.contains(&x.atoms._NET_WM_STATE_FULLSCREEN);
        let maxed = state.contains(&x.atoms._NET_WM_STATE_MAXIMIZED_VERT)
            && state.contains(&x.atoms._NET_WM_STATE_MAXIMIZED_HORZ);
        if !(full || maxed) {
            return false;
        }
        // Same monitor: the window spans the island's horizontal position.
        let geo = x.conn.get_geometry(active).ok().and_then(|c| c.reply().ok());
        let pos = x.conn.translate_coordinates(active, x.root, 0, 0).ok().and_then(|c| c.reply().ok());
        match (geo, pos) {
            (Some(g), Some(p)) => island.0 >= p.dst_x as f64 && island.0 <= p.dst_x as f64 + g.width as f64,
            _ => true,
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
#[allow(dead_code)]
mod platform {
    use tauri::WebviewWindow;
    pub fn cursor(win: &WebviewWindow) -> Option<(f64, f64)> {
        win.cursor_position().ok().map(|c| (c.x, c.y))
    }
    pub fn remember_foreground() {}
    pub fn restore_foreground() {}
    pub fn mouse_down() -> bool {
        false
    }
    pub fn ctrl_down() -> bool {
        false
    }
    pub fn foreground_covers(_: (f64, f64)) -> bool {
        false
    }
    pub fn keep_on_top(_: &WebviewWindow) {}
}
