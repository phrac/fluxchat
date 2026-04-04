//! Convert Matrix HTML message bodies into Iced rich_text Spans.
//!
//! Handles: bold, italic, strikethrough, code, pre/code blocks,
//! links, Matrix mention pills, blockquotes, and plain text.

use std::sync::LazyLock;

use iced::widget::text::Span;
use iced::{Color, Font, font};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::theme;

static SS: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static TS: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// A parsed segment of a Matrix HTML message.
#[derive(Debug)]
enum Segment {
    Text(String, Style),
    Link { text: String, url: String, style: Style },
    CodeBlock { code: String, language: Option<String> },
    LineBreak,
}

/// Accumulated inline style state from nested HTML tags.
#[derive(Debug, Clone, Default)]
struct Style {
    bold: bool,
    italic: bool,
    strikethrough: bool,
    code: bool,
    mention: bool,
    mention_user_id: Option<String>,
    blockquote: bool,
}

/// A part of a rendered message — either text spans or a code block.
pub enum MessagePart {
    /// Normal text (may include inline code, links, mentions, etc.)
    Text(Vec<Span<'static, String>>),
    /// A syntax-highlighted code block with line numbers.
    CodeBlock(Vec<Span<'static, String>>),
}

/// Parse Matrix HTML into a list of Iced Spans.
///
/// Returns a Vec of Spans suitable for `rich_text!` or `Rich::from_iter()`.
/// The `Link` type parameter is `String` (URL for clickable links).
pub fn html_to_spans(html: &str) -> Vec<Span<'static, String>> {
    html_to_spans_with_rooms(html, &[])
}

/// Parse Matrix HTML into Iced Spans, resolving matrix.to room links to pills.
pub fn html_to_spans_with_rooms(html: &str, rooms: &[fluxchat_core::RoomInfo]) -> Vec<Span<'static, String>> {
    let segments = parse_html(html);
    segments_to_spans_with_rooms(segments, rooms)
}

/// Parse Matrix HTML into structured parts (text + code blocks as separate elements).
pub fn html_to_parts(html: &str, rooms: &[fluxchat_core::RoomInfo]) -> Vec<MessagePart> {
    let segments = parse_html(html);
    segments_to_parts(segments, rooms)
}

