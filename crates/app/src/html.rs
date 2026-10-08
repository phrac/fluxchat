//! Convert Matrix HTML message bodies into Iced rich_text Spans.
//!
//! Handles: bold, italic, strikethrough, code, pre/code blocks,
//! links, Matrix mention pills, blockquotes, and plain text.
//!
//! Uses html5ever for proper HTML parsing with a minimal arena-based DOM.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::sync::LazyLock;

use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{Attribute, ExpandedName, QualName};
use iced::widget::text::Span;
use iced::{Color, Font, font};
use markup5ever::{local_name, ns};
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
    Table(TableData),
}

/// Parsed table: headers (one row, may be empty) and body rows. Each cell holds
/// its own list of inline Segments so styling/links/mentions render normally.
#[derive(Debug)]
struct TableData {
    headers: Vec<Vec<Segment>>,
    rows: Vec<Vec<Vec<Segment>>>,
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
    /// 0 = none, 1-6 = h1-h6
    heading_level: u8,
}

/// A part of a rendered message — either text spans, a code block, or a table.
#[derive(Clone)]
pub enum MessagePart {
    /// Normal text (may include inline code, links, mentions, etc.)
    Text(Vec<Span<'static, String>>),
    /// A syntax-highlighted code block with line numbers.
    CodeBlock(Vec<Span<'static, String>>),
    /// A table: optional header row + body rows. Each cell is its own span list.
    Table {
        headers: Vec<Vec<Span<'static, String>>>,
        rows: Vec<Vec<Vec<Span<'static, String>>>>,
    },
}

/// A message body converted to spans, ready to hand to widgets.
pub enum RenderedBody {
    Spans(Vec<Span<'static, String>>),
    Parts(Vec<MessagePart>),
}

/// Max parsed bodies kept before the cache is reset.
const BODY_CACHE_LIMIT: usize = 4000;

/// Memoizes message body parsing. `view()` runs after every update, and
/// re-parsing HTML with html5ever + syntax-highlighting every visible message
/// each time is the dominant render cost. Entries are keyed by the source text
/// and every input that affects the output; theme or room-list changes must
/// call `clear()` since spans bake in colors and room names.
#[derive(Default)]
pub struct BodyCache {
    map: RefCell<std::collections::HashMap<u64, std::rc::Rc<RenderedBody>>>,
}

impl BodyCache {
    pub fn get_or_insert(
        &self,
        key: impl std::hash::Hash,
        build: impl FnOnce() -> RenderedBody,
    ) -> std::rc::Rc<RenderedBody> {
        use std::hash::{BuildHasher, BuildHasherDefault};
        let hash = BuildHasherDefault::<std::collections::hash_map::DefaultHasher>::default().hash_one(key);
        if let Some(hit) = self.map.borrow().get(&hash) {
            return hit.clone();
        }
        let body = std::rc::Rc::new(build());
        let mut map = self.map.borrow_mut();
        if map.len() >= BODY_CACHE_LIMIT {
            map.clear();
        }
        map.insert(hash, body.clone());
        body
    }

    pub fn clear(&self) {
        self.map.borrow_mut().clear();
    }
}

/// Parse Matrix HTML into a list of Iced Spans.
///
/// Returns a Vec of Spans suitable for `rich_text!` or `Rich::from_iter()`.
/// The `Link` type parameter is `String` (URL for clickable links).
pub fn html_to_spans(html: &str) -> Vec<Span<'static, String>> {
    html_to_spans_with_rooms(html, &[], 14.0)
}

/// Parse Matrix HTML into Iced Spans, resolving matrix.to room links to pills.
/// `base_size` is the parent text size in pixels — headings size relative to it.
pub fn html_to_spans_with_rooms(html: &str, rooms: &[fluxchat_core::RoomInfo], base_size: f32) -> Vec<Span<'static, String>> {
    let segments = parse_html(html);
    segments_to_spans_with_rooms(segments, rooms, base_size)
}

/// Parse Matrix HTML into structured parts (text + code blocks as separate elements).
pub fn html_to_parts(html: &str, rooms: &[fluxchat_core::RoomInfo], base_size: f32) -> Vec<MessagePart> {
    let segments = parse_html(html);
    segments_to_parts(segments, rooms, base_size)
}

// ---------------------------------------------------------------------------
// Minimal arena-based DOM for html5ever TreeSink
// ---------------------------------------------------------------------------

type NodeId = usize;

#[derive(Debug)]
enum NodeData {
    Document,
    Text(String),
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
    },
    Comment,
}

#[derive(Debug)]
struct Node {
    data: NodeData,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

struct Arena {
    nodes: RefCell<Vec<Node>>,
    document: NodeId,
    errors: RefCell<Vec<String>>,
    quirks_mode: Cell<QuirksMode>,
}

impl Arena {
    fn new() -> Self {
        let doc = Node { data: NodeData::Document, parent: None, children: Vec::new() };
        Arena {
            nodes: RefCell::new(vec![doc]),
            document: 0,
            errors: RefCell::new(Vec::new()),
            quirks_mode: Cell::new(QuirksMode::NoQuirks),
        }
    }

