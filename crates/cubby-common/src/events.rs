//! Events emitted by long-running operations. The Tauri app forwards them to the frontend as-is,
//! so every type here is `Serialize` with camelCase fields.

use serde::Serialize;

/// Receives events. Implemented by the Tauri app (emits to the window) and by tests (collects).
pub trait EventSink: Send + Sync {
    fn emit(&self, event: Event);
}

/// Discards every event.
pub struct NoopSink;

impl EventSink for NoopSink {
    fn emit(&self, _event: Event) {}
}

impl<F: Fn(Event) + Send + Sync> EventSink for F {
    fn emit(&self, event: Event) {
        self(event)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    /// Progress of a named task (installing an instance, downloading Java...).
    #[serde(rename_all = "camelCase")]
    Progress {
        /// Stable id of the task, e.g. `install:<instance-id>`.
        task: String,
        /// Human-readable current step, e.g. "Downloading libraries".
        label: String,
        done: u64,
        total: u64,
    },
    /// A task finished, successfully or not.
    #[serde(rename_all = "camelCase")]
    TaskFinished { task: String, error: Option<String> },
    /// One line printed by a running game.
    #[serde(rename_all = "camelCase")]
    GameLog { instance_id: String, line: String, stream: LogStream },
    /// A game process started or stopped.
    #[serde(rename_all = "camelCase")]
    GameState { instance_id: String, running: bool, exit_code: Option<i32> },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LogStream {
    Stdout,
    Stderr,
}
