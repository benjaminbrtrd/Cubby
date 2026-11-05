//! Tauri commands called by the UI (`ui/src/lib/api.ts`). Errors are user-facing English messages.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cubby_auth::AccountStore;
use cubby_common::contracts::{JavaInstall, LaunchCredentials};
use cubby_common::events::EventSink;
use cubby_instance::{Instance, Loader, ModpackInfo, NewInstance, OnLaunch, PackKind, Settings};
use cubby_launch::{LaunchOptions, LogLine, QuickPlay, RunningGame};
use cubby_meta::{LoaderKind, LoaderSpec};
use cubby_modrinth::{Folder, PackLoader, Renamed, SearchQuery, Target};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::watch;

use crate::dto::{self, *};
use crate::state::{Core, PendingLogin};

pub(crate) mod accounts;
pub(crate) mod app;
pub(crate) mod content;
pub(crate) mod discover;
pub(crate) mod instances;
pub(crate) mod launch;
pub(crate) mod prism;
pub(crate) mod settings;

pub(crate) use content::*;
pub(crate) use discover::*;
pub(crate) use instances::*;

pub(crate) type CmdResult<T> = Result<T, String>;

pub(crate) trait OrMessage<T> {
    fn msg(self) -> CmdResult<T>;
}

impl<T> OrMessage<T> for cubby_common::Result<T> {
    fn msg(self) -> CmdResult<T> {
        self.map_err(dto::message)
    }
}

const NO_MSA_CLIENT_ID: &str = "This build of Cubby has no Microsoft application ID. Add one in Settings › Accounts, or set CUBBY_MSA_CLIENT_ID.";

pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("The task stopped unexpectedly ({e})."))?
}

// Background tasks

struct TaskSlot {
    app: AppHandle,
    task: String,
}

impl Drop for TaskSlot {
    fn drop(&mut self) {
        self.app
            .state::<Core>()
            .tasks
            .lock()
            .expect("tasks lock")
            .remove(&self.task);
    }
}

/// Runs `work` on the async runtime as task `task`, one task per id at a time. Its `TaskFinished` event
/// carries the error, if any. The returned handle yields the result.
pub(crate) fn spawn_task<T, Fut>(
    app: &AppHandle,
    task: String,
    work: impl FnOnce(AppHandle, String) -> Fut,
) -> CmdResult<tauri::async_runtime::JoinHandle<CmdResult<T>>>
where
    T: Send + 'static,
    Fut: Future<Output = CmdResult<T>> + Send + 'static,
{
    if !app
        .state::<Core>()
        .tasks
        .lock()
        .expect("tasks lock")
        .insert(task.clone())
    {
        return Err(
            "Another task is already running on this instance. Wait for it to finish.".into(),
        );
    }
    let slot = TaskSlot {
        app: app.clone(),
        task: task.clone(),
    };
    let work = work(app.clone(), task.clone());
    let app = app.clone();
    Ok(tauri::async_runtime::spawn(async move {
        let result = work.await;
        drop(slot);
        app.state::<Core>()
            .sink
            .finished(&task, result.as_ref().err().cloned());
        result
    }))
}

async fn run_task<T, Fut>(
    app: &AppHandle,
    task: String,
    work: impl FnOnce(AppHandle, String) -> Fut,
) -> CmdResult<T>
where
    T: Send + 'static,
    Fut: Future<Output = CmdResult<T>> + Send + 'static,
{
    match spawn_task(app, task.clone(), work)?.await {
        Ok(result) => result,
        Err(e) => {
            let message = format!("The task stopped unexpectedly ({e}).");
            app.state::<Core>()
                .sink
                .finished(&task, Some(message.clone()));
            Err(message)
        }
    }
}

fn ensure_stopped(core: &Core, id: &str) -> CmdResult<()> {
    if core.games.is_running(id) {
        return Err("Close the game before changing this instance.".into());
    }
    Ok(())
}

pub(crate) fn download_concurrency(core: &Core) -> usize {
    core.settings().download_concurrency.clamp(1, 64) as usize
}