    fn new_node(&self, data: NodeData) -> NodeId {
        let mut nodes = self.nodes.borrow_mut();
        let id = nodes.len();
        nodes.push(Node { data, parent: None, children: Vec::new() });
        id
    }

    fn append_child(&self, parent: NodeId, child: NodeId) {
        let mut nodes = self.nodes.borrow_mut();
        nodes[child].parent = Some(parent);
        nodes[parent].children.push(child);
    }

    fn detach(&self, target: NodeId) {
        let mut nodes = self.nodes.borrow_mut();
        if let Some(parent_id) = nodes[target].parent {
            nodes[parent_id].children.retain(|&c| c != target);
            nodes[target].parent = None;
        }
    }
}

impl TreeSink for &Arena {
    type Handle = NodeId;
    type Output = ();
    type ElemName<'a> = ExpandedName<'a> where Self: 'a;

    fn finish(self) {}

    fn parse_error(&self, msg: Cow<'static, str>) {
        self.errors.borrow_mut().push(msg.into_owned());
    }

    fn get_document(&self) -> NodeId {
        self.document
    }

    fn elem_name(&self, target: &NodeId) -> ExpandedName<'_> {
        let nodes = self.nodes.borrow();
        match &nodes[*target].data {
            NodeData::Element { name, .. } => {
                // SAFETY: Atom values (Namespace/LocalName) are interned in a global
                // table, so the references in ExpandedName remain valid beyond the
                // RefCell borrow guard's lifetime.
                let expanded = name.expanded();
                unsafe { std::mem::transmute::<ExpandedName<'_>, ExpandedName<'_>>(expanded) }
            }
            _ => panic!("elem_name called on non-element"),
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, _flags: ElementFlags) -> NodeId {
        self.new_node(NodeData::Element { name, attrs })
    }

    fn create_comment(&self, _text: html5ever::tendril::StrTendril) -> NodeId {
        self.new_node(NodeData::Comment)
    }

    fn create_pi(&self, _target: html5ever::tendril::StrTendril, _data: html5ever::tendril::StrTendril) -> NodeId {
        self.new_node(NodeData::Comment)
    }

    fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
        match child {
            NodeOrText::AppendNode(id) => self.append_child(*parent, id),
            NodeOrText::AppendText(text) => {
                // Merge with previous text node if possible
                let nodes = self.nodes.borrow();
                if let Some(&last_child_id) = nodes[*parent].children.last() {
                    if let NodeData::Text(ref _existing) = nodes[last_child_id].data {
                        drop(nodes);
                        let mut nodes = self.nodes.borrow_mut();
                        if let NodeData::Text(ref mut existing) = nodes[last_child_id].data {
                            existing.push_str(&text);
                        }
                        return;
                    }
                }
                drop(nodes);
                let id = self.new_node(NodeData::Text(text.to_string()));
                self.append_child(*parent, id);
            }
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &NodeId,
        prev_element: &NodeId,
        child: NodeOrText<NodeId>,
    ) {
        let has_parent = self.nodes.borrow()[*element].parent.is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: html5ever::tendril::StrTendril,
        _public_id: html5ever::tendril::StrTendril,
        _system_id: html5ever::tendril::StrTendril,
    ) {}

    fn get_template_contents(&self, target: &NodeId) -> NodeId {
        *target
    }

    fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
        *x == *y
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.quirks_mode.set(mode);
    }

    fn append_before_sibling(&self, sibling: &NodeId, new_node: NodeOrText<NodeId>) {
        let parent_id = self.nodes.borrow()[*sibling].parent.unwrap_or(self.document);
        match new_node {
            NodeOrText::AppendNode(id) => {
                self.detach(id);
                let mut nodes = self.nodes.borrow_mut();
                nodes[id].parent = Some(parent_id);
                let children = &mut nodes[parent_id].children;
                let pos = children.iter().position(|&c| c == *sibling).unwrap_or(children.len());
                children.insert(pos, id);
            }
            NodeOrText::AppendText(text) => {
                let id = self.new_node(NodeData::Text(text.to_string()));
                let mut nodes = self.nodes.borrow_mut();
                nodes[id].parent = Some(parent_id);
                let children = &mut nodes[parent_id].children;
                let pos = children.iter().position(|&c| c == *sibling).unwrap_or(children.len());
                children.insert(pos, id);
            }
        }
    }

    fn add_attrs_if_missing(&self, target: &NodeId, new_attrs: Vec<Attribute>) {
        let mut nodes = self.nodes.borrow_mut();
        if let NodeData::Element { ref mut attrs, .. } = nodes[*target].data {
            for attr in new_attrs {
                if !attrs.iter().any(|a| a.name == attr.name) {
                    attrs.push(attr);
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &NodeId) {
        self.detach(*target);
    }

    fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
        let children: Vec<NodeId> = {
            let mut nodes = self.nodes.borrow_mut();
            let children = std::mem::take(&mut nodes[*node].children);
            for &child_id in &children {
                nodes[child_id].parent = Some(*new_parent);
            }
            children
        };
        let mut nodes = self.nodes.borrow_mut();
        nodes[*new_parent].children.extend(children);
    }
}

// ---------------------------------------------------------------------------
// DOM tree walker -> Segments
// ---------------------------------------------------------------------------

/// Context passed through the tree walk.
/// Deepest element nesting rendered with formatting. Messages come from
/// anyone in the room and the walk recurses, so beyond this the rest of the
/// subtree is shown as plain text (a 64 KB event can nest ~20 000 tags,
/// which overflowed the main thread's stack). Real messages stay far below.
const MAX_HTML_DEPTH: usize = 64;

struct WalkCtx<'a> {
    nodes: &'a [Node],
    segments: Vec<Segment>,
    depth: usize,
}

impl<'a> WalkCtx<'a> {
    /// Check if a node is an element with the given local name.
    fn is_element(&self, id: NodeId, name: &str) -> bool {
        match &self.nodes[id].data {
            NodeData::Element { name: qn, .. } => &*qn.local == name,
            _ => false,
        }
    }

