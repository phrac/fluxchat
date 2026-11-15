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
    /// Whitespace seen between inline elements, held until more inline
    /// text follows it: `segments.len()` at the time, whether it had a
    /// newline, and its style.
    pending_space: Option<(usize, bool, Style)>,
    /// Lists we're inside of (nested items are indented).
    list_depth: usize,
    /// Width of the current list item's marker: later blocks of the item
    /// line up under its text.
    item_indent: usize,
    /// `segments.len()` right after a list marker or item indent: a block
    /// starting there stays on that line.
    line_lead: Option<usize>,
    /// Math spans still converted in this message (`MAX_MATH_SPANS`).
    math_left: usize,
}

impl<'a> WalkCtx<'a> {
    fn new(nodes: &'a [Node]) -> Self {
        WalkCtx {
            nodes,
            segments: Vec::new(),
            depth: 0,
            pending_space: None,
            list_depth: 0,
            item_indent: 0,
            line_lead: None,
            math_left: MAX_MATH_SPANS,
        }
    }

    /// Before inline text is pushed: the space that separated it from the
    /// inline text before it (`<b>a</b> <i>b</i>`), unless a block or line
    /// break came in between.
    fn flush_space(&mut self, next: &str) {
        let Some((at, newline, style)) = self.pending_space.take() else { return };
        if at != self.segments.len() || next.starts_with(char::is_whitespace) {
            return;
        }
        self.segments.push(if newline { Segment::LineBreak } else { Segment::Text(" ".to_string(), style) });
    }

    /// End the current line, if anything is on it.
    fn end_line(&mut self) {
        if !matches!(self.segments.last(), None | Some(Segment::LineBreak)) {
            self.segments.push(Segment::LineBreak);
        }
    }

    /// A block (paragraph, quote) starts on its own line. In a list item
    /// the first one stays beside the marker and later ones are indented
    /// under it.
    fn start_block(&mut self) {
        if self.line_lead == Some(self.segments.len()) {
            return;
        }
        self.end_line();
        if self.item_indent > 0 {
            self.segments.push(Segment::Text(" ".repeat(self.item_indent), Style::default()));
            self.line_lead = Some(self.segments.len());
        }
    }

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
        match plain_line_breaks(&out) {
            Cow::Borrowed(_) => out,
            Cow::Owned(fixed) => fixed,
        }
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
                self.flush_space(&text);
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
                // Whitespace-only text: between inline content
                // (`<b>a</b> <i>b</i>`, two mention pills) it's a space
                // the sender typed, kept if more inline text follows;
                // next to a block it's only the markup's formatting.
                if text.trim().is_empty() {
                    if let Some(Segment::Text(before, _) | Segment::Link { text: before, .. }) = self.segments.last() {
                        if !before.ends_with(char::is_whitespace) {
                            self.pending_space = Some((self.segments.len(), text.contains('\n'), style.clone()));
                        }
                    }
                    return;
                }
                let text = plain_line_breaks(text);
                // Code keeps its dollars (`$HOME:$PATH`).
                let text = if style.code { text.into_owned() } else { convert_inline_latex(&text, &mut self.math_left) };
                self.flush_space(&text);
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

