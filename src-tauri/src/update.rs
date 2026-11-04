//! Updates of Cubby itself: look for a signed release on GitHub, download it in the background and
//! install it, on request or when Cubby closes. Never while a game is running.

use std::sync::Mutex;

use cubby_instance::{ProxyMode, UpdateChannel, UpdateMode};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::commands::CmdResult;
use crate::state::Core;

/// Task id of the download, as the UI sees it in progress events.
const TASK: &str = "self-update";

const REPO: &str = "https://github.com/benjaminbrtrd/Cubby";

/// The manifest of a channel lives on a fixed release, refreshed each time a release is published
/// (see `.github/workflows/update-manifest.yml`): GitHub's "latest" skips preview releases.
fn manifest_url(channel: UpdateChannel) -> String {
    let tag = match channel {
        UpdateChannel::Stable => "updates-stable",
        UpdateChannel::Preview => "updates-preview",
    };
    format!("{REPO}/releases/download/{tag}/latest.json")
}

/// The update found by the last check, and its installer once downloaded.
#[derive(Default)]
pub struct Pending {
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
}

pub type PendingState = Mutex<Pending>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDto {
    pub version: String,
    pub current_version: String,
    /// Release notes, as written on the release page (markdown).
    pub notes: String,
    pub date: Option<String>,
    /// The installer is downloaded and waiting.
    pub downloaded: bool,
}

fn dto(update: &Update, downloaded: bool) -> UpdateDto {
    UpdateDto {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone().unwrap_or_default(),
        date: update.date.map(|d| d.to_string()),
        downloaded,
    }
}

fn fail(what: &str, e: impl std::fmt::Display) -> String {
    log::warn!("update: {what}: {e}");
    format!("{what}: {e}")
}

/// Looks for a newer version on the settings' channel. `None` when Cubby is up to date.
#[tauri::command]
pub async fn check_update(app: AppHandle, core: State<'_, Core>, pending: State<'_, PendingState>) -> CmdResult<Option<UpdateDto>> {
    let settings = core.settings();
    let mut builder = app
        .updater_builder()
        .endpoints(vec![manifest_url(settings.updates.channel).parse().expect("manifest url")])
        .map_err(|e| fail("Couldn't check for updates", e))?;
    builder = match settings.proxy.mode {
        ProxyMode::System => builder,
        ProxyMode::None => builder.no_proxy(),
        ProxyMode::Manual => match settings.proxy.url.trim().parse() {
            Ok(url) => builder.proxy(url),
            Err(_) => builder,
        },
    };
    let updater = builder.build().map_err(|e| fail("Couldn't check for updates", e))?;
    let found = updater.check().await.map_err(|e| fail("Couldn't check for updates", e))?;
    let mut pending = pending.lock().expect("update lock");
    match found {
        Some(update) => {
            // The same version found again keeps its downloaded installer.
            let same = pending.update.as_ref().is_some_and(|u| u.version == update.version);
            let downloaded = same && pending.bytes.is_some();
            let out = dto(&update, downloaded);
            if !same {
                pending.bytes = None;
            }
            pending.update = Some(update);
            Ok(Some(out))
        }
        None => {
            *pending = Pending::default();
            Ok(None)
        }
    }
}

/// Downloads the installer found by `check_update`. Progress goes out on the `update` task.
#[tauri::command]
pub async fn download_update(core: State<'_, Core>, pending: State<'_, PendingState>) -> CmdResult<()> {
    let update = pending.lock().expect("update lock").update.clone().ok_or("There is no update to download.")?;
    if pending.lock().expect("update lock").bytes.is_some() {
        return Ok(());
    }
    let label = format!("Downloading Cubby {}", update.version);
    let sink = core.sink.clone();
    sink.progress(TASK, &label, 0, 0);
    let mut done: u64 = 0;
    let result = update
        .download(
            |chunk, total| {
                done += chunk as u64;
                sink.progress(TASK, &label, done, total.unwrap_or(0));
            },
            || {},
        )
        .await;
    match result {
        Ok(bytes) => {
            pending.lock().expect("update lock").bytes = Some(bytes);
            core.sink.finished(TASK, None);
            Ok(())
        }
        Err(e) => {
            let message = fail("Couldn't download the update", &e);
            core.sink.finished(TASK, Some(message.clone()));
            Err(message)
        }
    }
}

/// Installs the downloaded update and restarts Cubby. Refuses while a game is running.
#[tauri::command]
pub async fn install_update(app: AppHandle, core: State<'_, Core>, pending: State<'_, PendingState>) -> CmdResult<()> {
    if !core.games.running().is_empty() {
        return Err("Minecraft is still running. Close the game first.".into());
    }
    let (update, bytes) = {
        let pending = pending.lock().expect("update lock");
        (pending.update.clone(), pending.bytes.clone())
    };
    let (update, bytes) = update.zip(bytes).ok_or("The update isn't downloaded yet.")?;
    // On Windows the installer takes over and ends this process; elsewhere Cubby restarts itself.
    update.install(bytes).map_err(|e| fail("Couldn't install the update", e))?;
    app.restart();
}

/// Called when Cubby exits: a downloaded update goes in now, unless a game is still running or the
/// player turned updates off.
pub fn install_on_exit(app: &AppHandle) {
    let core = app.state::<Core>();
    if core.settings().updates.mode == UpdateMode::Off || !core.games.running().is_empty() {
        return;
    }
    let (update, bytes) = {
        let pending = app.state::<PendingState>();
        let pending = pending.lock().expect("update lock");
        (pending.update.clone(), pending.bytes.clone())
    };
    if let Some((update, bytes)) = update.zip(bytes)
        && let Err(e) = update.install(bytes)
    {
        log::warn!("update: could not install on exit: {e}");
    }
}