    /// Get attribute value from an element node.
    fn get_attr(&self, id: NodeId, attr_name: &str) -> Option<String> {
        match &self.nodes[id].data {
            NodeData::Element { attrs, .. } => {
                attrs.iter().find(|a| &*a.name.local == attr_name).map(|a| a.value.to_string())
            }
            _ => None,
        }
    }


    /// Collect all text content from a subtree (stripping tags).
    fn collect_text(&self, id: NodeId) -> String {
        let mut out = String::new();
        self.collect_text_inner(id, &mut out);
        out
    }

    /// Iterative (explicit stack): used on arbitrarily deep subtrees.
    fn collect_text_inner(&self, id: NodeId, out: &mut String) {
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            match &self.nodes[id].data {
                NodeData::Text(t) => out.push_str(t),
                _ => stack.extend(self.nodes[id].children.iter().rev().copied()),
            }
        }
    }


    /// Check if this node is inside a `<li>` (for suppressing p/div line breaks).
    fn inside_li(&self, id: NodeId) -> bool {
        let mut cur = self.nodes[id].parent;
        while let Some(pid) = cur {
            if self.is_element(pid, "li") {
                return true;
            }
            cur = self.nodes[pid].parent;
        }
        false
    }

    /// Walk a node and its children, accumulating Segments. Past
    /// `MAX_HTML_DEPTH` the subtree is flattened to its text.
    fn walk(&mut self, id: NodeId, style: &Style) {
        if self.depth >= MAX_HTML_DEPTH {
            let text = self.collect_text(id);
            if !text.trim().is_empty() {
                self.segments.push(Segment::Text(text, style.clone()));
            }
            return;
        }
        self.depth += 1;
        self.walk_node(id, style);
        self.depth -= 1;
    }

