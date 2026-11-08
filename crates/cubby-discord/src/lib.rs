//! Discord Rich Presence: shows "Playing Minecraft · <instance>" on the player's Discord profile.
//!
//! Talks to the Discord app running on the same computer through its local IPC socket (a named pipe on
//! Windows), like the official SDK: a handshake with the application id, then `SET_ACTIVITY`. Nothing goes
//! over the network from Cubby. When Discord isn't running, updates are dropped and retried on the next one.
//!
//! The application id comes from a Discord application whose Rich Presence art has an asset named `cubby`.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use serde_json::{json, Value};

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;
const MAX_FRAME: usize = 64 * 1024;

/// What the player is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    /// First line: "Playing All the Mods 9".
    pub details: String,
    /// Second line: "Minecraft 1.20.1 · Forge".
    pub state: String,
    /// Unix time the game started, in seconds: Discord shows the elapsed time.
    pub started: i64,
}

/// The JSON of a `SET_ACTIVITY` command. `None` clears the activity.
pub fn set_activity_payload(pid: u32, activity: Option<&Activity>, nonce: u64) -> Value {
    let activity = activity.map(|a| {
        json!({
            "details": clip(&a.details),
            "state": clip(&a.state),
            "timestamps": { "start": a.started },
            "assets": { "large_image": "cubby", "large_text": "Cubby" },
        })
    });
    json!({ "cmd": "SET_ACTIVITY", "args": { "pid": pid, "activity": activity }, "nonce": nonce.to_string() })
}

/// Discord refuses texts longer than 128 characters (and shorter than 2).
fn clip(text: &str) -> String {
    let mut out: String = text.chars().take(128).collect();
    while out.chars().count() < 2 {
        out.push(' ');
    }
    out
}

/// A frame: operation code and length (little endian), then the JSON.
pub fn encode_frame(op: u32, payload: &Value) -> Vec<u8> {
    let body = payload.to_string();
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend(op.to_le_bytes());
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body.as_bytes());
    out
}

trait Pipe: Read + Write + Send {}
impl<T: Read + Write + Send> Pipe for T {}

#[cfg(unix)]
fn connect() -> Option<Box<dyn Pipe>> {
    use std::os::unix::net::UnixStream;
    let mut bases: Vec<std::path::PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"].iter().filter_map(std::env::var_os).map(Into::into).collect();
    bases.push("/tmp".into());
    // Flatpak and Snap builds of Discord put the socket in their own folder.
    let subs = ["", "app/com.discordapp.Discord", "app/com.discordapp.DiscordCanary", "snap.discord", ".flatpak/com.discordapp.Discord/xdg-run"];
    for base in &bases {
        for sub in subs {
            for n in 0..10 {
                let path = base.join(sub).join(format!("discord-ipc-{n}"));
                if let Ok(stream) = UnixStream::connect(&path) {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                    return Some(Box::new(stream));
                }
            }
        }
    }
    None
}

#[cfg(windows)]
fn connect() -> Option<Box<dyn Pipe>> {
    for n in 0..10 {
        let path = format!(r"\\.\pipe\discord-ipc-{n}");
        if let Ok(file) = std::fs::OpenOptions::new().read(true).write(true).open(&path) {
            return Some(Box::new(file));
        }
    }
    None
}

#[cfg(not(any(unix, windows)))]
fn connect() -> Option<Box<dyn Pipe>> {
    None
}