/// Parse HTML string into intermediate Segments.
fn parse_html(html: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut remaining = html;

    while !remaining.is_empty() {
        if remaining.starts_with('<') {
            // Find end of tag
            let tag_end = remaining.find('>').unwrap_or(remaining.len());
            let tag_content = &remaining[1..tag_end];
            remaining = &remaining[tag_end.saturating_add(1).min(remaining.len())..];

            let is_closing = tag_content.starts_with('/');
            let tag_name = if is_closing {
                tag_content[1..].split_whitespace().next().unwrap_or("")
            } else {
                tag_content.split_whitespace().next().unwrap_or("")
                    .split('/').next().unwrap_or("") // handle self-closing
            };
            let tag_lower = tag_name.to_lowercase();

            match tag_lower.as_str() {
                "br" | "br/" => {
                    segments.push(Segment::LineBreak);
                }
                "b" | "strong" => {
                    if is_closing {
                        pop_style(&mut style_stack);
                    } else {
                        let mut s = current_style(&style_stack).clone();
                        s.bold = true;
                        style_stack.push(s);
                    }
                }
                "i" | "em" => {
                    if is_closing {
                        pop_style(&mut style_stack);
                    } else {
                        let mut s = current_style(&style_stack).clone();
                        s.italic = true;
                        style_stack.push(s);
                    }
                }
                "del" | "s" | "strike" => {
                    if is_closing {
                        pop_style(&mut style_stack);
                    } else {
                        let mut s = current_style(&style_stack).clone();
                        s.strikethrough = true;
                        style_stack.push(s);
                    }
                }
                "code" => {
                    if is_closing {
                        pop_style(&mut style_stack);
                    } else {
                        let mut s = current_style(&style_stack).clone();
                        s.code = true;
                        style_stack.push(s);
                    }
                }
                "pre" => {
                    if is_closing {
                        // Don't pop — we handle pre+code together
                    } else {
                        // Look for <code> inside <pre>, extract until </code></pre>
                        let language = extract_code_language(remaining);
                        // Skip past the inner <code...> tag if present
                        let code_start = if remaining.starts_with("<code") {
                            remaining.find('>').map(|i| i + 1).unwrap_or(0)
                        } else {
                            0
                        };
                        let code_content = &remaining[code_start..];
                        let end_marker = if code_start > 0 {
                            "</code></pre>"
                        } else {
                            "</pre>"
                        };
                        if let Some(end_pos) = code_content.find(end_marker) {
                            let raw_code = &code_content[..end_pos];
                            // Order matters: restore markdown chars from formatting tags,
                            // replace <br> with newlines, strip remaining HTML tags,
                            // THEN unescape entities (so &gt; &lt; aren't eaten by strip_tags)
                            let code = raw_code
                                .replace("<br>", "\n")
                                .replace("<br/>", "\n")
                                .replace("<br />", "\n")
                                .replace("<em>", "*").replace("</em>", "*")
                                .replace("<strong>", "**").replace("</strong>", "**")
                                .replace("<del>", "~~").replace("</del>", "~~");
                            let code = strip_tags(&code);
                            let code = unescape_html(&code);
                            // Collapse double newlines caused by \n<br>\n in source
                            let mut collapsed = String::with_capacity(code.len());
                            let mut prev_newline = false;
                            for ch in code.chars() {
                                if ch == '\n' {
                                    if !prev_newline {
                                        collapsed.push('\n');
                                    }
                                    prev_newline = true;
                                } else {
                                    prev_newline = false;
                                    collapsed.push(ch);
                                }
                            }
                            segments.push(Segment::CodeBlock {
                                code: collapsed.trim_matches('\n').to_string(),
                                language,
                            });
                            remaining = &code_content[end_pos + end_marker.len()..];
                        }
                    }
                }
                "blockquote" => {
                    if is_closing {
                        pop_style(&mut style_stack);
                    } else {
                        let mut s = current_style(&style_stack).clone();
                        s.blockquote = true;
                        style_stack.push(s);
                    }
                }
                "a" => {
                    if is_closing {
                        pop_style(&mut style_stack);
                    } else {
                        // Extract href
                        let href = extract_attr(tag_content, "href");
                        let is_mention = href.as_ref()
                            .is_some_and(|h| h.contains("matrix.to/#/@") || h.contains("matrix.to/#/%40"));
                        let user_id = if is_mention {
                            href.as_ref().and_then(|h| {
                                let idx = h.find("/#/@")
                                    .map(|i| i + 3) // skip "/#/"
                                    .or_else(|| h.find("/#/%40").map(|i| i + 3));
                                idx.map(|i| {
                                    let raw = &h[i..];
                                    let raw = raw.split('?').next().unwrap_or(raw); // strip query
                                    raw.replace("%3A", ":").replace("%40", "@")
                                })
                            })
                        } else {
                            None
                        };

                        let mut s = current_style(&style_stack).clone();
                        s.mention = is_mention;
                        s.mention_user_id = user_id;
                        style_stack.push(s);

                        // For non-mention links, we'll collect text and emit a Link segment
                        // when we hit </a>. For mentions, we style inline.
                        if !is_mention {
                            if let Some(url) = href {
                                // Collect link text
                                let link_text = collect_until_close(remaining, "a");
                                let text = unescape_html(&strip_tags(&link_text.0));
                                remaining = link_text.1;
                                pop_style(&mut style_stack);
                                segments.push(Segment::Link {
                                    text,
                                    url,
                                    style: current_style(&style_stack).clone(),
                                });
                                continue;
                            }
                        }
                    }
                }
                "p" | "div" => {
                    if is_closing {
                        segments.push(Segment::LineBreak);
                    }
                }
                "span" => {
                    if is_closing {
                        pop_style(&mut style_stack);
                    } else {
                        // Just push current style (span is mostly used for mention pills)
                        style_stack.push(current_style(&style_stack).clone());
                    }
                }
                "mx-reply" => {
                    if !is_closing {
                        // Skip the entire mx-reply block (Matrix reply fallback)
                        if let Some(end) = remaining.find("</mx-reply>") {
                            remaining = &remaining[end + "</mx-reply>".len()..];
                        }
                    }
                }
                _ => {
                    // Unknown tag — ignore
                }
            }
        } else {
            // Plain text until next tag
            let text_end = remaining.find('<').unwrap_or(remaining.len());
            let text = &remaining[..text_end];
            if !text.is_empty() {
                let unescaped = unescape_html(text);
                segments.push(Segment::Text(unescaped, current_style(&style_stack).clone()));
            }
            remaining = &remaining[text_end..];
        }
    }

    segments
}