    fn walk_node(&mut self, id: NodeId, style: &Style) {
        match &self.nodes[id].data {
            NodeData::Document => {
                let children: Vec<NodeId> = self.nodes[id].children.clone();
                for child in children {
                    self.walk(child, style);
                }
            }
            NodeData::Text(text) => {
                if text.is_empty() {
                    return;
                }
                // Skip whitespace-only text nodes between block elements
                if text.trim().is_empty() {
                    return;
                }
                let text = convert_inline_latex(text);
                if style.code || style.mention {
                    self.segments.push(Segment::Text(text, style.clone()));
                    return;
                }
                // Bare URLs in HTML bodies (agents send formatted bodies
                // without <a> tags) become links, like in plain bodies
                let mut rest = text.as_str();
                for (range, url) in find_urls(&text) {
                    let before = &text[text.len() - rest.len()..range.start];
                    if !before.is_empty() {
                        self.segments.push(Segment::Text(before.to_string(), style.clone()));
                    }
                    self.segments.push(Segment::Link { text: text[range.clone()].to_string(), url, style: style.clone() });
                    rest = &text[range.end..];
                }
                if !rest.is_empty() {
                    self.segments.push(Segment::Text(rest.to_string(), style.clone()));
                }
            }
            NodeData::Comment => {}
            NodeData::Element { name, attrs } => {
                let tag = &*name.local;
                match tag {
                    // Skip mx-reply blocks entirely
                    "mx-reply" => return,

                    "br" => {
                        self.segments.push(Segment::LineBreak);
                        return;
                    }

                    // Headings: render as bold + sized text + line break
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let level = tag.as_bytes()[1] - b'0';
                        // Headings start on their own line
                        self.segments.push(Segment::LineBreak);
                        let mut s = style.clone();
                        s.bold = true;
                        s.heading_level = level;
                        let children: Vec<NodeId> = self.nodes[id].children.clone();
                        for child in children {
                            self.walk(child, &s);
                        }
                        self.segments.push(Segment::LineBreak);
                        return;
                    }

                    // Pre/code blocks
                    "pre" => {
                        self.emit_code_block(id);
                        return;
                    }

                    // Tables
                    "table" => {
                        let table = self.walk_table(id, style);
                        self.segments.push(Segment::LineBreak);
                        self.segments.push(Segment::Table(table));
                        self.segments.push(Segment::LineBreak);
                        return;
                    }

                    // Anchor tags
                    "a" => {
                        let href = attrs.iter()
                            .find(|a| &*a.name.local == "href")
                            .map(|a| a.value.to_string());
                        let is_mention = href.as_ref()
                            .is_some_and(|h| h.contains("matrix.to/#/@") || h.contains("matrix.to/#/%40"));

                        if is_mention {
                            let user_id = href.as_ref().and_then(|h| {
                                let idx = h.find("/#/@")
                                    .map(|i| i + 3)
                                    .or_else(|| h.find("/#/%40").map(|i| i + 3));
                                idx.map(|i| {
                                    let raw = &h[i..];
                                    let raw = raw.split('?').next().unwrap_or(raw);
                                    raw.replace("%3A", ":").replace("%40", "@")
                                })
                            });
                            let mut s = style.clone();
                            s.mention = true;
                            s.mention_user_id = user_id;
                            let children: Vec<NodeId> = self.nodes[id].children.clone();
                            for child in children {
                                self.walk(child, &s);
                            }
                        } else if let Some(url) = href {
                            let text = self.collect_text(id);
                            self.segments.push(Segment::Link {
                                text,
                                url,
                                style: style.clone(),
                            });
                        } else {
                            // No href — just render children normally
                            let children: Vec<NodeId> = self.nodes[id].children.clone();
                            for child in children {
                                self.walk(child, style);
                            }
                        }
                        return;
                    }

                    _ => {}
                }

                // Compute updated style for inline elements
                let mut s = style.clone();
                match tag {
                    "b" | "strong" => s.bold = true,
                    "i" | "em" => s.italic = true,
                    "del" | "s" | "strike" => s.strikethrough = true,
                    "code" => s.code = true,
                    "blockquote" => s.blockquote = true,
                    _ => {}
                }

                let children: Vec<NodeId> = self.nodes[id].children.clone();

                match tag {
                    "p" | "div" => {
                        // Inside <li>, don't add extra line breaks for <p>
                        if self.inside_li(id) {
                            for child in children {
                                self.walk(child, &s);
                            }
                        } else {
                            for child in children {
                                self.walk(child, &s);
                            }
                            self.segments.push(Segment::LineBreak);
                            self.segments.push(Segment::LineBreak);
                        }
                    }
                    "ol" | "ul" => {
                        let mut li_count = 0u32;
                        for child in children.iter() {
                            if self.is_element(*child, "li") {
                                li_count += 1;
                                if tag == "ol" {
                                    // Numbered list
                                    let num = format!("{li_count}. ");
                                    self.segments.push(Segment::Text(num, s.clone()));
                                } else {
                                    // Bullet list
                                    self.segments.push(Segment::Text("\u{2022} ".to_string(), s.clone()));
                                }
                                // Walk li children
                                let li_children: Vec<NodeId> = self.nodes[*child].children.clone();
                                for li_child in li_children {
                                    self.walk(li_child, &s);
                                }
                                self.segments.push(Segment::LineBreak);
                            } else {
                                self.walk(*child, &s);
                            }
                        }
                    }
                    "li" => {
                        // li handled by parent ol/ul — but if we get here directly, just walk children
                        for child in children {
                            self.walk(child, &s);
                        }
                    }
                    "blockquote" => {
                        for child in children {
                            self.walk(child, &s);
                        }
                    }
                    _ => {
                        for child in children {
                            self.walk(child, &s);
                        }
                    }
                }
            }
        }
    }

    /// Recursively collect all <tr> descendants of a node.
    fn collect_trs(&self, id: NodeId, out: &mut Vec<NodeId>) {
        if self.is_element(id, "tr") {
            out.push(id);
            return;
        }
        for &child in &self.nodes[id].children {
            if matches!(self.nodes[child].data, NodeData::Element { .. }) {
                self.collect_trs(child, out);
            }
        }
    }

    /// Walk a <table> element and return a TableData. Cells are walked into
    /// their own segment buffers via temporary swap of `self.segments`.
    fn walk_table(&mut self, table_id: NodeId, style: &Style) -> TableData {
        let mut tr_ids = Vec::new();
        self.collect_trs(table_id, &mut tr_ids);

        let mut headers: Vec<Vec<Segment>> = Vec::new();
        let mut rows: Vec<Vec<Vec<Segment>>> = Vec::new();

        for tr_id in tr_ids {
            let cell_ids: Vec<NodeId> = self.nodes[tr_id]
                .children
                .iter()
                .copied()
                .filter(|&c| self.is_element(c, "td") || self.is_element(c, "th"))
                .collect();
            if cell_ids.is_empty() {
                continue;
            }
            let mut is_header_row = false;
            let mut cells: Vec<Vec<Segment>> = Vec::with_capacity(cell_ids.len());
            for cell_id in cell_ids {
                if self.is_element(cell_id, "th") {
                    is_header_row = true;
                }
                // Walk cell into an isolated segment buffer
                let saved = std::mem::take(&mut self.segments);
                let children: Vec<NodeId> = self.nodes[cell_id].children.clone();
                let mut cell_style = style.clone();
                if self.is_element(cell_id, "th") {
                    cell_style.bold = true;
                }
                for child in children {
                    self.walk(child, &cell_style);
                }
                let cell_segs = std::mem::replace(&mut self.segments, saved);
                cells.push(cell_segs);
            }
            if is_header_row && headers.is_empty() {
                headers = cells;
            } else {
                rows.push(cells);
            }
        }

        TableData { headers, rows }
    }

    /// Extract and emit a code block from a <pre> element.
    fn emit_code_block(&mut self, pre_id: NodeId) {
        // Look for a <code> child to extract language tag
        let children: Vec<NodeId> = self.nodes[pre_id].children.clone();

        let (language, code_root) = if let Some(&code_id) = children.iter().find(|&&c| self.is_element(c, "code")) {
            let lang = self.get_attr(code_id, "class")
                .and_then(|cls| {
                    cls.split_whitespace()
                        .find(|c| c.starts_with("language-"))
                        .map(|c| c.strip_prefix("language-").unwrap_or(c).to_string())
                });
            (lang, code_id)
        } else {
            (None, pre_id)
        };

        // Collect all text from the code subtree
        let code = self.collect_code_text(code_root);

        // Collapse consecutive newlines
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

        self.segments.push(Segment::CodeBlock {
            code: collapsed.trim_matches('\n').to_string(),
            language,
        });
    }

    /// Collect text from a code block, converting <br> to newlines.
    fn collect_code_text(&self, id: NodeId) -> String {
        let mut out = String::new();
        self.collect_code_text_inner(id, &mut out);
        out
    }

    /// Iterative (explicit stack): code blocks can hold arbitrarily deep markup.
    fn collect_code_text_inner(&self, id: NodeId, out: &mut String) {
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            match &self.nodes[id].data {
                NodeData::Text(t) => out.push_str(t),
                NodeData::Element { name, .. } if &*name.local == "br" => out.push('\n'),
                _ => stack.extend(self.nodes[id].children.iter().rev().copied()),
            }
        }
    }
}