fn read_frame(pipe: &mut dyn Pipe) -> std::io::Result<(u32, Value)> {
    let mut head = [0u8; 8];
    pipe.read_exact(&mut head)?;
    let op = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
    let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
    if len > MAX_FRAME {
        return Err(std::io::Error::other("Discord sent a frame that is too big"));
    }
    let mut body = vec![0u8; len];
    pipe.read_exact(&mut body)?;
    Ok((op, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

struct Connection {
    pipe: Box<dyn Pipe>,
}

impl Connection {
    fn open(client_id: &str) -> Option<Self> {
        let mut pipe = connect()?;
        pipe.write_all(&encode_frame(OP_HANDSHAKE, &json!({ "v": 1, "client_id": client_id }))).ok()?;
        match read_frame(pipe.as_mut()) {
            Ok((OP_FRAME, v)) if v.get("evt").and_then(Value::as_str) == Some("READY") => Some(Self { pipe }),
            Ok((op, v)) => {
                tracing::debug!("Discord refused the handshake (op {op}): {v}");
                None
            }
            Err(e) => {
                tracing::debug!("no answer from Discord: {e}");
                None
            }
        }
    }

    fn send(&mut self, payload: &Value) -> std::io::Result<()> {
        self.pipe.write_all(&encode_frame(OP_FRAME, payload))?;
        let (op, answer) = read_frame(self.pipe.as_mut())?;
        if op == OP_CLOSE {
            return Err(std::io::Error::other("Discord closed the connection"));
        }
        if answer.get("evt").and_then(Value::as_str) == Some("ERROR") {
            tracing::debug!("Discord didn't take the activity: {answer}");
        }
        Ok(())
    }
}

enum Message {
    Set(Option<Activity>),
    Stop,
}

/// Sends activities to Discord from a thread of its own, so a slow or missing Discord never holds up Cubby.
/// Dropping it clears the activity.
pub struct Presence {
    tx: Sender<Message>,
}

impl Presence {
    /// Starts the worker for this Discord application id.
    pub fn new(client_id: String) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || worker(client_id, rx))
            .map_err(|e| tracing::warn!("couldn't start the Discord thread: {e}"))
            .ok();
        Self { tx }
    }

    /// Shows this activity, or clears it with `None`. Only the newest one counts.
    pub fn set(&self, activity: Option<Activity>) {
        let _ = self.tx.send(Message::Set(activity));
    }
}

impl Drop for Presence {
    fn drop(&mut self) {
        let _ = self.tx.send(Message::Set(None));
        let _ = self.tx.send(Message::Stop);
    }
}

fn worker(client_id: String, rx: Receiver<Message>) {
    let mut conn: Option<Connection> = None;
    let mut nonce = 0u64;
    let pid = std::process::id();
    while let Ok(first) = rx.recv() {
        // Skip to the newest update: a game that started and stopped while Discord was slow shows nothing.
        let mut msg = first;
        while let Ok(next) = rx.try_recv() {
            msg = next;
        }
        let activity = match msg {
            Message::Stop => break,
            Message::Set(a) => a,
        };
        if conn.is_none() {
            if activity.is_none() {
                continue;
            }
            conn = Connection::open(&client_id);
        }
        let Some(c) = conn.as_mut() else { continue };
        nonce += 1;
        if let Err(e) = c.send(&set_activity_payload(pid, activity.as_ref(), nonce)) {
            tracing::debug!("lost Discord: {e}");
            conn = None;
            // Discord may have restarted: try once more with a new connection.
            if let Some(mut fresh) = activity.as_ref().and_then(|_| Connection::open(&client_id)) {
                if fresh.send(&set_activity_payload(pid, activity.as_ref(), nonce)).is_ok() {
                    conn = Some(fresh);
                }
            }
        }
    }
}

/// The Discord application id: `CUBBY_DISCORD_CLIENT_ID` when Cubby runs, else the one it was built with.
pub fn client_id() -> Option<String> {
    let clean = |s: &str| {
        let s = s.trim();
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.to_string())
    };
    std::env::var("CUBBY_DISCORD_CLIENT_ID")
        .ok()
        .as_deref()
        .and_then(clean)
        .or_else(|| option_env!("CUBBY_DISCORD_CLIENT_ID").and_then(clean))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_payloads() {
        let frame = encode_frame(OP_HANDSHAKE, &json!({"v": 1}));
        assert_eq!(&frame[..4], &[0, 0, 0, 0]);
        assert_eq!(u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize, frame.len() - 8);

        let a = Activity { details: "Playing All the Mods 9".into(), state: "Minecraft 1.20.1 · Forge".into(), started: 1_700_000_000 };
        let p = set_activity_payload(42, Some(&a), 7);
        assert_eq!(p["cmd"], "SET_ACTIVITY");
        assert_eq!(p["args"]["pid"], 42);
        assert_eq!(p["args"]["activity"]["timestamps"]["start"], 1_700_000_000);
        assert_eq!(p["args"]["activity"]["assets"]["large_image"], "cubby");
        assert_eq!(p["nonce"], "7");
        assert!(set_activity_payload(42, None, 8)["args"]["activity"].is_null());
        assert_eq!(clip("x"), "x ");
        assert_eq!(clip(&"é".repeat(200)).chars().count(), 128);
    }

    #[test]
    fn no_discord_is_fine() {
        // Without Discord running, updates are dropped quietly.
        let presence = Presence::new("1".into());
        presence.set(Some(Activity { details: "Playing".into(), state: "Minecraft".into(), started: 0 }));
        presence.set(None);
        drop(presence);
    }
}
