//! Minecraft Java "Server List Ping" (modern protocol, 1.7+).
//!
//! [`ping`] never fails: an unreachable or misbehaving server is reported as offline.
//! SRV records (`_minecraft._tcp`) are not resolved; the address is used as given, with 25565 as
//! the default port.

use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

const PROTOCOL_VERSION: i32 = 767;
const DEFAULT_PORT: u16 = 25565;
const MAX_PACKET: usize = 1 << 22;

/// Result of pinging a server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub online: bool,
    /// Message of the day as plain text (formatting codes stripped, newlines kept).
    pub motd: String,
    /// Message of the day with its colours and styles as `§` codes, the way the game draws it: `§0`–`§f` colours,
    /// `§l` bold, `§o` italic, `§n` underlined, `§m` struck, `§k` obfuscated, `§r` reset, and `§#rrggbb` for a
    /// hex colour from a JSON component.
    pub motd_formatted: String,
    pub players_online: u32,
    pub players_max: u32,
    pub version: Option<String>,
    /// The protocol number the server reports. A server that takes several versions (a proxy, ViaVersion) answers
    /// with the protocol the client asked with when it accepts it, and with its own otherwise.
    pub protocol: Option<i32>,
    /// The protocol the ping was made with.
    pub asked_protocol: i32,
    /// Round trip of the ping/pong exchange, in milliseconds.
    pub ping_ms: Option<u32>,
    /// The `data:image/png;base64,...` icon, when the server has one.
    pub favicon: Option<String>,
}

impl ServerStatus {
    fn offline() -> Self {
        Self { online: false, motd: String::new(), motd_formatted: String::new(), players_online: 0, players_max: 0, version: None, protocol: None, asked_protocol: PROTOCOL_VERSION, ping_ms: None, favicon: None }
    }
}

/// Pings `address` (`host`, `host:port` or `[ipv6]:port`), bounded by `timeout` as a whole.
pub async fn ping(address: &str, timeout: Duration) -> ServerStatus {
    ping_with(address, timeout, PROTOCOL_VERSION).await
}

/// Like [`ping`], asking with the protocol number of a given Minecraft version: the answer tells whether the server
/// takes that version (see [`ServerStatus::protocol`]).
pub async fn ping_with(address: &str, timeout: Duration, protocol: i32) -> ServerStatus {
    let Some((host, port)) = parse_address(address) else {
        return ServerStatus::offline();
    };
    match tokio::time::timeout(timeout, query(&host, port, protocol)).await {
        Ok(Ok(status)) => status,
        _ => ServerStatus::offline(),
    }
}

/// The protocol number of a Minecraft version, from the `version.json` inside its client jar (versions from 1.14 on);
/// `None` for an older or unreadable jar.
pub fn client_protocol(jar: &std::path::Path) -> Option<i32> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    let mut text = String::new();
    zip.by_name("version.json").ok()?.take(64 * 1024).read_to_string(&mut text).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    i32::try_from(value["protocol_version"].as_i64()?).ok()
}

fn parse_address(address: &str) -> Option<(String, u16)> {
    let address = address.trim();
    if address.is_empty() {
        return None;
    }
    if let Some(rest) = address.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        if host.is_empty() {
            return None;
        }
        return match tail {
            "" => Some((host.to_string(), DEFAULT_PORT)),
            _ => Some((host.to_string(), tail.strip_prefix(':')?.parse().ok()?)),
        };
    }
    match address.matches(':').count() {
        0 => Some((address.to_string(), DEFAULT_PORT)),
        1 => {
            let (host, port) = address.split_once(':')?;
            if host.is_empty() {
                return None;
            }
            Some((host.to_string(), port.parse().ok()?))
        }
        // A bare IPv6 address without brackets.
        _ => Some((address.to_string(), DEFAULT_PORT)),
    }
}