/// Convert Segments into Iced Spans.
fn segments_to_spans(segments: Vec<Segment>) -> Vec<Span<'static, String>> {
    segments_to_spans_with_rooms(segments, &[])
}

fn segments_to_spans_with_rooms(segments: Vec<Segment>, rooms: &[fluxchat_core::RoomInfo]) -> Vec<Span<'static, String>> {
    let mut spans = Vec::new();

    for segment in segments {
        match segment {
            Segment::Text(text, style) => {
                spans.push(styled_span(text, &style));
            }
            Segment::Link { text, url, style } => {
                // Check if this is a matrix.to room link
                if let Some(pill) = try_room_pill(&url, rooms) {
                    spans.push(pill);
                } else {
                    let mut span = styled_span(text, &style);
                    span.color = Some(theme::ACCENT);
                    span.underline = true;
                    span.link = Some(url);
                    spans.push(span);
                }
            }
            Segment::CodeBlock { code, language } => {
                // Newline before code block
                spans.push(Span::new("\n"));

                // Syntax highlight with syntect
                let syntax = language
                    .as_deref()
                    .and_then(|l| SS.find_syntax_by_token(l))
                    .or_else(|| {
                        // Auto-detect language from first line if no tag
                        let first_line = code.lines().next().unwrap_or("");
                        SS.find_syntax_by_first_line(first_line)
                    })
                    .unwrap_or_else(|| SS.find_syntax_plain_text());
                let theme = &TS.themes["base16-ocean.dark"];

                let highlighter = syntect::highlighting::Highlighter::new(theme);
                let mut highlight_state = syntect::highlighting::HighlightState::new(
                    &highlighter,
                    syntect::parsing::ScopeStack::new(),
                );
                let mut parse_state = syntect::parsing::ParseState::new(syntax);

                let code_bg = Color::from_rgba(0.0, 0.0, 0.0, 0.3);
                let line_num_color = Color::from_rgba(1.0, 1.0, 1.0, 0.25);
                let code_size = 13;
                let line_count = code.lines().count();
                let num_width = if line_count >= 100 { 4 } else if line_count >= 10 { 3 } else { 2 };

                for (i, line) in code.lines().enumerate() {
                    // Line number
                    let line_num = format!("{:>width$} ", i + 1, width = num_width);
                    spans.push(
                        Span::new(line_num)
                            .font(Font::MONOSPACE)
                            .size(code_size)
                            .color(line_num_color)
                            .background(code_bg),
                    );

                    let ops = parse_state.parse_line(line, &SS).unwrap_or_default();
                    let regions = syntect::highlighting::HighlightIterator::new(
                        &mut highlight_state,
                        &ops,
                        line,
                        &highlighter,
                    );
                    for (style, text) in regions {
                        let color = Color::from_rgb(
                            style.foreground.r as f32 / 255.0,
                            style.foreground.g as f32 / 255.0,
                            style.foreground.b as f32 / 255.0,
                        );
                        spans.push(
                            Span::new(text.to_string())
                                .font(Font::MONOSPACE)
                                .size(code_size)
                                .color(color)
                                .background(code_bg),
                        );
                    }
                    spans.push(
                        Span::new("\n")
                            .font(Font::MONOSPACE)
                            .size(code_size)
                            .background(code_bg),
                    );
                }
            }
            Segment::LineBreak => {
                // Collapse consecutive line breaks — don't add if the last span is already a newline
                let is_duplicate = spans.last()
                    .is_some_and(|s: &Span<'static, String>| s.text.as_ref() == "\n");
                if !is_duplicate {
                    spans.push(Span::new("\n"));
                }
            }
        }
    }

    spans
}

