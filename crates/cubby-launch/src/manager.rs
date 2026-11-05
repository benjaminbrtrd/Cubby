use std::collections::{HashMap, VecDeque};
use std::process::Stdio;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chrono::{DateTime, Utc};
use cubby_common::events::{Event, EventSink, LogStream};
use cubby_common::{Error, Result};
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::{oneshot, watch};

use crate::command::LaunchCommand;
use crate::log4j::Log4jDecoder;

/// Number of log lines kept per instance.
pub const LOG_BUFFER_LINES: usize = 5_000;

const MASK: &str = "<MASKED>";
/// How long to wait for the output pipes to drain once the process is gone.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

/// One line printed by the game.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub line: String,
    pub stream: LogStream,
    pub time: DateTime<Utc>,
}

/// A game currently running.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningGame {
    pub instance_id: String,
    pub pid: Option<u32>,
    pub started_at: DateTime<Utc>,
}

/// How a game run ended. `duration` is the play time (to be added to the instance's total).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameExit {
    pub instance_id: String,
    /// `None` when the process was terminated by a signal.
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    /// Wall-clock run time.
    pub duration: Duration,
}

/// Returned by [`GameManager::launch`]: lets the caller await the end of the run and read its [`GameExit`].
#[derive(Debug)]
pub struct GameHandle {
    pub pid: Option<u32>,
    pub started_at: DateTime<Utc>,
    done: watch::Receiver<Option<GameExit>>,
}

impl GameHandle {
    /// Waits until the game has exited (and its output has been drained) and returns how it ended.
    pub async fn wait(mut self) -> GameExit {
        let result = self.done.wait_for(|v| v.is_some()).await;
        match result {
            Ok(v) => v.clone().expect("checked by predicate"),
            Err(_) => {
                // The supervisor task vanished (runtime shutdown): synthesize an exit.
                let now = Utc::now();
                GameExit {
                    instance_id: String::new(),
                    exit_code: None,
                    started_at: self.started_at,
                    ended_at: now,
                    duration: (now - self.started_at).to_std().unwrap_or_default(),
                }
            }
        }
    }
}

struct Active {
    pid: Option<u32>,
    started_at: DateTime<Utc>,
    kill: Option<oneshot::Sender<()>>,
}

