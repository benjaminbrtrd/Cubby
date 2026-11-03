//! Small text helpers: provider descriptions arrive as HTML (CurseForge, Technic) and the UI shows plain text.

/// Plain text of an HTML fragment: tags are dropped, `<script>` / `<style>` content too, block elements
/// become paragraph breaks (blank line), `<br>` a line break, `<li>` a "- " item, and common entities
/// are decoded. Never fails; broken markup just yields whatever text is there.
pub fn html_to_text(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // `<!-- comment -->`
            if lower[i..].starts_with("<!--") {
                i = lower[i + 4..].find("-->").map_or(bytes.len(), |end| i + 4 + end + 3);
                continue;
            }
            let next = bytes.get(i + 1).copied().unwrap_or(b' ');
            if next.is_ascii_alphabetic() || next == b'/' || next == b'!' || next == b'?' {
                let Some(end) = html[i..].find('>').map(|e| i + e) else {
                    break; // unterminated tag: drop the rest
                };
                let tag = &lower[i + 1..end];
                let closing = tag.starts_with('/');
                let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                i = end + 1;
                match name.as_str() {
                    "script" | "style" if !closing => {
                        let close = format!("</{name}");
                        i = lower[i..].find(&close).map_or(bytes.len(), |p| i + p);
                        // The closing tag itself is handled by the next loop turns.
                    }
                    "br" => out.push('\n'),
                    "li" if !closing => out.push_str("\n- "),
                    "li" => {}
                    "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "table" | "tr" | "blockquote" | "pre" | "hr" | "section" | "article" => {
                        // Paragraph breaks at the end of a block, and before headings and rules.
                        if closing || matches!(name.as_str(), "hr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                            out.push_str("\n\n");
                        } else {
                            out.push('\n');
                        }
                    }
                    _ => {}
                }
                continue;
            }
        }
        let run_end = html[i..].char_indices().skip(1).find(|(_, c)| *c == '<').map_or(bytes.len(), |(p, _)| i + p);
        out.push_str(&html[i..run_end]);
        i = run_end;
    }
    normalize(&decode_entities(&out))
}

/// Trims lines, collapses runs of spaces and limits blank lines to one.
fn normalize(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in text.replace('\r', "").split('\n') {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() && lines.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        // An entity is `&` + at most 10 chars + `;`.
        let decoded = tail.char_indices().take(12).find(|(_, c)| *c == ';').and_then(|(semi, _)| decode_entity(&tail[1..semi]).map(|c| (c, semi + 1)));
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &tail[len..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_entity(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code).filter(|c| *c != '\0');
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "ndash" => '\u{2013}',
        "mdash" => '\u{2014}',
        "hellip" => '\u{2026}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "ldquo" => '\u{201C}',
        "rdquo" => '\u{201D}',
        "bull" => '\u{2022}',
        "copy" => '\u{00A9}',
        "reg" => '\u{00AE}',
        "trade" => '\u{2122}',
        "times" => '\u{00D7}',
        "middot" => '\u{00B7}',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_and_breaks() {
        assert_eq!(html_to_text("<p>First  paragraph.</p><p>Second<br>line</p>"), "First paragraph.\n\nSecond\nline");
    }

    #[test]
    fn lists_and_headings() {
        let text = html_to_text("<h2>Features</h2><ul><li>One</li><li>Two &amp; three</li></ul><p>End</p>");
        assert_eq!(text, "Features\n\n- One\n- Two & three\n\nEnd");
    }

    #[test]
    fn drops_scripts_styles_comments_and_unknown_tags() {
        let text = html_to_text("a<script>alert('x < y')</script><style>p{}</style><!-- hidden --><span class=\"x\">b</span><img src=\"i.png\">c");
        assert_eq!(text, "abc");
    }

    #[test]
    fn entities() {
        assert_eq!(html_to_text("Fish &amp; chips &lt;3 &quot;q&quot; &#39;s&#39; &#x41;&nbsp;B &unknown; &"), "Fish & chips <3 \"q\" 's' A B &unknown; &");
        // Escaped markup stays text.
        assert_eq!(html_to_text("&lt;b&gt;bold&lt;/b&gt;"), "<b>bold</b>");
    }

    #[test]
    fn plain_text_and_lone_brackets_pass_through() {
        assert_eq!(html_to_text("1 < 2 and 3 > 2"), "1 < 2 and 3 > 2");
        assert_eq!(html_to_text("Héllo wörld ✓"), "Héllo wörld ✓");
        assert_eq!(html_to_text(""), "");
    }

    #[test]
    fn broken_markup_does_not_panic() {
        assert_eq!(html_to_text("text <b unterminated"), "text");
        assert_eq!(html_to_text("<p>open"), "open");
    }
}