/// Convert Segments into structured MessageParts, separating code blocks from text.
fn segments_to_parts(segments: Vec<Segment>, rooms: &[fluxchat_core::RoomInfo]) -> Vec<MessagePart> {
    let mut parts = Vec::new();
    let mut current_spans: Vec<Span<'static, String>> = Vec::new();

    for segment in segments {
        match segment {
            Segment::Text(text, style) => {
                current_spans.push(styled_span(text, &style));
            }
            Segment::Link { text, url, style } => {
                if let Some(pill) = try_room_pill(&url, rooms) {
                    current_spans.push(pill);
                } else {
                    let mut span = styled_span(text, &style);
                    span.color = Some(theme::ACCENT);
                    span.underline = true;
                    span.link = Some(url);
                    current_spans.push(span);
                }
            }
            Segment::CodeBlock { code, language } => {
                // Flush any accumulated text spans
                if !current_spans.is_empty() {
                    parts.push(MessagePart::Text(std::mem::take(&mut current_spans)));
                }
                // Build code block spans
                parts.push(MessagePart::CodeBlock(highlight_code_block(&code, language.as_deref())));
            }
            Segment::LineBreak => {
                let is_duplicate = current_spans.last()
                    .is_some_and(|s: &Span<'static, String>| s.text.as_ref() == "\n");
                if !is_duplicate {
                    current_spans.push(Span::new("\n"));
                }
            }
        }
    }

    // Flush remaining text
    if !current_spans.is_empty() {
        parts.push(MessagePart::Text(current_spans));
    }

    parts
}

/// Syntax-highlight a code block into spans with line numbers.
fn highlight_code_block(code: &str, language: Option<&str>) -> Vec<Span<'static, String>> {
    use syntect::easy::HighlightLines;

    let syntax = language
        .and_then(|l| SS.find_syntax_by_token(l))
        .or_else(|| {
            let first_line = code.lines().next().unwrap_or("");
            SS.find_syntax_by_first_line(first_line)
        })
        .unwrap_or_else(|| SS.find_syntax_plain_text());
    let theme = &TS.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);

    let line_num_color = Color::from_rgba(1.0, 1.0, 1.0, 0.25);
    let line_count = code.lines().count();
    let num_width = if line_count >= 100 { 4 } else if line_count >= 10 { 3 } else { 2 };

    let mut spans = Vec::new();
    for (i, line) in code.lines().enumerate() {
        let line_num = format!("{:>width$} ", i + 1, width = num_width);
        spans.push(
            Span::new(line_num)
                .font(Font::MONOSPACE)
                .color(line_num_color),
        );

        // HighlightLines expects lines WITH newline terminators
        let line_with_nl = format!("{line}\n");
        let regions = h.highlight_line(&line_with_nl, &SS).unwrap_or_default();
        for (style, text) in regions {
            // Skip the trailing newline we added
            let text = text.trim_end_matches('\n');
            if text.is_empty() { continue; }
            let color = Color::from_rgb(
                style.foreground.r as f32 / 255.0,
                style.foreground.g as f32 / 255.0,
                style.foreground.b as f32 / 255.0,
            );
            spans.push(
                Span::new(text.to_string())
                    .font(Font::MONOSPACE)
                    .color(color),
            );
        }
        // Add newline except after last line
        if i < line_count - 1 {
            spans.push(Span::new("\n").font(Font::MONOSPACE));
        }
    }

    spans
}