#[derive(Default)]
struct Inner {
    active: Mutex<HashMap<String, Active>>,
    logs: Mutex<HashMap<String, VecDeque<LogLine>>>,
    last_exit: Mutex<HashMap<String, GameExit>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Spawns and supervises game processes, one per instance id. Cheap to clone.
#[derive(Clone, Default)]
pub struct GameManager {
    inner: Arc<Inner>,
}

impl GameManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts the game for `instance_id` and returns as soon as the process is spawned.
    ///
    /// Emits `GameState { running: true }`, one `GameLog` per output line (secrets masked) and finally
    /// `GameState { running: false, exit_code }`. The play time is available from the returned
    /// [`GameHandle::wait`] and afterwards from [`GameManager::last_exit`].
    /// Fails if that instance is already running. The log buffer of the instance is reset.
    pub async fn launch(&self, cmd: LaunchCommand, instance_id: &str, sink: Arc<dyn EventSink>) -> Result<GameHandle> {
        std::fs::create_dir_all(&cmd.cwd).map_err(|e| Error::io(&cmd.cwd, e))?;

        let mut process = tokio::process::Command::new(&cmd.program);
        process
            .args(&cmd.args)
            .current_dir(&cmd.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        process.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

        let started_at = Utc::now();
        let (kill_tx, kill_rx) = oneshot::channel();
        let (done_tx, done_rx) = watch::channel(None);
        let mut child;
        {
            let mut active = lock(&self.inner.active);
            if active.contains_key(instance_id) {
                return Err(Error::Launch(format!("Instance {instance_id} is already running.")));
            }
            child = process
                .spawn()
                .map_err(|e| Error::Launch(format!("Couldn't start {}: {e}", cmd.program.display())))?;
            active.insert(instance_id.to_string(), Active { pid: child.id(), started_at, kill: Some(kill_tx) });
            lock(&self.inner.logs).remove(instance_id);
            lock(&self.inner.last_exit).remove(instance_id);
        }
        let pid = child.id();

        sink.emit(Event::GameState { instance_id: instance_id.to_string(), running: true, exit_code: None });

        let secrets: Arc<Vec<String>> = Arc::new(cmd.secrets.into_iter().filter(|s| s.len() >= 8).collect());
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let inner = self.inner.clone();
        let id = instance_id.to_string();

        tokio::spawn(async move {
            let readers = [
                spawn_reader(stdout, LogStream::Stdout, &inner, &id, &sink, &secrets),
                spawn_reader(stderr, LogStream::Stderr, &inner, &id, &sink, &secrets),
            ];
            let status = tokio::select! {
                status = child.wait() => status,
                _ = kill_rx => {
                    let _ = child.kill().await;
                    child.wait().await
                }
            };
            for reader in readers.into_iter().flatten() {
                let _ = tokio::time::timeout(DRAIN_TIMEOUT, reader).await;
            }
            let exit_code = status.ok().and_then(|s| s.code());
            let ended_at = Utc::now();
            let exit = GameExit {
                instance_id: id.clone(),
                exit_code,
                started_at,
                ended_at,
                duration: (ended_at - started_at).to_std().unwrap_or_default(),
            };
            lock(&inner.last_exit).insert(id.clone(), exit.clone());
            lock(&inner.active).remove(&id);
            sink.emit(Event::GameState { instance_id: id, running: false, exit_code });
            let _ = done_tx.send(Some(exit));
        });

        Ok(GameHandle { pid, started_at, done: done_rx })
    }

    /// Asks the supervisor to kill the game; returns once the request is made (the exit event follows).
    pub fn kill(&self, instance_id: &str) -> Result<()> {
        let mut active = lock(&self.inner.active);
        let tx = active.get_mut(instance_id).and_then(|a| a.kill.take());
        match tx {
            Some(tx) => {
                let _ = tx.send(());
                Ok(())
            }
            None => Err(Error::Launch(format!("Instance {instance_id} isn't running."))),
        }
    }

    /// Whether the instance currently has a running game.
    pub fn is_running(&self, instance_id: &str) -> bool {
        lock(&self.inner.active).contains_key(instance_id)
    }

    /// All running games, oldest first.
    pub fn running(&self) -> Vec<RunningGame> {
        let mut games: Vec<_> = lock(&self.inner.active)
            .iter()
            .map(|(id, a)| RunningGame { instance_id: id.clone(), pid: a.pid, started_at: a.started_at })
            .collect();
        games.sort_by_key(|g| g.started_at);
        games
    }

    /// The last [`LOG_BUFFER_LINES`] lines of the instance's latest run (kept after the game exits).
    pub fn log_lines(&self, instance_id: &str) -> Vec<LogLine> {
        lock(&self.inner.logs).get(instance_id).map(|b| b.iter().cloned().collect()).unwrap_or_default()
    }

    /// How the instance's latest finished run ended, if any since the last launch started.
    pub fn last_exit(&self, instance_id: &str) -> Option<GameExit> {
        lock(&self.inner.last_exit).get(instance_id).cloned()
    }
}

fn spawn_reader<R: AsyncRead + Unpin + Send + 'static>(
    pipe: Option<R>,
    stream: LogStream,
    inner: &Arc<Inner>,
    id: &str,
    sink: &Arc<dyn EventSink>,
    secrets: &Arc<Vec<String>>,
) -> Option<tokio::task::JoinHandle<()>> {
    let pipe = pipe?;
    let (inner, id, sink, secrets) = (inner.clone(), id.to_string(), sink.clone(), secrets.clone());
    Some(tokio::spawn(async move {
        let mut reader = BufReader::new(pipe);
        let mut buf = Vec::new();
        let mut decoder = Log4jDecoder::default();
        loop {
            buf.clear();
            let lines = match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => decoder.flush(),
                Ok(_) => decoder.feed(String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n'])),
            };
            for line in lines {
                publish(&inner, &id, &sink, &secrets, line, stream);
            }
            if buf.is_empty() {
                break;
            }
        }
    }))
}

/// Masks the secrets of one display line, keeps it in the ring buffer and emits it.
fn publish(inner: &Inner, id: &str, sink: &Arc<dyn EventSink>, secrets: &[String], mut line: String, stream: LogStream) {
    for secret in secrets {
        if line.contains(secret.as_str()) {
            line = line.replace(secret.as_str(), MASK);
        }
    }
    {
        let mut logs = lock(&inner.logs);
        let buffer = logs.entry(id.to_string()).or_default();
        if buffer.len() >= LOG_BUFFER_LINES {
            buffer.pop_front();
        }
        buffer.push_back(LogLine { line: line.clone(), stream, time: Utc::now() });
    }
    sink.emit(Event::GameLog { instance_id: id.to_string(), line, stream });
}
