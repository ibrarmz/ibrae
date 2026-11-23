//! Updates. Reads `latest.json` from the newest GitHub release at startup and
//! every 12 h (if "Buscar actualizaciones" is on) and tells the UI when there
//! is a newer version.
//!
//! Installing is only automatic where it can be done safely: the Windows
//! installer (.exe, NSIS), the MSI and the Linux AppImage, through
//! tauri-plugin-updater, which checks the download against the public key in
//! tauri.conf.json before using it (the AppImage file is replaced in place and
//! the app restarts). The portable .zip and the .deb / .rpm packages just get a
//! link to the release (their package manager owns the files).
//!
//! `ISLAND_UPDATE_URL` points both the check and the install at another
//! latest.json (to test an update end to end); signatures are still checked.
//! `ISLAND_UPDATE_NOW=1` checks and installs right away at startup (same test).

use serde::{Deserialize, Serialize};
use std::{sync::Mutex, thread, time::Duration};
use tauri::{AppHandle, Emitter, Manager, State};

const LATEST_JSON: &str = "https://github.com/ibrarmz/ibrae/releases/latest/download/latest.json";
pub const RELEASES: &str = "https://github.com/ibrarmz/ibrae/releases/latest";
const EVERY: Duration = Duration::from_secs(12 * 3600);

#[derive(Serialize, Clone, Default, PartialEq)]
pub struct UpdateStatus {
    pub current: String,
    /// Newer version available, if any.
    pub latest: Option<String>,
    /// Can install by itself (Windows installer / MSI); otherwise "Descargar".
    pub can_install: bool,
    pub installing: bool,
    /// Spanish, ready to show.
    pub error: Option<String>,
    /// Epoch ms of the last successful check (0 = never).
    pub checked_at: i64,
}

#[derive(Default)]
pub struct UpdateState {
    status: Mutex<UpdateStatus>,
}

#[derive(Deserialize)]
struct Latest {
    version: String,
}

pub fn start(app: AppHandle) {
    {
        let state = app.state::<UpdateState>();
        let mut s = state.status.lock().unwrap();
        s.current = app.package_info().version.to_string();
        s.can_install = can_install();
    }
    thread::spawn(move || {
        if std::env::var_os("ISLAND_UPDATE_NOW").is_some() {
            std::env::remove_var("ISLAND_UPDATE_NOW");
            if check(&app).latest.is_some() {
                let result = tauri::async_runtime::block_on(update_install(app.clone()));
                eprintln!("update: {result:?}");
            }
        }
        // Let the island settle first; nobody needs this in the first seconds.
        thread::sleep(Duration::from_secs(30));
        loop {
            if crate::settings::get(&app).check_updates {
                check(&app);
            }
            thread::sleep(EVERY);
        }
    });
}

/// "0.1.12" → (0, 1, 12). Anything after a '-' (pre-release) is ignored.
fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let v = v.split(['-', '+']).next()?;
    let mut it = v.split('.').map(|n| n.parse::<u64>().ok());
    Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
}

fn newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(l), Some(c)) if l > c)
}

/// The newest published version, or None when that release has no latest.json
/// (versions built without the signing key).
fn latest_url() -> String {
    std::env::var("ISLAND_UPDATE_URL").unwrap_or_else(|_| LATEST_JSON.to_string())
}

fn fetch_latest() -> Result<Option<String>, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .build()
        .new_agent();
    let mut res = agent.get(latest_url()).call().map_err(|_| "Sin conexión con GitHub".to_string())?;
    match res.status().as_u16() {
        200..=299 => {}
        404 => return Ok(None),
        code => return Err(format!("GitHub respondió {code}")),
    }
    let body = res.body_mut().read_to_string().map_err(|_| "La respuesta se cortó".to_string())?;
    serde_json::from_str::<Latest>(&body).map(|l| Some(l.version)).map_err(|_| "Respuesta inesperada".to_string())
}