/// Tag nesting above which a message isn't parsed as HTML at all. The HTML5
/// tree builder checks element scope by scanning the open-element stack, so
/// parse time grows with the square of the nesting: 20 000 nested list items
/// took seconds on the UI thread. Real messages nest a few levels.
const MAX_PARSE_NESTING: usize = 512;

/// Rough maximum nesting of `html` (opening tags minus closing tags, void
/// and self-closed tags ignored). Only used to reject pathological input.
fn approx_nesting(html: &str) -> usize {
    const VOID: &[&str] = &["br", "img", "hr", "input", "meta", "link", "wbr", "col", "area", "base", "source"];
    let (mut depth, mut max) = (0usize, 0usize);
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        rest = &rest[i + 1..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        if let Some(_closing) = tag.strip_prefix('/') {
            depth = depth.saturating_sub(1);
        } else if !tag.ends_with('/') && !tag.starts_with('!') {
            let name: String = tag.chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
            if !name.is_empty() && !VOID.contains(&name.as_str()) {
                depth += 1;
                max = max.max(depth);
            }
        }
        rest = &rest[end.min(rest.len())..];
    }
    max
}

/// Parse HTML string into intermediate Segments using html5ever.
fn parse_html(html: &str) -> Vec<Segment> {
    if approx_nesting(html) > MAX_PARSE_NESTING {
        // Pathological markup: show its text instead of parsing it.
        let mut text = String::with_capacity(html.len());
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                c if !in_tag => text.push(c),
                _ => {}
            }
        }
        return vec![Segment::Text(text, Style::default())];
    }
    let arena = Arena::new();

    // Parse as an HTML fragment (body context)
    let context_name = QualName::new(None, ns!(html), local_name!("body"));
    let context_attrs = vec![];
    html5ever::parse_fragment(
        &arena,
        html5ever::ParseOpts::default(),
        context_name,
        context_attrs,
        false,
    )
    .one(html);

    // Walk the DOM tree to produce Segments
    let nodes = arena.nodes.into_inner();
    let mut ctx = WalkCtx {
        nodes: &nodes,
        segments: Vec::new(),
        depth: 0,
    };

    let doc_children: Vec<NodeId> = nodes[arena.document].children.clone();
    let style = Style::default();
    for child in doc_children {
        ctx.walk(child, &style);
    }

    ctx.segments
}