async fn query(host: &str, port: u16, protocol: i32) -> std::io::Result<ServerStatus> {
    let mut stream = TcpStream::connect((host, port)).await?;
    let _ = stream.set_nodelay(true);

    let mut handshake = Vec::new();
    write_varint(&mut handshake, 0x00);
    write_varint(&mut handshake, protocol);
    write_string(&mut handshake, host);
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, 1);
    send_packet(&mut stream, &handshake).await?;
    send_packet(&mut stream, &[0x00]).await?;

    let response = read_packet(&mut stream).await?;
    let mut cursor = &response[..];
    if read_varint_slice(&mut cursor)? != 0x00 {
        return Err(invalid("unexpected status packet id"));
    }
    let len = read_varint_slice(&mut cursor)? as usize;
    let json = cursor.get(..len).ok_or_else(|| invalid("truncated status string"))?;
    let value: Value = serde_json::from_slice(json).map_err(|e| invalid(&e.to_string()))?;
    let mut status = parse_status(&value);
    status.asked_protocol = protocol;

    // The ping is best effort: a server that does not answer it is still online.
    let token = 0x5348_4152_4400_0001i64;
    let mut payload = vec![0x01];
    payload.extend_from_slice(&token.to_be_bytes());
    let started = Instant::now();
    if send_packet(&mut stream, &payload).await.is_ok() {
        if let Ok(pong) = read_packet(&mut stream).await {
            if pong.first() == Some(&0x01) {
                status.ping_ms = Some(started.elapsed().as_millis().min(u32::MAX as u128) as u32);
            }
        }
    }
    Ok(status)
}

fn parse_status(value: &Value) -> ServerStatus {
    let players = &value["players"];
    ServerStatus {
        online: true,
        motd: value.get("description").map(flatten_motd).unwrap_or_default(),
        motd_formatted: value.get("description").map(formatted_motd).unwrap_or_default(),
        players_online: players["online"].as_u64().unwrap_or(0) as u32,
        players_max: players["max"].as_u64().unwrap_or(0) as u32,
        version: value["version"]["name"].as_str().map(str::to_string),
        protocol: value["version"]["protocol"].as_i64().and_then(|p| i32::try_from(p).ok()),
        asked_protocol: PROTOCOL_VERSION,
        ping_ms: None,
        favicon: value["favicon"].as_str().map(str::to_string),
    }
}

/// Flattens a chat component (string, object with `text`/`translate`/`extra`, or array) to plain
/// text and strips `§` formatting codes.
fn flatten_motd(value: &Value) -> String {
    let mut out = String::new();
    push_component(value, &mut out);
    strip_formatting(&out)
}

fn push_component(value: &Value, out: &mut String) {
    match value {
        Value::String(s) => out.push_str(s),
        Value::Array(items) => items.iter().for_each(|item| push_component(item, out)),
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(Value::as_str) {
                out.push_str(text);
            } else if let Some(translate) = map.get("translate").and_then(Value::as_str) {
                out.push_str(translate);
            }
            if let Some(extra) = map.get("extra") {
                push_component(extra, out);
            }
        }
        _ => {}
    }
}

/// Colours and styles a chat component sets, inherited by its `extra` children.
#[derive(Debug, Clone, Default)]
struct Style {
    color: Option<String>,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
    obfuscated: bool,
}

impl Style {
    fn with(&self, map: &serde_json::Map<String, Value>) -> Style {
        let flag = |key: &str, current: bool| map.get(key).and_then(Value::as_bool).unwrap_or(current);
        Style {
            color: map.get("color").and_then(Value::as_str).and_then(color_code).or_else(|| self.color.clone()),
            bold: flag("bold", self.bold),
            italic: flag("italic", self.italic),
            underlined: flag("underlined", self.underlined),
            strikethrough: flag("strikethrough", self.strikethrough),
            obfuscated: flag("obfuscated", self.obfuscated),
        }
    }

    /// `§r`, then the colour, then the styles: what puts the text in this style whatever came before.
    fn codes(&self) -> String {
        let mut out = String::from("§r");
        if let Some(color) = &self.color {
            out.push_str(color);
        }
        for (on, code) in [(self.obfuscated, 'k'), (self.bold, 'l'), (self.strikethrough, 'm'), (self.underlined, 'n'), (self.italic, 'o')] {
            if on {
                out.push('§');
                out.push(code);
            }
        }
        out
    }
}

/// `§` code of a JSON colour: a name (`gold`) or `#rrggbb`.
fn color_code(name: &str) -> Option<String> {
    const NAMES: [&str; 16] = [
        "black", "dark_blue", "dark_green", "dark_aqua", "dark_red", "dark_purple", "gold", "gray",
        "dark_gray", "blue", "green", "aqua", "red", "light_purple", "yellow", "white",
    ];
    if let Some(i) = NAMES.iter().position(|n| *n == name) {
        return Some(format!("§{:x}", i));
    }
    let hex = name.strip_prefix('#')?;
    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())).then(|| format!("§#{}", hex.to_ascii_lowercase()))
}