/// Create a styled Span from text and accumulated style.
fn styled_span(text: String, style: &Style) -> Span<'static, String> {
    let mut span = Span::new(text);

    // Font: bold/italic/code combinations
    let weight = if style.bold {
        font::Weight::Bold
    } else {
        font::Weight::Normal
    };
    let font_style = if style.italic {
        font::Style::Italic
    } else {
        font::Style::Normal
    };

    if style.code {
        span.font = Some(Font {
            family: font::Family::Name("JetBrainsMono Nerd Font"),
            weight,
            style: font_style,
            ..Font::default()
        });
        span.color = Some(theme::ACCENT);
        span = span
            .background(Color::from_rgba(0.0, 0.0, 0.0, 0.3))
            .border(iced::Border {
                radius: 3.0.into(),
                ..Default::default()
            });
        span.padding = iced::Padding::from([0, 3]);
    } else if style.bold || style.italic {
        span.font = Some(Font {
            family: font::Family::Name("JetBrainsMono Nerd Font"),
            weight,
            style: font_style,
            ..Font::MONOSPACE
        });
    }

    if style.strikethrough {
        span.strikethrough = true;
    }

    // Mention pill styling
    if style.mention {
        let nick_text = span.text.to_string();
        let color = theme::nick_color(&nick_text);
        // Prefix with @ symbol
        span.text = format!("@{nick_text}").into();
        span.color = Some(color);
        span.font = Some(Font {
            family: font::Family::Name("JetBrainsMono Nerd Font"),
            weight: font::Weight::Bold,
            ..Font::MONOSPACE
        });
        span = span
            .background(Color::from_rgba(color.r, color.g, color.b, 0.15))
            .border(iced::Border { radius: 8.0.into(), ..Default::default() });
        span.padding = iced::Padding::from([2, 6]);
        span.underline = false;
        // Make clickable — OpenUrl handler intercepts matrix.to links
        if let Some(ref uid) = style.mention_user_id {
            let encoded = uid.replace('@', "%40").replace(':', "%3A");
            span.link = Some(format!("https://matrix.to/#/{encoded}"));
        }
    }

    // Blockquote — dim + left border effect via background
    if style.blockquote && !style.code {
        span.color = Some(theme::TEXT_MUTED);
    }

    // Default text color if not set
    if span.color.is_none() {
        span.color = Some(theme::TEXT_NORMAL);
    }

    span
}

/// Try to convert a matrix.to room link into a pill span.
/// Handles both room IDs (`!roomid:server`) and aliases (`#alias:server`).
fn try_room_pill(url: &str, rooms: &[fluxchat_core::RoomInfo]) -> Option<Span<'static, String>> {
    // Extract room ID or alias from matrix.to URL
    let room_id = if let Some(rest) = url.strip_prefix("https://matrix.to/#/") {
        let decoded = rest.replace("%3A", ":").replace("%23", "#");
        // Strip query params (?via=...)
        decoded.split('?').next().unwrap_or(&decoded).to_string()
    } else {
        return None;
    };

    // Must start with ! (room ID) or # (alias)
    if !room_id.starts_with('!') && !room_id.starts_with('#') {
        return None;
    }

    // Look up display name; fall back to alias or shortened room ID
    let display = if let Some(r) = rooms.iter().find(|r| r.room_id == room_id) {
        let name = &r.name;
        format!("#{name}")
    } else if room_id.starts_with('#') {
        room_id.clone()
    } else {
        // Unknown room ID — show shortened form
        let short = room_id.split(':').next().unwrap_or(&room_id);
        format!("{short}:...")
    };

    let color = theme::ACCENT;
    let mut span = Span::new(display);
    span.color = Some(color);
    span.font = Some(Font {
        family: font::Family::Name("JetBrainsMono Nerd Font"),
        weight: font::Weight::Bold,
        ..Font::MONOSPACE
    });
    span = span
        .background(Color::from_rgba(color.r, color.g, color.b, 0.15))
        .border(iced::Border { radius: 8.0.into(), ..Default::default() });
    span.padding = iced::Padding::from([2, 6]);
    span.link = Some(url.to_string());
    Some(span)
}

// --- Helpers ---

fn current_style(stack: &[Style]) -> &Style {
    stack.last().unwrap_or(&Style {
        bold: false,
        italic: false,
        strikethrough: false,
        code: false,
        mention: false,
        mention_user_id: None,
        blockquote: false,
    })
}

fn pop_style(stack: &mut Vec<Style>) {
    if stack.len() > 1 {
        stack.pop();
    }
}