fn check(app: &AppHandle) -> UpdateStatus {
    let result = fetch_latest();
    let state = app.state::<UpdateState>();
    let snap = {
        let mut s = state.status.lock().unwrap();
        match result {
            Ok(v) => {
                s.latest = v.filter(|v| newer(v, &s.current));
                s.error = None;
                s.checked_at = chrono::Utc::now().timestamp_millis();
            }
            Err(e) => s.error = Some(e),
        }
        s.clone()
    };
    let _ = app.emit("update", snap.clone());
    snap
}

/// Installed with the .exe (its uninstaller sits next to us) or the MSI, or
/// running as an AppImage (the AppImage runtime sets `APPIMAGE` to its file).
fn can_install() -> bool {
    #[cfg(windows)]
    {
        use tauri::utils::{config::BundleType, platform::bundle_type};
        let exe = std::env::current_exe().ok();
        let nsis = exe.as_ref().and_then(|e| e.parent()).is_some_and(|d| d.join("uninstall.exe").exists());
        // The build marks the plain exe as "msi" too (the last bundle made), and
        // that exe also goes into the portable .zip: only trust it in Program Files.
        let in_program_files = exe.as_ref().is_some_and(|e| {
            ["ProgramW6432", "ProgramFiles"]
                .iter()
                .filter_map(std::env::var_os)
                .any(|pf| e.starts_with(std::path::PathBuf::from(pf)))
        });
        nsis || (bundle_type() == Some(BundleType::Msi) && in_program_files)
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("APPIMAGE").is_some_and(|p| std::path::Path::new(&p).is_file())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    false
}

#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
fn set(app: &AppHandle, f: impl FnOnce(&mut UpdateStatus)) {
    let state = app.state::<UpdateState>();
    let snap = {
        let mut s = state.status.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    let _ = app.emit("update", snap);
}

#[tauri::command]
pub fn update_status(state: State<UpdateState>) -> UpdateStatus {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<UpdateStatus, String> {
    tauri::async_runtime::spawn_blocking(move || check(&app)).await.map_err(|e| e.to_string())
}

/// Download, verify and install. Windows: the app closes and the installer
/// reopens it. AppImage: the file is replaced and we restart into it.
/// Where that isn't possible, open the release page instead.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    if !can_install() {
        return crate::calendar::open_url(RELEASES.into());
    }
    #[cfg(any(windows, target_os = "linux"))]
    {
        use tauri_plugin_updater::UpdaterExt;
        set(&app, |s| {
            s.installing = true;
            s.error = None;
        });
        let result = async {
            let mut builder = app.updater_builder();
            if let Ok(url) = std::env::var("ISLAND_UPDATE_URL") {
                let url = url.parse().map_err(|_| "ISLAND_UPDATE_URL no es una URL".to_string())?;
                builder = builder.endpoints(vec![url]).map_err(|e| e.to_string())?;
            }
            let update = builder
                .build()
                .map_err(|e| e.to_string())?
                .check()
                .await
                .map_err(|_| "No se pudo consultar la actualización".to_string())?
                .ok_or_else(|| "Ya tienes la última versión".to_string())?;
            update
                .download_and_install(|_, _| {}, || {})
                .await
                .map_err(|_| "No se pudo descargar o verificar la actualización".to_string())
        }
        .await;
        // Windows never gets here after a successful install (the installer closes us).
        #[cfg(target_os = "linux")]
        if result.is_ok() {
            // The new instance waits for this one to let go of the Claude channel.
            std::env::set_var("ISLAND_RESTART", "1");
            app.restart();
        }
        if let Err(e) = &result {
            let e = e.clone();
            set(&app, |s| {
                s.installing = false;
                s.error = Some(e);
            });
        }
        return result;
    }
    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer("0.1.13", "0.1.12"));
        assert!(newer("0.2.0", "0.1.99"));
        assert!(newer("v1.0.0", "0.9.9"));
        assert!(!newer("0.1.12", "0.1.12"));
        assert!(!newer("0.1.9", "0.1.12"));
        assert!(!newer("basura", "0.1.12"));
    }
}