/// The message of the day with `§` codes for its colours and styles (see [`ServerStatus::motd_formatted`]).
fn formatted_motd(value: &Value) -> String {
    let mut out = String::new();
    push_formatted(value, &Style::default(), &mut out);
    out
}

fn push_formatted(value: &Value, style: &Style, out: &mut String) {
    match value {
        Value::String(s) => {
            if !s.is_empty() {
                out.push_str(&style.codes());
                out.push_str(s);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| push_formatted(item, style, out)),
        Value::Object(map) => {
            let style = style.with(map);
            if let Some(text) = map.get("text").or_else(|| map.get("translate")).and_then(Value::as_str) {
                if !text.is_empty() {
                    out.push_str(&style.codes());
                    out.push_str(text);
                }
            }
            if let Some(extra) = map.get("extra") {
                push_formatted(extra, &style, out);
            }
        }
        _ => {}
    }
}

fn strip_formatting(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

fn invalid(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.to_string())
}

fn write_varint(buf: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            buf.push(byte);
            return;
        }
        buf.push(byte | 0x80);
    }
}

fn write_string(buf: &mut Vec<u8>, text: &str) {
    write_varint(buf, text.len() as i32);
    buf.extend_from_slice(text.as_bytes());
}

fn read_varint_slice(cursor: &mut &[u8]) -> std::io::Result<i32> {
    let mut result = 0u32;
    for i in 0..5 {
        let (&byte, rest) = cursor.split_first().ok_or_else(|| invalid("truncated VarInt"))?;
        *cursor = rest;
        result |= ((byte & 0x7f) as u32) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok(result as i32);
        }
    }
    Err(invalid("VarInt too long"))
}

async fn read_varint<R: AsyncRead + Unpin>(reader: &mut R) -> std::io::Result<i32> {
    let mut result = 0u32;
    for i in 0..5 {
        let byte = reader.read_u8().await?;
        result |= ((byte & 0x7f) as u32) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok(result as i32);
        }
    }
    Err(invalid("VarInt too long"))
}

async fn send_packet<W: AsyncWrite + Unpin>(writer: &mut W, body: &[u8]) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(body.len() + 5);
    write_varint(&mut frame, body.len() as i32);
    frame.extend_from_slice(body);
    writer.write_all(&frame).await?;
    writer.flush().await
}