/// Extract an attribute value from a tag string.
/// e.g. extract_attr(r#"a href="https://example.com" class="x""#, "href") -> Some("https://example.com")
fn extract_attr(tag: &str, attr_name: &str) -> Option<String> {
    let pattern = format!("{attr_name}=\"");
    let start = tag.find(&pattern)? + pattern.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Extract the language from a `<code class="language-xxx">` tag.
fn extract_code_language(s: &str) -> Option<String> {
    if s.starts_with("<code class=\"language-") {
        let start = "<code class=\"language-".len();
        let rest = &s[start..];
        rest.find('"').map(|end| rest[..end].to_string())
    } else {
        None
    }
}

/// Collect text content until a closing tag, returning (content, remaining).
fn collect_until_close<'a>(s: &'a str, tag: &str) -> (String, &'a str) {
    let close = format!("</{tag}>");
    if let Some(pos) = s.find(&close) {
        (s[..pos].to_string(), &s[pos + close.len()..])
    } else {
        (s.to_string(), "")
    }
}

/// Strip all HTML tags from a string, leaving only text content.
fn strip_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }
    result
}

/// Convert plain text with basic markdown to Spans.
/// Handles **bold**, *italic*, ~~strike~~, `code`, and ```codeblocks```.
pub fn plain_text_to_spans(input: &str) -> Vec<Span<'static, String>> {
    // Convert basic markdown to HTML, then use the HTML parser
    let mut html = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '`' if chars.peek() == Some(&'`') => {
                chars.next(); // second `
                if chars.peek() == Some(&'`') {
                    chars.next(); // third `
                    // Code block — collect until ```
                    let mut code = String::new();
                    loop {
                        match chars.next() {
                            Some('`') if chars.peek() == Some(&'`') => {
                                chars.next();
                                if chars.peek() == Some(&'`') {
                                    chars.next();
                                    break;
                                }
                                code.push_str("``");
                            }
                            Some(ch) => code.push(ch),
                            None => break,
                        }
                    }
                    html.push_str("<pre><code>");
                    html.push_str(&html_escape(&code));
                    html.push_str("</code></pre>");
                } else {
                    html.push_str("``");
                }
            }
            '`' => {
                let mut code = String::new();
                for ch in chars.by_ref() {
                    if ch == '`' { break; }
                    code.push(ch);
                }
                html.push_str("<code>");
                html.push_str(&html_escape(&code));
                html.push_str("</code>");
            }
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut bold = String::new();
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'*') => { chars.next(); break; }
                        Some(ch) => bold.push(ch),
                        None => break,
                    }
                }
                html.push_str("<b>");
                html.push_str(&html_escape(&bold));
                html.push_str("</b>");
            }
            '*' => {
                let mut italic = String::new();
                for ch in chars.by_ref() {
                    if ch == '*' { break; }
                    italic.push(ch);
                }
                html.push_str("<i>");
                html.push_str(&html_escape(&italic));
                html.push_str("</i>");
            }
            '~' if chars.peek() == Some(&'~') => {
                chars.next();
                let mut strike = String::new();
                loop {
                    match chars.next() {
                        Some('~') if chars.peek() == Some(&'~') => { chars.next(); break; }
                        Some(ch) => strike.push(ch),
                        None => break,
                    }
                }
                html.push_str("<del>");
                html.push_str(&html_escape(&strike));
                html.push_str("</del>");
            }
            '<' => html.push_str("&lt;"),
            '>' => html.push_str("&gt;"),
            '&' => html.push_str("&amp;"),
            '\n' => html.push_str("<br>"),
            'h' => {
                // Detect http:// or https:// URLs
                let mut url = String::from('h');
                let prefix_rest: String = chars.clone().take(7).collect();
                if prefix_rest.starts_with("ttps://") || prefix_rest.starts_with("ttp://") {
                    // Consume the URL
                    for ch in chars.by_ref() {
                        if ch.is_whitespace() || ch == '<' || ch == '>' {
                            break;
                        }
                        url.push(ch);
                    }
                    // Trim trailing punctuation
                    let trimmed = url.trim_end_matches(|c: char| c == '.' || c == ',' || c == ')' || c == ']' || c == ';');
                    let trailing = &url[trimmed.len()..];
                    let escaped = html_escape(trimmed);
                    html.push_str(&format!("<a href=\"{escaped}\">{escaped}</a>"));
                    html.push_str(&html_escape(trailing));
                } else {
                    html.push('h');
                }
            }
            _ => html.push(c),
        }
    }

    html_to_spans(&html)
}

/// Escape special HTML characters in user content.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Unescape common HTML entities.
fn unescape_html(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