fn segments_to_spans_with_rooms(segments: Vec<Segment>, rooms: &[fluxchat_core::RoomInfo], base_size: f32) -> Vec<Span<'static, String>> {
    let mut spans = Vec::new();

    for segment in segments {
        match segment {
            Segment::Text(text, style) => {
                spans.push(styled_span(text, &style, base_size));
            }
            Segment::Link { text, url, style } => {
                // Check if this is a matrix.to room link
                if let Some(pill) = try_room_pill(&url, rooms) {
                    spans.push(pill);
                } else {
                    let mut span = styled_span(text, &style, base_size);
                    span.color = Some(theme::colors().accent);
                    span.underline = true;
                    span.link = Some(url);
                    spans.push(span);
                }
            }
            Segment::CodeBlock { code, language } => {
                // Newline before code block
                spans.push(Span::new("\n"));

                // Syntax highlight with syntect
                let syntax = syntax_for(&code, language.as_deref());
                let theme = &TS.themes["base16-ocean.dark"];

                let highlighter = syntect::highlighting::Highlighter::new(theme);
                let mut highlight_state = syntect::highlighting::HighlightState::new(
                    &highlighter,
                    syntect::parsing::ScopeStack::new(),
                );
                let mut parse_state = syntect::parsing::ParseState::new(syntax);

                let is_dark = theme::colors().dark;
                let code_bg = theme::colors().code_bg;
                let line_num_color = if is_dark {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.25)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.3)
                };
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
                        let color = if is_dark {
                            Color::from_rgb(
                                style.foreground.r as f32 / 255.0,
                                style.foreground.g as f32 / 255.0,
                                style.foreground.b as f32 / 255.0,
                            )
                        } else {
                            theme::colors().code_text
                        };
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
            Segment::Table(td) => {
                // Inline fallback: render as " | "-separated rows. The parts
                // pipeline (used when <table> is present) renders a real grid.
                if !spans.is_empty() {
                    spans.push(Span::new("\n"));
                }
                let header_cells: Vec<String> = td.headers.iter()
                    .map(|c| collect_segment_text(c)).collect();
                if !header_cells.is_empty() {
                    spans.push(styled_span(header_cells.join(" | "), &Style { bold: true, ..Style::default() }, base_size));
                    spans.push(Span::new("\n"));
                }
                for row in &td.rows {
                    let cells: Vec<String> = row.iter().map(|c| collect_segment_text(c)).collect();
                    spans.push(styled_span(cells.join(" | "), &Style::default(), base_size));
                    spans.push(Span::new("\n"));
                }
            }
            Segment::LineBreak => {
                // Allow up to 2 consecutive newlines (paragraph spacing)
                let trailing_newlines = spans.iter().rev()
                    .take_while(|s: &&Span<'static, String>| {
                        let t = s.text.as_ref();
                        t == "\n" || t == "\n\n"
                    })
                    .map(|s| s.text.as_ref().matches('\n').count())
                    .sum::<usize>();
                if trailing_newlines < 2 {
                    spans.push(Span::new("\n"));
                }
            }
        }
    }

    spans
}

