use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use cubby_common::events::{Event, EventSink};
use tauri::{AppHandle, Emitter};

pub const EVENT_NAME: &str = "cubby://event";

/// Progress events are throttled per task: installing assets alone emits thousands of them.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(60);

/// Forwards core events to the window; progress labels are shown as the core crates word them.
pub struct TauriSink {
    app: AppHandle,
    last_progress: Mutex<HashMap<String, Instant>>,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app, last_progress: Mutex::new(HashMap::new()) }
    }

    pub fn progress(&self, task: &str, label: &str, done: u64, total: u64) {
        self.emit(Event::Progress { task: task.to_string(), label: label.to_string(), done, total });
    }

    pub fn finished(&self, task: &str, error: Option<String>) {
        self.last_progress.lock().expect("progress lock").remove(task);
        self.emit(Event::TaskFinished { task: task.to_string(), error });
    }
}

impl EventSink for TauriSink {
    fn emit(&self, event: Event) {
        let event = match event {
            Event::Progress { task, label, done, total } => {
                let boundary = done == 0 || done >= total;
                let mut last = self.last_progress.lock().expect("progress lock");
                let now = Instant::now();
                if !boundary && last.get(&task).is_some_and(|t| now.duration_since(*t) < PROGRESS_INTERVAL) {
                    return;
                }
                last.insert(task.clone(), now);
                Event::Progress { task, label, done, total }
            }
            other => other,
        };
        if let Err(e) = self.app.emit(EVENT_NAME, &event) {
            log::warn!("could not emit event: {e}");
        }
    }
}
