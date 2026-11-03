//! Decoding of the game's log4j XML output.
//!
//! The logging configuration shipped by Mojang (passed with `-Dlog4j.configurationFile`) makes the
//! game print every log record as an XML `<log4j:Event>` spread over several lines. The launcher
//! turns them back into the familiar `[12:34:56] [Render thread/INFO]: message` lines. Anything
//! outside an event (plain `System.out` output, JVM errors) passes through unchanged.

use chrono::{Local, TimeZone};

const EVENT_START: &str = "<log4j:Event";
const EVENT_END: &str = "</log4j:Event>";

/// Line-by-line decoder for one output stream.
#[derive(Debug, Default)]
pub struct Log4jDecoder {
    /// Lines of the event being read, if any.
    event: Option<String>,
}

impl Log4jDecoder {
    /// Feeds one output line; returns the display lines it completes (none while inside an event).
    pub fn feed(&mut self, line: &str) -> Vec<String> {
        match &mut self.event {
            Some(event) => {
                event.push('\n');
                event.push_str(line);
            }
            None if line.trim_start().starts_with(EVENT_START) => self.event = Some(line.to_string()),
            None => return vec![line.to_string()],
        }
        if !line.contains(EVENT_END) {
            return Vec::new();
        }
        let event = self.event.take().unwrap_or_default();
        decode_event(&event)
    }

    /// Returns what is left of an unfinished event (when the stream ends), as is.
    pub fn flush(&mut self) -> Vec<String> {
        self.event.take().map(|e| e.lines().map(str::to_owned).collect()).unwrap_or_default()
    }
}

/// Formats one complete `<log4j:Event>`; falls back to the raw lines when it cannot be read.
fn decode_event(xml: &str) -> Vec<String> {
    let Some(tag_end) = xml.find('>') else {
        return xml.lines().map(str::to_owned).collect();
    };
    let tag = &xml[..tag_end];
    let level = attribute(tag, "level").unwrap_or("INFO");
    let thread = attribute(tag, "thread").unwrap_or("main");
    let time = attribute(tag, "timestamp")
        .and_then(|t| t.parse::<i64>().ok())
        .and_then(|ms| Local.timestamp_millis_opt(ms).single())
        .map(|t| t.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "--:--:--".into());
    let message = element_text(xml, "log4j:Message").unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    let mut lines = message.lines();
    out.push(format!("[{time}] [{}/{level}]: {}", unescape(thread), lines.next().unwrap_or("")));
    out.extend(lines.map(str::to_owned));
    if let Some(throwable) = element_text(xml, "log4j:Throwable") {
        out.extend(throwable.lines().filter(|l| !l.is_empty()).map(str::to_owned));
    }
    out
}

/// Value of `name="..."` inside a start tag.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

/// Text content of the first `<name>` element: its CDATA sections joined, or its unescaped text.
fn element_text(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let len = xml[start..].find(&close)?;
    let body = &xml[start..start + len];
    if !body.contains("<![CDATA[") {
        return Some(unescape(body.trim()));
    }
    // A message containing "]]>" is split by log4j into several adjacent CDATA sections.
    let mut text = String::new();
    let mut rest = body;
    while let Some(i) = rest.find("<![CDATA[") {
        rest = &rest[i + "<![CDATA[".len()..];
        let end = rest.find("]]>").unwrap_or(rest.len());
        text.push_str(&rest[..end]);
        rest = rest.get(end + 3..).unwrap_or("");
    }
    Some(text)
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(lines: &[&str]) -> Vec<String> {
        let mut d = Log4jDecoder::default();
        lines.iter().flat_map(|l| d.feed(l)).collect()
    }

    #[test]
    fn decodes_an_event_spread_over_lines() {
        let out = feed_all(&[
            r#"<log4j:Event logger="gqk" timestamp="1793480760433" level="WARN" thread="Render thread">"#,
            "  <log4j:Message><![CDATA[Created: 256x128x0 atlas]]></log4j:Message>",
            "</log4j:Event>",
        ]);
        assert_eq!(out.len(), 1);
        assert!(out[0].ends_with("] [Render thread/WARN]: Created: 256x128x0 atlas"), "{}", out[0]);
        assert!(out[0].starts_with('['));
    }

    #[test]
    fn keeps_plain_lines_and_throwables() {
        let out = feed_all(&[
            "plain line",
            r#"<log4j:Event logger="x" timestamp="0" level="ERROR" thread="main">"#,
            "<log4j:Message><![CDATA[Boom ]]]]><![CDATA[> here]]></log4j:Message>",
            "<log4j:Throwable><![CDATA[java.lang.RuntimeException: x",
            "\tat a.b(C.java:1)",
            "]]></log4j:Throwable>",
            "</log4j:Event>",
        ]);
        assert_eq!(out[0], "plain line");
        assert!(out[1].ends_with("[main/ERROR]: Boom ]]> here"), "{}", out[1]);
        assert_eq!(&out[2..], ["java.lang.RuntimeException: x", "\tat a.b(C.java:1)"]);
    }

    #[test]
    fn unfinished_event_is_flushed_raw() {
        let mut d = Log4jDecoder::default();
        assert!(d.feed(r#"<log4j:Event level="INFO">"#).is_empty());
        assert_eq!(d.flush(), [r#"<log4j:Event level="INFO">"#]);
    }
}