async fn read_packet<R: AsyncRead + Unpin>(reader: &mut R) -> std::io::Result<Vec<u8>> {
    let len = read_varint(reader).await?;
    if len <= 0 || len as usize > MAX_PACKET {
        return Err(invalid("invalid packet length"));
    }
    let mut body = vec![0u8; len as usize];
    reader.read_exact(&mut body).await?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::net::TcpListener;

    #[test]
    fn varint_roundtrip() {
        for value in [0, 1, 127, 128, 255, 25565, 767, 2_147_483_647, -1, i32::MIN] {
            let mut buf = Vec::new();
            write_varint(&mut buf, value);
            let mut cursor = &buf[..];
            assert_eq!(read_varint_slice(&mut cursor).unwrap(), value);
            assert!(cursor.is_empty());
        }
        let mut buf = Vec::new();
        write_varint(&mut buf, 300);
        assert_eq!(buf, [0xac, 0x02]);
        let mut buf = Vec::new();
        write_varint(&mut buf, -1);
        assert_eq!(buf, [0xff, 0xff, 0xff, 0xff, 0x0f]);
        assert!(read_varint_slice(&mut &[0x80u8, 0x80][..]).is_err());
    }

    #[test]
    fn motd_formatting() {
        assert_eq!(formatted_motd(&json!("§aHello §lworld")), "§r§aHello §lworld");
        let component = json!({"text": "Cubby ", "color": "gold", "bold": true, "extra": [{"text": "test", "bold": false, "color": "#4CC9D6"}, "!"]});
        assert_eq!(formatted_motd(&component), "§r§6§lCubby §r§#4cc9d6test§r§6§l!");
        assert_eq!(formatted_motd(&json!({"text": "", "extra": [{"text": "x", "italic": true}]})), "§r§ox");
        assert_eq!(color_code("pink"), None);
    }

    #[test]
    fn motd_flattening() {
        assert_eq!(flatten_motd(&json!("§aHello §lworld")), "Hello world");
        let nested = json!({"text": "A", "extra": [{"text": "B", "extra": [{"text": "C"}]}, "D\n", {"translate": "x.y"}]});
        assert_eq!(flatten_motd(&nested), "ABCD\nx.y");
        assert_eq!(flatten_motd(&json!({"text": "§6Gold\nline2"})), "Gold\nline2");
    }

    #[test]
    fn address_parsing() {
        assert_eq!(parse_address("example.com"), Some(("example.com".into(), 25565)));
        assert_eq!(parse_address("example.com:1234"), Some(("example.com".into(), 1234)));
        assert_eq!(parse_address("[::1]:1234"), Some(("::1".into(), 1234)));
        assert_eq!(parse_address("[::1]"), Some(("::1".into(), 25565)));
        assert_eq!(parse_address("a:notaport"), None);
        assert_eq!(parse_address(""), None);
    }

    #[tokio::test]
    async fn end_to_end_against_fake_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let status_json = json!({
            "version": {"name": "1.21", "protocol": 767},
            "players": {"max": 20, "online": 3},
            "description": {"text": "§aCubby ", "extra": [{"text": "test"}]},
            "favicon": "data:image/png;base64,AAAA"
        })
        .to_string();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let handshake = read_packet(&mut socket).await.unwrap();
            let mut cursor = &handshake[..];
            assert_eq!(read_varint_slice(&mut cursor).unwrap(), 0);
            assert_eq!(read_varint_slice(&mut cursor).unwrap(), PROTOCOL_VERSION);
            assert_eq!(read_packet(&mut socket).await.unwrap(), [0x00]);
            let mut body = vec![0x00];
            write_string(&mut body, &status_json);
            send_packet(&mut socket, &body).await.unwrap();
            let ping = read_packet(&mut socket).await.unwrap();
            assert_eq!(ping[0], 0x01);
            send_packet(&mut socket, &ping).await.unwrap();
        });

        let status = ping(&format!("127.0.0.1:{port}"), Duration::from_secs(5)).await;
        assert!(status.online);
        assert_eq!(status.motd, "Cubby test");
        assert_eq!((status.players_online, status.players_max), (3, 20));
        assert_eq!(status.version.as_deref(), Some("1.21"));
        assert_eq!((status.protocol, status.asked_protocol), (Some(767), PROTOCOL_VERSION));
        assert_eq!(status.favicon.as_deref(), Some("data:image/png;base64,AAAA"));
        assert!(status.ping_ms.is_some());
    }

    #[test]
    fn reads_the_protocol_of_a_client_jar() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("cubby-protocol-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let jar = dir.join("c.jar");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
        zip.start_file("version.json", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"{"id":"1.21.1","protocol_version":767,"world_version":3955}"#).unwrap();
        zip.finish().unwrap();
        assert_eq!(client_protocol(&jar), Some(767));
        assert_eq!(client_protocol(&dir.join("missing.jar")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn the_protocol_asked_is_the_one_sent() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let handshake = read_packet(&mut socket).await.unwrap();
            let mut cursor = &handshake[..];
            assert_eq!(read_varint_slice(&mut cursor).unwrap(), 0);
            let asked = read_varint_slice(&mut cursor).unwrap();
            let _ = read_packet(&mut socket).await.unwrap();
            // a server that takes several versions answers with the protocol asked
            let mut body = vec![0x00];
            write_string(&mut body, &json!({"version": {"name": "Requires MC 1.8 / 1.21", "protocol": asked}, "players": {"max": 1, "online": 0}, "description": "x"}).to_string());
            send_packet(&mut socket, &body).await.unwrap();
        });
        let status = ping_with(&format!("127.0.0.1:{port}"), Duration::from_secs(5), 773).await;
        assert!(status.online);
        assert_eq!((status.protocol, status.asked_protocol), (Some(773), 773));
    }

    #[tokio::test]
    async fn unreachable_server_is_offline() {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            listener.local_addr().unwrap().port()
        };
        let started = Instant::now();
        let status = ping(&format!("127.0.0.1:{port}"), Duration::from_secs(3)).await;
        assert_eq!(status, ServerStatus::offline());
        assert!(started.elapsed() < Duration::from_secs(4));
    }
}