/// Convert Segments into structured MessageParts, separating code blocks from text.
fn segments_to_parts(segments: Vec<Segment>, rooms: &[fluxchat_core::RoomInfo], base_size: f32) -> Vec<MessagePart> {
    let mut parts = Vec::new();
    let mut current_spans: Vec<Span<'static, String>> = Vec::new();

    for segment in segments {
        match segment {
            Segment::Text(text, style) => {
                current_spans.push(styled_span(text, &style, base_size));
            }
            Segment::Link { text, url, style } => {
                if let Some(pill) = try_room_pill(&url, rooms) {
                    current_spans.push(pill);
                } else {
                    let mut span = styled_span(text, &style, base_size);
                    span.color = Some(theme::colors().accent);
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
            Segment::Table(td) => {
                if !current_spans.is_empty() {
                    parts.push(MessagePart::Text(std::mem::take(&mut current_spans)));
                }
                let headers: Vec<Vec<Span<'static, String>>> = td.headers.into_iter()
                    .map(|cell| segments_to_spans_with_rooms(cell, rooms, base_size))
                    .collect();
                let rows: Vec<Vec<Vec<Span<'static, String>>>> = td.rows.into_iter()
                    .map(|row| row.into_iter()
                        .map(|cell| segments_to_spans_with_rooms(cell, rooms, base_size))
                        .collect())
                    .collect();
                parts.push(MessagePart::Table { headers, rows });
            }
            Segment::LineBreak => {
                let trailing_newlines = current_spans.iter().rev()
                    .take_while(|s: &&Span<'static, String>| s.text.as_ref() == "\n")
                    .count();
                if trailing_newlines < 2 {
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
/// The syntax for a code block: its language tag, else a unified diff
/// (agents post `git diff` output untagged), else a guess from the first
/// line, else plain text.
fn syntax_for(code: &str, language: Option<&str>) -> &'static syntect::parsing::SyntaxReference {
    language
        .and_then(|l| SS.find_syntax_by_token(l))
        .or_else(|| looks_like_diff(code).then(|| SS.find_syntax_by_extension("diff")).flatten())
        .or_else(|| SS.find_syntax_by_first_line(code.lines().next().unwrap_or("")))
        .unwrap_or_else(|| SS.find_syntax_plain_text())
}

/// A unified diff: a `diff --git` / `@@` hunk header, or `---` and `+++` file lines.
pub(crate) fn looks_like_diff(code: &str) -> bool {
    let has = |prefix: &str| code.lines().any(|l| l.starts_with(prefix));
    has("diff --git ") || has("@@ -") || (has("--- ") && has("+++ "))
}

fn highlight_code_block(code: &str, language: Option<&str>) -> Vec<Span<'static, String>> {
    use syntect::easy::HighlightLines;

    let is_dark = theme::colors().dark;
    let code_text = theme::colors().code_text;

    let syntax = syntax_for(code, language);
    let theme = &TS.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);

    let line_num_color = if is_dark {
        Color::from_rgba(1.0, 1.0, 1.0, 0.25)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.3)
    };
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
            let color = if is_dark {
                Color::from_rgb(
                    style.foreground.r as f32 / 255.0,
                    style.foreground.g as f32 / 255.0,
                    style.foreground.b as f32 / 255.0,
                )
            } else {
                code_text
            };
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


/// Flatten a segment list to plain text (used by the inline table fallback).
fn collect_segment_text(segments: &[Segment]) -> String {
    let mut out = String::new();
    for s in segments {
        match s {
            Segment::Text(t, _) => out.push_str(t),
            Segment::Link { text, .. } => out.push_str(text),
            Segment::CodeBlock { code, .. } => out.push_str(code),
            Segment::LineBreak => out.push(' '),
            Segment::Table(_) => {}
        }
    }
    out
}

/// Create a styled Span from text and accumulated style.
fn styled_span(text: String, style: &Style, base_size: f32) -> Span<'static, String> {
    let mut span = Span::new(text);

    // Heading size: scale relative to base
    if style.heading_level > 0 {
        let scale = match style.heading_level {
            1 => 1.7,
            2 => 1.45,
            3 => 1.25,
            4 => 1.15,
            5 => 1.05,
            _ => 1.0,
        };
        span = span.size(base_size * scale);
    }

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
        span.color = Some(theme::colors().accent);
        span = span
            .background(theme::colors().code_bg)
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
        span.color = Some(theme::colors().text_muted);
    }

    // Default text color if not set
    if span.color.is_none() {
        span.color = Some(theme::colors().text_normal);
    }

    span
}

/// Try to convert a matrix.to room link into a pill span.
/// Handles both room IDs (`!roomid:server`) and aliases (`#alias:server`).
fn try_room_pill(url: &str, rooms: &[fluxchat_core::RoomInfo]) -> Option<Span<'static, String>> {
    // Extract room ID or alias from matrix.to URL, dropping query params (?via=...)
    let rest = url.strip_prefix("https://matrix.to/#/")?;
    let decoded = rest.replace("%3A", ":").replace("%23", "#");
    let room_id = decoded.split('?').next().unwrap_or(&decoded).to_string();

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

    let color = theme::colors().accent;
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

/// Convert plain text with basic markdown to Spans.
/// Handles **bold**, *italic*, ~~strike~~, `code`, and ```codeblocks```.
pub fn plain_text_to_spans(input: &str) -> Vec<Span<'static, String>> {
    // Convert inline LaTeX before markdown processing
    let input = convert_inline_latex(input);
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
                    let trimmed = url.trim_end_matches(['.', ',', ')', ']', ';']);
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

/// Bare URLs in text: `http(s)://…`, `www.…`, and `host.tld/path` (a path
/// is required without a scheme, so file names like `main.rs` stay text).
/// Returns each match's byte range and the URL to open (scheme added).
fn find_urls(text: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let mut found = Vec::new();
    let mut start = 0;
    for token in text.split_inclusive(char::is_whitespace) {
        let word = token.trim_end_matches(char::is_whitespace);
        let at = start;
        start += token.len();
        // Leading punctuation: "(https://…" or "\"www.…"
        let lead = word.len() - word.trim_start_matches(['(', '[', '"', '\'', '<']).len();
        let candidate = word[lead..].trim_end_matches(['.', ',', ')', ']', ';', ':', '!', '?', '"', '\'', '>']);
        if candidate.is_empty() {
            continue;
        }
        let lower = candidate.to_ascii_lowercase();
        let url = if lower.starts_with("https://") || lower.starts_with("http://") {
            if candidate.len() <= lower.find("//").unwrap_or(0) + 2 {
                continue;
            }
            candidate.to_string()
        } else if (lower.starts_with("www.") && candidate.len() > 4) || looks_like_host_path(candidate) {
            format!("https://{candidate}")
        } else {
            continue;
        };
        let from = at + lead;
        found.push((from..from + candidate.len(), url));
    }
    found
}

/// `ebay.com/itm/123`: dot-separated host labels, alphabetic TLD, then a path.
fn looks_like_host_path(s: &str) -> bool {
    let Some((host, path)) = s.split_once('/') else { return false };
    if path.is_empty() {
        return false;
    }
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && labels.last().is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
}

/// Escape special HTML characters in user content.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Convert inline LaTeX math ($...$) to Unicode via unicodeit.
/// Leaves non-math text unchanged.
fn convert_inline_latex(text: &str) -> String {
    if !text.contains('$') {
        return text.to_string();
    }
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(start) = remaining.find('$') {
        result.push_str(&remaining[..start]);
        let after_dollar = &remaining[start + 1..];
        // Don't match $$ (display math) — just skip the dollar
        if after_dollar.starts_with('$') {
            result.push('$');
            remaining = after_dollar;
            continue;
        }
        if let Some(end) = after_dollar.find('$') {
            let latex = &after_dollar[..end];
            // Strip \text{...}, \mathrm{...}, \mathbf{...} etc. to just their content
            let mut cleaned = latex.to_string();
            for cmd in &["\\text", "\\mathrm", "\\mathbf", "\\mathit", "\\textbf", "\\textit", "\\operatorname"] {
                while let Some(pos) = cleaned.find(cmd) {
                    let after_cmd = &cleaned[pos + cmd.len()..];
                    if let Some(brace_start) = after_cmd.find('{') {
                        let inner_start = pos + cmd.len() + brace_start + 1;
                        if let Some(brace_end) = cleaned[inner_start..].find('}') {
                            let inner = cleaned[inner_start..inner_start + brace_end].to_string();
                            cleaned = format!("{}{}{}", &cleaned[..pos], inner, &cleaned[inner_start + brace_end + 1..]);
                            continue;
                        }
                    }
                    break;
                }
            }
            // Convert remaining LaTeX to Unicode
            let unicode = unicodeit::replace(&cleaned);
            result.push_str(&unicode);
            remaining = &after_dollar[end + 1..];
        } else {
            // No closing $ — keep the dollar literal
            result.push('$');
            remaining = after_dollar;
        }
    }
    result.push_str(remaining);
    result
}


#[cfg(test)]
mod tests {
    use super::*;

    fn texts(spans: &[Span<'static, String>]) -> Vec<String> {
        spans.iter().map(|s| s.text.to_string()).collect()
    }

    /// Anyone in a room can send deeply nested HTML; rendering it must not
    /// overflow the (8 MB, main-thread) stack or stall the UI.
    #[test]
    fn deeply_nested_html_is_safe() {
        for (tag, depth) in [("blockquote", 20_000), ("b", 20_000), ("ul><li", 20_000), ("pre><b", 20_000), ("b", 400), ("blockquote", 400)] {
            let close = tag.split('>').next().unwrap();
            let html = format!("{}deep{}", format!("<{tag}>").repeat(depth), format!("</{close}>").repeat(depth));
            let (parts, elapsed) = std::thread::Builder::new()
                .stack_size(8 * 1024 * 1024)
                .spawn(move || {
                    let t = std::time::Instant::now();
                    let parts = html_to_parts(&html, &[], 14.0);
                    (parts, t.elapsed())
                })
                .unwrap()
                .join()
                .unwrap();
            assert!(!parts.is_empty(), "{tag} x{depth}");
            // Generous (debug builds), but far below the seconds it took.
            assert!(elapsed < std::time::Duration::from_secs(3), "{tag} x{depth}: {elapsed:?}");
        }
    }

    #[test]
    fn nesting_estimate() {
        assert_eq!(approx_nesting("<b>x<br><i>y</i></b><img src=a/>"), 2);
        assert_eq!(approx_nesting("plain"), 0);
    }

    #[test]
    fn inline_formatting_splits_spans() {
        let spans = html_to_spans("<b>bold</b> plain <code>x</code>");
        assert_eq!(texts(&spans), ["bold", " plain ", "x"]);
        assert_eq!(spans[0].font.map(|f| f.weight), Some(font::Weight::Bold));
    }

    #[test]
    fn links_carry_their_url() {
        let spans = html_to_spans("see <a href=\"https://example.org\">here</a>");
        assert_eq!(spans[1].link.as_deref(), Some("https://example.org"));
    }

    #[test]
    fn bare_urls_in_html_become_links() {
        let spans = html_to_spans("<b>deal:</b> see https://www.ebay.com/itm/377552588783. Also ebay.com/itm/206592999843; or www.example.org");
        let links: Vec<_> = spans.iter().filter_map(|s| s.link.as_deref()).collect();
        assert_eq!(links, ["https://www.ebay.com/itm/377552588783", "https://ebay.com/itm/206592999843", "https://www.example.org"]);
        assert_eq!(texts(&spans).concat(), "deal: see https://www.ebay.com/itm/377552588783. Also ebay.com/itm/206592999843; or www.example.org");
    }

    #[test]
    fn bare_url_detection_skips_non_urls() {
        assert!(find_urls("main.rs and 1.5/2 and https:// and src/main.rs").is_empty());
        let spans = html_to_spans("<code>https://example.org/x</code>");
        assert!(spans.iter().all(|s| s.link.is_none()));
    }

    #[test]
    fn mention_pills_get_at_prefix_and_user_id() {
        let spans = html_to_spans("<a href=\"https://matrix.to/#/@alice:example.org\">Alice</a>");
        assert_eq!(texts(&spans), ["@Alice"]);
    }

    #[test]
    fn code_blocks_and_tables_become_parts() {
        let parts = html_to_parts("<p>intro</p><pre><code>let x = 1;</code></pre>", &[], 14.0);
        assert!(matches!(parts[0], MessagePart::Text(_)));
        assert!(matches!(parts[1], MessagePart::CodeBlock(_)));
        let parts = html_to_parts("<table><tr><th>h</th></tr><tr><td>c</td></tr></table>", &[], 14.0);
        assert!(parts.iter().any(|p| matches!(p, MessagePart::Table { headers, rows } if headers.len() == 1 && rows.len() == 1)));
    }

    #[test]
    fn body_cache_memoizes_by_key() {
        let cache = BodyCache::default();
        let mut builds = 0;
        for _ in 0..3 {
            cache.get_or_insert(("k", 1u8), || { builds += 1; RenderedBody::Spans(Vec::new()) });
        }
        assert_eq!(builds, 1);
        cache.get_or_insert(("k", 2u8), || { builds += 1; RenderedBody::Spans(Vec::new()) });
        assert_eq!(builds, 2);
    }
}