                    // Custom emoji and inline images aren't drawn in the
                    // text: their written form (`:party:`) stands in, or a
                    // message of only custom emoji would be an empty row.
                    "img" => {
                        let label = ["alt", "title"].iter()
                            .filter_map(|a| self.get_attr(id, a))
                            .find(|l| !l.trim().is_empty());
                        if let Some(label) = label {
                            self.flush_space(&label);
                            self.segments.push(Segment::Text(plain_line_breaks(&label).into_owned(), style.clone()));
                        }
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
                            self.flush_space(&text);
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
                        // After text ("a<div>b</div>") or an item's earlier
                        // paragraph, a paragraph starts a new line.
                        self.start_block();
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
                        // On its own line: a list nested in an item would
                        // otherwise continue the item's line ("• a• b").
                        self.end_line();
                        let indent = "  ".repeat(self.list_depth);
                        // `<ol start="3">`: a list that carries on after a
                        // paragraph or a code block.
                        let mut number = self.get_attr(id, "start")
                            .filter(|_| tag == "ol")
                            .and_then(|n| n.trim().parse::<i64>().ok())
                            .unwrap_or(1);
                        let outer = (self.item_indent, self.line_lead);
                        self.list_depth += 1;
                        for child in children.iter() {
                            if self.is_element(*child, "li") {
                                let marker = if tag == "ol" {
                                    // Numbered list
                                    format!("{indent}{number}. ")
                                } else {
                                    // Bullet list
                                    format!("{indent}\u{2022} ")
                                };
                                number = number.saturating_add(1);
                                self.item_indent = marker.chars().count();
                                self.segments.push(Segment::Text(marker, s.clone()));
                                self.line_lead = Some(self.segments.len());
                                // Walk li children
                                let li_children: Vec<NodeId> = self.nodes[*child].children.clone();
                                for li_child in li_children {
                                    self.walk(li_child, &s);
                                }
                                // One break, also after a nested list (which
                                // ended its own last line).
                                self.end_line();
                            } else {
                                self.walk(*child, &s);
                            }
                        }
                        self.list_depth -= 1;
                        (self.item_indent, self.line_lead) = outer;
                    }
                    "li" => {
                        // li handled by parent ol/ul — but if we get here directly, just walk children
                        for child in children {
                            self.walk(child, &s);
                        }
                    }
                    "blockquote" => {
                        // A quote is its own lines, also when it holds bare
                        // text ("<blockquote>quoted</blockquote>reply").
                        self.start_block();
                        for child in children {
                            self.walk(child, &s);
                        }
                        self.end_line();
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
        // Cells are laid out on their own: nothing of the line or list
        // item the table sits in carries into them.
        let outer = (self.list_depth, self.item_indent, self.line_lead);
        (self.list_depth, self.item_indent, self.line_lead) = (0, 0, None);

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
                self.pending_space = None;
                let children: Vec<NodeId> = self.nodes[cell_id].children.clone();
                let mut cell_style = style.clone();
                if self.is_element(cell_id, "th") {
                    cell_style.bold = true;
                }
                for child in children {
                    self.walk(child, &cell_style);
                }
                let cell_segs = std::mem::replace(&mut self.segments, saved);
                self.pending_space = None;
                cells.push(cell_segs);
            }
            if is_header_row && headers.is_empty() {
                headers = cells;
            } else {
                rows.push(cells);
            }
        }

        (self.list_depth, self.item_indent, self.line_lead) = outer;
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

        // Collect all text from the code subtree (blank lines included:
        // they're part of the code)
        let code = self.collect_code_text(code_root);

        self.segments.push(Segment::CodeBlock {
            code: plain_line_breaks(code.trim_matches('\n')).into_owned(),
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
        // A `<br>` and a newline right beside it ("a<br>\nb") are one line
        // break, not a blank line: `after_br` drops the newline that follows
        // one, `text_newline` the `<br>` that follows a newline.
        let (mut after_br, mut text_newline) = (false, false);
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            match &self.nodes[id].data {
                NodeData::Text(t) => {
                    let t = if after_br { t.strip_prefix('\n').unwrap_or(t) } else { t.as_str() };
                    after_br = false;
                    if !t.is_empty() {
                        text_newline = t.ends_with('\n');
                        out.push_str(t);
                    }
                }
                NodeData::Element { name, .. } if &*name.local == "br" => {
                    if !text_newline {
                        out.push('\n');
                    }
                    (after_br, text_newline) = (true, false);
                }
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
        return vec![Segment::Text(plain_line_breaks(&text).into_owned(), Style::default())];
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
    let mut ctx = WalkCtx::new(&nodes);

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
        // Some clients put the @ in the pill's text themselves ("@Alice",
        // or the whole user ID): one @, and the nick's own colour.
        let nick_text = span.text.strip_prefix('@').unwrap_or(&span.text).to_string();
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
    // The room part only: a link to a message (`!room:server/$event`) is
    // still that room's pill, by name.
    let room_id = decoded.split(['?', '/']).next().unwrap_or(&decoded).to_string();

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

/// Markers of the basic markdown read in plain bodies, with the tag each
/// wraps its text in (`**` before `*`).
const PLAIN_MARKERS: [(&str, &str); 4] = [("**", "b"), ("*", "i"), ("~~", "del"), ("`", "code")];

/// Length of the text a just-opened `marker` wraps: up to its closing marker
/// on the same line. Emphasis hugs its text on both sides, as in Markdown,
/// and a code span may hold anything.
fn marked_len(after: &str, marker: &str) -> Option<usize> {
    let line = after.split('\n').next().unwrap_or(after);
    line.match_indices(marker).map(|(i, _)| i).find(|&i| {
        i > 0
            && (marker == "`" || !line[..i].ends_with(char::is_whitespace))
            // A `*` isn't closed by half of a `**`.
            && (marker != "*" || !(line[..i].ends_with('*') || line[i + 1..].starts_with('*')))
    })
}

/// Convert plain text with basic markdown to Spans.
/// Handles **bold**, *italic*, ~~strike~~, `code`, and ```codeblocks```.
///
/// A marker only counts with its closing one: plain bodies are mostly not
/// markdown, and `SELECT * FROM t`, `ls *.rs`, a stray backtick or a
/// `* bullet` would otherwise lose the character and restyle what follows.
pub fn plain_text_to_spans(input: &str) -> Vec<Span<'static, String>> {
    // Convert basic markdown to HTML, then use the HTML parser (which also
    // converts inline LaTeX — outside code, so `$HOME:$PATH` keeps its dollars)
    let mut html = String::with_capacity(input.len());
    // Per marker: the end of the line on which a search for its closing one
    // failed. Nothing later on that line closes it either, so a long line of
    // lone `*` is searched once, not once per `*`.
    let mut unclosed = [0usize; PLAIN_MARKERS.len()];
    let mut i = 0;

    while let Some(c) = input[i..].chars().next() {
        let rest = &input[i..];
        if let Some(after) = rest.strip_prefix("```") {
            // Code block — until the closing ``` (or the end)
            let code = after.find("```").map_or(after, |end| &after[..end]);
            html.push_str("<pre><code>");
            html.push_str(&html_escape(code));
            html.push_str("</code></pre>");
            i += (code.len() + 6).min(rest.len());
            continue;
        }
        if rest.starts_with("``") {
            html.push_str("``");
            i += 2;
            continue;
        }
        if let Some((k, (marker, tag))) = PLAIN_MARKERS.iter().enumerate().find(|(_, (m, _))| rest.starts_with(m)) {
            let after = &rest[marker.len()..];
            let opens = *marker == "`" || !after.starts_with(char::is_whitespace);
            let wrapped = (opens && i >= unclosed[k]).then(|| marked_len(after, marker)).flatten();
            match wrapped {
                Some(len) => {
                    html.push_str(&format!("<{tag}>{}</{tag}>", html_escape(&after[..len])));
                    i += len + 2 * marker.len();
                }
                None => {
                    if opens && i >= unclosed[k] {
                        unclosed[k] = i + rest.find('\n').unwrap_or(rest.len());
                    }
                    html.push_str(marker);
                    i += marker.len();
                }
            }
            continue;
        }
        if rest.starts_with("http://") || rest.starts_with("https://") {
            // The character ending the URL stays in the text.
            let run = rest.find(|c: char| c.is_whitespace() || c == '<' || c == '>').map_or(rest, |end| &rest[..end]);
            let url = trim_url_end(run);
            let escaped = html_escape(url);
            html.push_str(&format!("<a href=\"{}\">{escaped}</a>", escaped.replace('"', "&quot;")));
            i += url.len();
            continue;
        }
        match c {
            '<' => html.push_str("&lt;"),
            '>' => html.push_str("&gt;"),
            '&' => html.push_str("&amp;"),
            '\n' => html.push_str("<br>"),
            // CRLF is one line break (the parser reads a CR as another).
            '\r' if rest[1..].starts_with('\n') => {}
            _ => html.push(c),
        }
        i += c.len_utf8();
    }

    html_to_spans(&html)
}

/// Punctuation that ends a sentence rather than a URL written in it.
const URL_END_PUNCTUATION: [char; 10] = ['.', ',', ']', ';', ':', '!', '?', '"', '\'', '>'];

/// `url` without the punctuation of the sentence around it. A `)` that
/// closes a `(` inside the URL is part of it (`…/wiki/Rust_(language)`).
fn trim_url_end(url: &str) -> &str {
    let opens = url.matches('(').count();
    let mut closes = url.matches(')').count();
    let mut end = url.len();
    for c in url.chars().rev() {
        if c == ')' && closes > opens {
            closes -= 1;
        } else if !URL_END_PUNCTUATION.contains(&c) {
            break;
        }
        end -= c.len_utf8();
    }
    &url[..end]
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
        let candidate = trim_url_end(&word[lead..]);
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

/// `text` with every paragraph separator as a plain `\n`. The text layout
/// starts a new line at each of them (a CR written as `&#13;`, NEL, U+2029,
/// the FS/GS/RS controls) while selection offsets count `\n` lines, so a
/// selection below one would copy other text than it highlights.
fn plain_line_breaks(text: &str) -> Cow<'_, str> {
    const BREAKS: [char; 6] = ['\r', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2029}'];
    if text.contains(BREAKS) {
        Cow::Owned(text.replace("\r\n", "\n").replace(BREAKS, "\n"))
    } else {
        Cow::Borrowed(text)
    }
}

/// Math spans converted in one message; later ones stay as typed. Each is
/// thousands of passes in unicodeit (a fraction of a millisecond), which a
/// message of 16 000 `$a$` turned into a window frozen for seconds.
const MAX_MATH_SPANS: usize = 200;
/// Longest math span converted (bytes): those passes, and the checks here,
/// grow with its length. Inline math is a few dozen characters.
const MAX_MATH_LEN: usize = 4000;

/// Where the `$` closing inline math is, for the text after an opening `$`.
/// Pandoc's rule, so prices stay prices ("$0.18 of $5.00"): the opening `$`
/// has a non-space right after it (`math_opens`), the closing one a
/// non-space right before it and no digit after it.
fn math_end(after_dollar: &str) -> Option<usize> {
    after_dollar.match_indices('$').map(|(i, _)| i).find(|&i| {
        let before = after_dollar[..i].chars().next_back();
        let after = after_dollar[i + 1..].chars().next();
        before.is_some_and(|c| !c.is_whitespace()) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

fn math_opens(after_dollar: &str) -> bool {
    !after_dollar.chars().next().is_none_or(char::is_whitespace)
}

/// unicodeit's accents (`\hat{x}`, `\vec{v}`, …) take one character: it
/// keeps a group's first and drops the rest (`\overline{AB}` → "A̅}"), and
/// it slices by byte doing so — `\hat{y_i}`, whose subscript is three bytes
/// by then, panics. Math with a longer group is left as typed.
fn accents_fit(latex: &str) -> bool {
    const ACCENTS: [&str; 14] = [
        "\\doubleunderline", "\\strikethrough", "\\underline", "\\overline", "\\tilde", "\\grave",
        "\\acute", "\\slash", "\\breve", "\\ddot", "\\dot", "\\bar", "\\vec", "\\hat",
    ];
    ACCENTS.iter().all(|accent| {
        latex.match_indices(accent).all(|(at, _)| {
            let Some(group) = latex[at + accent.len()..].strip_prefix('{') else { return true };
            let mut inner = group.split('}').next().unwrap_or(group).chars();
            match inner.next() {
                // One command (`\vec{\alpha}`) is one character once converted.
                Some('\\') => inner.all(|c| c.is_ascii_alphabetic()),
                Some(_) => inner.next().is_none(),
                None => true,
            }
        })
    })
}

/// One math span as Unicode; `None` when it's shown as typed instead.
fn latex_to_unicode(latex: &str) -> Option<String> {
    if latex.len() > MAX_MATH_LEN {
        return None;
    }
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
    let cleaned = slash_negations(&cleaned);
    if !accents_fit(&cleaned) {
        return None;
    }
    // Convert remaining LaTeX to Unicode. Message text is anyone's, and the
    // accent check doesn't see every way into that slicing (`\not\\alpha`
    // gets there through unicodeit's own rewrite of `\not`): a panic costs
    // the span its conversion, not everyone in the room their window.
    std::panic::catch_unwind(|| unicodeit::replace(&cleaned)).ok()
}

/// `\not\in` as `\slash{\in}`: what unicodeit makes of it itself, but it
/// adds a stray `"` that then shows after the symbol.
fn slash_negations(latex: &str) -> String {
    const NOT: &str = "\\not";
    let mut out = String::with_capacity(latex.len());
    let mut rest = latex;
    while let Some(at) = rest.find("\\not\\") {
        let command = &rest[at + NOT.len() + 1..];
        let len = command.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(command.len());
        if len == 0 {
            // Not a command name: left to unicodeit.
            out.push_str(&rest[..at + NOT.len()]);
            rest = &rest[at + NOT.len()..];
            continue;
        }
        out.push_str(&rest[..at]);
        out.push_str(&format!("\\slash{{\\{}}}", &command[..len]));
        rest = &command[len..];
    }
    out.push_str(rest);
    out
}

/// Convert inline LaTeX math ($...$) to Unicode via unicodeit.
/// Leaves non-math text unchanged. `budget` is how many more spans this
/// message may convert (`MAX_MATH_SPANS`).
fn convert_inline_latex(text: &str, budget: &mut usize) -> String {
    if !text.contains('$') {
        return text.to_string();
    }
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;
    // Cleared when a search for a closing `$` finds none: nothing further
    // on closes math either (a closing `$` doesn't depend on its opening
    // one), so a text full of lone `$` is searched once.
    let mut closable = true;
    while let Some(start) = remaining.find('$') {
        result.push_str(&remaining[..start]);
        let after_dollar = &remaining[start + 1..];
        // Don't match $$ (display math) — just skip the dollar
        if after_dollar.starts_with('$') {
            result.push('$');
            remaining = after_dollar;
            continue;
        }
        let end = if closable && *budget > 0 && math_opens(after_dollar) {
            let end = math_end(after_dollar);
            closable = end.is_some();
            end
        } else {
            None
        };
        if let Some(end) = end {
            let latex = &after_dollar[..end];
            *budget -= 1;
            match latex_to_unicode(latex) {
                Some(unicode) => result.push_str(&unicode),
                None => {
                    result.push('$');
                    result.push_str(latex);
                    result.push('$');
                }
            }
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

    fn latex(text: &str) -> String {
        let mut budget = MAX_MATH_SPANS;
        convert_inline_latex(text, &mut budget)
    }

    #[test]
    fn prices_are_not_math() {
        assert_eq!(latex("Spent $0.18 of $5.00 today"), "Spent $0.18 of $5.00 today");
        assert_eq!(latex("costs $5"), "costs $5");
        assert_eq!(latex("a $ b $ c"), "a $ b $ c");
        // Real inline math still converts.
        assert_eq!(latex("so $\\alpha$ it is"), "so \u{3b1} it is");
        assert_eq!(latex("$x$"), "x");
    }

    /// unicodeit slices by byte around an accent's group: these panicked,
    /// i.e. crashed the app for everyone the message was shown to.
    #[test]
    fn accented_math_with_a_longer_group_stays_as_typed() {
        for math in ["$\\hat{y_i}$", "$\\vec{v_1}$", "$\\bar{x^2}$", "$\\hat{a\\alpha}$", "$\\tilde{n\u{f1}}$", "$\\hat{y_i$", "$\\text{mean} = \\bar{\\text{x}_i}$"] {
            assert_eq!(latex(math), math);
        }
        // Dropped the B and left a brace before.
        assert_eq!(latex("$\\overline{AB}$"), "$\\overline{AB}$");
        // The same slicing, reached through unicodeit's own `\not` rewrite.
        assert_eq!(latex("$\\not\\\\alpha$"), "$\\not\\\\alpha$");
        assert_eq!(latex("$\\not\\_\\alpha$"), "$\\not\\_\\alpha$");
        // A negated symbol has no quote after it.
        assert_eq!(latex("$a \\not\\in B$"), "a \u{2208}\u{338} B");
        assert_eq!(latex("$a \\notin B, a \\not= b$"), unicodeit::replace("a \\notin B, a \\not= b"));
        // One character (or one symbol) under the accent still converts.
        assert_eq!(latex("$\\hat{x}$"), "x\u{302}");
        assert_eq!(latex("$\\vec{\\alpha}_i$"), "\u{3b1}\u{20d7}\u{1d62}");
        assert_eq!(latex("$\\tilde{\u{f1}}$"), "\u{f1}\u{303}");
        // Through the message path, where it crashed.
        assert_eq!(texts(&html_to_spans("<p>so $\\hat{y_i}$ is the estimate</p>")).concat().trim_end(), "so $\\hat{y_i}$ is the estimate");
    }

    #[test]
    fn a_message_full_of_math_or_dollars_is_parsed_in_bounded_time() {
        let t = std::time::Instant::now();
        // 16 000 spans took over 20 s (debug): the first MAX_MATH_SPANS convert.
        let spans = html_to_spans(&"$a$ ".repeat(16_000));
        let text = texts(&spans).concat();
        assert!(text.starts_with("a a a "), "{}", &text[..20]);
        assert!(text.ends_with("$a$ $a$ "), "{}", &text[text.len() - 20..]);
        // Split across text nodes: the budget is the message's, not the node's.
        let text = texts(&html_to_spans(&"<b>$a$</b> ".repeat(8_000))).concat();
        assert!(text.ends_with("$a$ $a$"), "{}", &text[text.len() - 20..]);
        // Lone dollars (no valid closing one) and one huge span.
        assert_eq!(latex(&"$a ".repeat(20_000)), "$a ".repeat(20_000));
        let huge = format!("${}$", "x+".repeat(30_000));
        assert_eq!(latex(&huge), huge);
        assert!(t.elapsed() < std::time::Duration::from_secs(5), "{:?}", t.elapsed());
    }

    #[test]
    fn spaces_between_inline_elements_are_kept() {
        let joined = |html: &str| texts(&html_to_spans(html)).concat();
        assert_eq!(joined("<strong>a</strong> <em>b</em> <code>c</code>"), "a b c");
        // Two people mentioned one after the other.
        assert_eq!(
            joined("<a href=\"https://matrix.to/#/@a:x.org\">Al</a> <a href=\"https://matrix.to/#/@b:x.org\">Bo</a>: hi"),
            "@Al @Bo: hi",
        );
        assert_eq!(joined("<a href=\"https://x.org\">one</a> <a href=\"https://y.org\">two</a>"), "one two");
        // A line break between them stays a line break, like one in text does.
        assert_eq!(joined("<b>Name:</b>\n<b>Other:</b>"), "Name:\nOther:");
        // Around blocks it is only the markup's own formatting.
        assert_eq!(joined("<p>a</p>\n<p>b</p>\n"), "a\n\nb\n\n");
        assert_eq!(joined("<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n"), "\u{2022} a\n\u{2022} b\n");
        assert_eq!(joined("<b>a</b> <br> <i>b</i>"), "a\nb");
        assert_eq!(joined("<b>a </b> <i>b</i>"), "a b");
        assert_eq!(joined("<b>a</b> "), "a");
    }

    #[test]
    fn lists_keep_their_numbers_nesting_and_paragraphs() {
        let joined = |html: &str| texts(&html_to_spans(html)).concat();
        // A numbered list continued after a code block or paragraph.
        assert_eq!(joined("<ol start=\"3\"><li>c</li><li>d</li></ol>"), "3. c\n4. d\n");
        assert_eq!(joined("<ol start=\"x\"><li>a</li></ol>"), "1. a\n");
        // A nested list is its own indented lines, with no blank line after.
        assert_eq!(
            joined("<ul><li>a<ul><li>b</li><li>c</li></ul></li><li>d</li></ul>"),
            "\u{2022} a\n  \u{2022} b\n  \u{2022} c\n\u{2022} d\n",
        );
        // An item's second paragraph is a new line under the first.
        assert_eq!(
            joined("<ol>\n<li>\n<p><strong>Title</strong></p>\n<p>Body</p>\n</li>\n<li>\n<p>Next</p>\n</li>\n</ol>"),
            "1. Title\n   Body\n2. Next\n",
        );
        // A list right after text starts on its own line.
        assert_eq!(joined("Items:<ul><li>a</li></ul>"), "Items:\n\u{2022} a\n");
    }

    #[test]
    fn quotes_and_paragraphs_start_their_own_line() {
        let joined = |html: &str| texts(&html_to_spans(html)).concat();
        assert_eq!(joined("<blockquote>quoted</blockquote>reply"), "quoted\nreply");
        assert_eq!(joined("intro<blockquote>quoted</blockquote>"), "intro\nquoted\n");
        assert_eq!(joined("line one<div>line two</div><div>line three</div>"), "line one\nline two\n\nline three\n\n");
        // Unchanged where the markup already had its paragraphs.
        assert_eq!(joined("<blockquote><p>quoted</p></blockquote><p>reply</p>"), "quoted\n\nreply\n\n");
    }

    #[test]
    fn blank_lines_stay_in_code_blocks() {
        let code = |html: &str| match html_to_parts(html, &[], 14.0).into_iter().next() {
            Some(MessagePart::CodeBlock(spans)) => texts(&spans).concat(),
            _ => panic!("no code block"),
        };
        assert_eq!(code("<pre><code>fn a() {}\n\nfn b() {}\n</code></pre>"), " 1 fn a() {}\n 2 \n 3 fn b() {}");
        // Lines as <br> (what this app sent until recently), blank ones too.
        assert_eq!(code("<pre><code>a<br>b<br><br>c</code></pre>"), " 1 a\n 2 b\n 3 \n 4 c");
        // A <br> and the newline beside it are one break.
        assert_eq!(code("<pre><code>a<br>\nb\n<br>c\n<br>\nd</code></pre>"), " 1 a\n 2 b\n 3 c\n 4 d");
    }

    #[test]
    fn custom_emoji_show_their_shortcode() {
        let joined = |html: &str| texts(&html_to_spans(html)).concat();
        assert_eq!(joined("<img data-mx-emoticon src=\"mxc://x/y\" alt=\":party:\" title=\":party:\" height=\"32\">"), ":party:");
        assert_eq!(joined("nice <img src=\"mxc://x/y\" title=\":tada:\"> <img src=\"mxc://x/z\" alt=\":100:\">"), "nice :tada: :100:");
        assert_eq!(joined("a<img src=\"mxc://x/y\">b"), "ab");
    }

    #[test]
    fn mention_pills_have_one_at_sign() {
        let pill = |text: &str| html_to_spans(&format!("<a href=\"https://matrix.to/#/@alice:example.org\">{text}</a>")).remove(0);
        assert_eq!(pill("@Alice").text, "@Alice");
        assert_eq!(pill("@alice:example.org").text, "@alice:example.org");
        // …and the colour the nick has everywhere else.
        assert_eq!(pill("@Alice").color, pill("Alice").color);
    }

    #[test]
    fn a_message_link_is_its_rooms_pill() {
        let rooms = [fluxchat_core::RoomInfo { room_id: "!r:x.org".into(), name: "general".into(), ..Default::default() }];
        let spans = html_to_spans_with_rooms("<a href=\"https://matrix.to/#/!r:x.org/$ev?via=x.org\">https://matrix.to/#/!r:x.org/$ev?via=x.org</a>", &rooms, 14.0);
        assert_eq!(texts(&spans), ["#general"]);
        assert_eq!(spans[0].link.as_deref(), Some("https://matrix.to/#/!r:x.org/$ev?via=x.org"));
    }

    #[test]
    fn plain_urls_keep_the_character_after_them() {
        let joined = |input: &str| texts(&plain_text_to_spans(input)).concat();
        assert_eq!(joined("see https://x.org now"), "see https://x.org now");
        assert_eq!(joined("https://x.org/a."), "https://x.org/a.");
    }

    #[test]
    fn urls_keep_their_balanced_parentheses() {
        let wiki = "https://en.wikipedia.org/wiki/Rust_(programming_language)";
        let links = |spans: Vec<Span<'static, String>>| spans.iter().filter_map(|s| s.link.clone()).collect::<Vec<_>>();
        for text in [format!("see {wiki} ok"), format!("({wiki})"), format!("it's {wiki}).")] {
            assert_eq!(links(html_to_spans(&text)), [wiki], "{text}");
            assert_eq!(links(plain_text_to_spans(&text)), [wiki], "{text}");
            assert_eq!(texts(&plain_text_to_spans(&text)).concat(), text);
        }
        assert_eq!(links(html_to_spans("(see https://x.org/a)")), ["https://x.org/a"]);
        assert_eq!(links(plain_text_to_spans("really https://x.org/a?! \"https://y.org/\"")), ["https://x.org/a", "https://y.org/"]);
        assert_eq!(trim_url_end(&")".repeat(60_000)), "");
    }

    #[test]
    fn plain_bodies_keep_unpaired_markdown_characters() {
        let joined = |input: &str| texts(&plain_text_to_spans(input)).concat();
        for text in [
            "SELECT * FROM t WHERE a * b",
            "ls *.rs and void *p, *q",
            "it`s 5 o'clock",
            "* item one\n* item two",
            "a ** b ~~ c",
            "rated ***** stars~~",
            "2 * 3 * 4",
        ] {
            let spans = plain_text_to_spans(text);
            assert_eq!(texts(&spans).concat(), text);
            assert!(spans.iter().all(|s| s.font.is_none() && !s.strikethrough), "{text}");
        }
        // Paired ones are still read.
        let spans = plain_text_to_spans("**bold** and *it* and ~~no~~ and `c` and a*b*c");
        assert_eq!(texts(&spans), ["bold", " and ", "it", " and ", "no", " and ", "c", " and a", "b", "c"]);
        assert_eq!(spans[0].font.map(|f| f.weight), Some(font::Weight::Bold));
        assert_eq!(spans[2].font.map(|f| f.style), Some(font::Style::Italic));
        assert!(spans[4].strikethrough);
        // Code keeps its dollars (they were read as math, and dropped).
        assert_eq!(joined("run `echo $HOME:$PATH` now"), "run echo $HOME:$PATH now");
        assert_eq!(joined("```\nexport PATH=$HOME/bin:$PATH\n```"), "\n 1 export PATH=$HOME/bin:$PATH\n");
        assert_eq!(joined("so $\\alpha$ it is"), "so \u{3b1} it is");
        // Windows line endings are one line break each.
        assert_eq!(joined("a\r\nb\r\n\r\nc"), "a\nb\n\nc");
        // A long line of lone markers isn't searched once per marker.
        let t = std::time::Instant::now();
        let stars = "*a ".repeat(20_000);
        assert_eq!(joined(&stars), stars);
        assert!(t.elapsed() < std::time::Duration::from_secs(3), "{:?}", t.elapsed());
    }

    /// Selection offsets count `\n` lines; the text layout also breaks at
    /// these.
    #[test]
    fn paragraph_separators_become_newlines() {
        let joined = |html: &str| texts(&html_to_spans(html)).concat();
        assert_eq!(joined("a&#13;b&#x2029;c\u{85}d &#x1c; e"), "a\nb\nc\nd \n e");
        assert_eq!(joined("<a href=\"https://x.org\">x&#13;&#10;y</a>"), "x\ny");
        let parts = html_to_parts("<pre><code>a\u{2029}b</code></pre>", &[], 14.0);
        assert!(matches!(&parts[0], MessagePart::CodeBlock(spans) if texts(spans).concat() == " 1 a\n 2 b"));
    }
}
