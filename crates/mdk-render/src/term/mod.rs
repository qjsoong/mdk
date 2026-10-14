//! ANSI and plain-text terminal rendering.

mod emit;
mod model;
mod table;

pub use emit::encode;

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use comrak::Node;
use comrak::nodes::{NodeValue, TableAlignment};
use mdk_core::ast::{collect_text, node_span};
use mdk_core::{Dialect, Source, with_parsed_options};

use crate::options::TermOptions;
use emit::Emitter;
use table::TableChars;

pub use model::{
    Block, BlockKind, Cell, Color, Continuation, Document, Line, LineKind, SourceMap,
    Span, SrcRange, Style,
};

/// Renders Markdown to a structured [`Document`] with source mapping.
///
/// Every line records the Markdown construct it came from, which block it
/// belongs to and whether it continues a wrapped line; every span records the
/// bytes of the source it displays. Byte ranges refer to `markdown`, so the
/// caller keeps that text to turn ranges back into strings.
#[must_use]
pub fn render_terminal_document(
    markdown: &str,
    dialect: Dialect,
    options: &TermOptions,
) -> Document {
    let source = Source::new(markdown.to_string());
    let comrak_options = dialect.comrak_options();
    with_parsed_options(&source, &comrak_options, |root| {
        let mut renderer = TerminalRenderer::new(options, root, &source);
        renderer.render_blocks(root, true);
        renderer.render_footnotes(root);
        renderer.finish()
    })
}

/// Renders Markdown for display in a terminal.
///
/// With `color` enabled the output uses ANSI escape sequences; otherwise the
/// result is plain text with ASCII tables, bullets and rules. This is
/// [`render_terminal_document`] encoded as text.
#[must_use]
pub fn render_terminal(
    markdown: &str,
    dialect: Dialect,
    options: &TermOptions,
) -> String {
    let document = render_terminal_document(markdown, dialect, options);
    emit::encode(&document.lines, options.color)
}

struct TerminalRenderer<'a> {
    emitter: Emitter,
    options: &'a TermOptions,
    source: &'a Source,
    blocks: Vec<Block>,
    footnote_numbers: BTreeMap<String, usize>,
}

impl<'a> TerminalRenderer<'a> {
    fn new(options: &'a TermOptions, root: Node<'_>, source: &'a Source) -> Self {
        let mut footnote_numbers = BTreeMap::new();
        let mut next = 1usize;
        for node in root.descendants() {
            if let NodeValue::FootnoteReference(reference) = &node.data().value {
                footnote_numbers
                    .entry(reference.name.clone())
                    .or_insert_with(|| {
                        let number = reference.ix as usize;
                        next = next.max(number + 1);
                        number
                    });
            }
        }
        for node in root.descendants() {
            if let NodeValue::FootnoteDefinition(definition) = &node.data().value {
                footnote_numbers
                    .entry(definition.name.clone())
                    .or_insert_with(|| {
                        let number = next;
                        next += 1;
                        number
                    });
            }
        }
        Self {
            emitter: Emitter::new(options.width, Vec::new()),
            options,
            source,
            blocks: Vec::new(),
            footnote_numbers,
        }
    }

    fn finish(self) -> Document {
        Document {
            lines: self.emitter.finish(),
            blocks: self.blocks,
        }
    }

    /// Registers a content block and returns its index.
    fn begin_block(&mut self, node: Node<'_>, kind: BlockKind) -> usize {
        let range = self.source.span_range(node_span(node));
        self.blocks.push(Block { kind, range });
        self.blocks.len() - 1
    }

    fn render_blocks(&mut self, node: Node<'_>, loose: bool) {
        let children: Vec<Node<'_>> = node
            .children()
            .filter(|child| !matches!(child.data().value, NodeValue::FrontMatter(_)))
            .collect();
        for (index, child) in children.iter().enumerate() {
            if index > 0 {
                if loose {
                    self.emitter.blank();
                } else {
                    self.emitter.end_line();
                }
            }
            self.render_block(child);
        }
    }

    fn render_block(&mut self, node: Node<'_>) {
        match &node.data().value {
            NodeValue::Paragraph => {
                let block = self.begin_block(node, BlockKind::Paragraph);
                self.emitter.set_context(LineKind::Text, Some(block));
                let spans = self.inline_spans(node, Style::default());
                self.emitter.push_spans(&spans);
            }
            NodeValue::Heading(heading) => {
                let block = self.begin_block(node, BlockKind::Heading(heading.level));
                self.emitter
                    .set_context(LineKind::Heading(heading.level), Some(block));
                let (style, plain_prefix) = heading_style(heading.level);
                if !self.options.color && plain_prefix > 0 {
                    self.emitter.push_text(
                        Style::default().dim(),
                        &format!("{} ", "#".repeat(plain_prefix)),
                    );
                }
                let spans = self.inline_spans(node, style);
                self.emitter.push_spans(&spans);
            }
            NodeValue::CodeBlock(code) => self.render_code_block(node, code),
            NodeValue::HtmlBlock(block) => self.render_html_block(node, block),
            NodeValue::BlockQuote => {
                let quote = if self.options.color { "│ " } else { "> " };
                let mut prefix = self.emitter.prefix().to_vec();
                prefix.push(Span::new(Style::default().dim(), quote));
                let old = self.emitter.set_prefix(prefix);
                self.render_blocks(node, true);
                self.emitter.set_prefix(old);
            }
            NodeValue::List(list) => self.render_list(node, list),
            NodeValue::Table(table) => {
                self.render_table(node, table.alignments.clone())
            }
            NodeValue::ThematicBreak => {
                let block = self.begin_block(node, BlockKind::Rule);
                self.emitter.set_context(LineKind::Rule, Some(block));
                let width = self.emitter.content_width().max(3);
                let rule: String = if self.options.color {
                    "─".repeat(width)
                } else {
                    "-".repeat(width)
                };
                self.emitter.push_text(Style::default().dim(), &rule);
                self.emitter.newline();
            }
            NodeValue::FootnoteDefinition(_) | NodeValue::FrontMatter(_) => {}
            _ => self.render_blocks(node, true),
        }
    }

    fn render_code_block(
        &mut self,
        node: Node<'_>,
        code: &comrak::nodes::NodeCodeBlock,
    ) {
        let block = self.begin_block(node, BlockKind::Code);
        self.emitter.set_context(LineKind::Code, Some(block));
        let style = Style::default().dim();
        let border = if self.options.color && self.options.code_border {
            "│ "
        } else if self.options.color {
            "  "
        } else {
            "    "
        };
        let mut prefix = self.emitter.prefix().to_vec();
        prefix.push(Span::new(style, border));
        let old = self.emitter.set_prefix(prefix);
        let content = code.literal.trim_end_matches('\n');
        if !content.is_empty() {
            let offsets = self.code_line_offsets(node, code, content);
            for (index, line) in content.split('\n').enumerate() {
                if line.is_empty() {
                    self.emitter.push_line(&[]);
                    continue;
                }
                let span = match offsets.get(index).copied().flatten() {
                    Some(offset) => {
                        Span::mapped(style, line, SourceMap::Verbatim(offset))
                    }
                    None => Span::new(style, line),
                };
                self.emitter.push_line(&[span]);
            }
        }
        self.emitter.set_prefix(old);
    }

    /// The source byte offset of every line of a code block's content.
    ///
    /// `comrak` strips the block indentation required by the fence or the
    /// CommonMark indented-code rule, so a content line is a suffix of its
    /// source line. Lines that cannot be located map to `None`.
    fn code_line_offsets(
        &self,
        node: Node<'_>,
        code: &comrak::nodes::NodeCodeBlock,
        content: &str,
    ) -> Vec<Option<usize>> {
        let span = node_span(node);
        let start = span.start.line.saturating_sub(1);
        let first = if code.fenced { start + 1 } else { start };
        content
            .split('\n')
            .enumerate()
            .map(|(index, line)| {
                let line_index = first + index;
                self.source.line_info(line_index).and_then(|info| {
                    literal_line_offset(self.source, info.range(), line)
                })
            })
            .collect()
    }

    fn render_html_block(
        &mut self,
        node: Node<'_>,
        block: &comrak::nodes::NodeHtmlBlock,
    ) {
        if self.options.strip_html {
            return;
        }
        let id = self.begin_block(node, BlockKind::Html);
        self.emitter.set_context(LineKind::Html, Some(id));
        let style = Style::default().dim();
        let range = self.source.span_range(node_span(node));
        let slice = self.source.slice(range.clone());
        let literal = block.literal.trim_end_matches('\n');
        let mut search = 0usize;
        for line in literal.split('\n') {
            let offset = if line.is_empty() {
                None
            } else {
                slice[search..]
                    .find(line)
                    .map(|offset| range.start + search + offset)
            };
            if line.is_empty() {
                self.emitter.push_line(&[]);
            } else if let Some(offset) = offset {
                search = offset - range.start + line.len();
                self.emitter.push_line(&[Span::mapped(
                    style,
                    line,
                    SourceMap::Verbatim(offset),
                )]);
            } else {
                self.emitter.push_line(&[Span::new(style, line)]);
            }
        }
    }

    fn render_list(&mut self, node: Node<'_>, list: &comrak::nodes::NodeList) {
        self.emitter.end_line();
        let items: Vec<Node<'_>> = node
            .children()
            .filter(|child| {
                matches!(
                    child.data().value,
                    NodeValue::Item(_) | NodeValue::TaskItem(_)
                )
            })
            .collect();
        let loose = !list.tight;
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                if loose {
                    self.emitter.blank();
                } else {
                    self.emitter.end_line();
                }
            }
            self.render_item(item, list, index, !list.tight);
        }
        self.emitter.end_line();
    }

    fn render_item(
        &mut self,
        item: Node<'_>,
        list: &comrak::nodes::NodeList,
        index: usize,
        loose: bool,
    ) {
        let marker = match list.list_type {
            comrak::nodes::ListType::Bullet => {
                if self.options.color {
                    "•".to_string()
                } else {
                    "-".to_string()
                }
            }
            comrak::nodes::ListType::Ordered => format!("{}.", list.start + index),
        };
        // The marker is its own line when the item's first child is another
        // block; do not stamp it with the previous block's context.
        self.emitter.set_context(LineKind::Text, None);
        self.emitter
            .push_text(Style::default(), &format!("{marker} "));
        let mut marker_width = mdk_core::text::display_width(&marker) + 1;

        if let NodeValue::TaskItem(task) = &item.data().value {
            let (text, style) = if task.symbol.is_some() {
                ("[x] ", Style::default().fg(Color::Green))
            } else {
                ("[ ] ", Style::default().dim())
            };
            self.emitter.push_text(style, text);
            marker_width += 4;
        }

        let mut prefix = self.emitter.prefix().to_vec();
        prefix.push(Span::plain(" ".repeat(marker_width)));
        let old = self.emitter.set_prefix(prefix);
        self.render_blocks(item, loose);
        self.emitter.set_prefix(old);
    }

    fn render_table(&mut self, node: Node<'_>, alignments: Vec<TableAlignment>) {
        let block = self.begin_block(node, BlockKind::Table);
        let rows: Vec<Vec<Vec<Span>>> = node
            .children()
            .filter_map(|row| match &row.data().value {
                NodeValue::TableRow(_) => Some(
                    row.children()
                        .filter(|cell| {
                            matches!(cell.data().value, NodeValue::TableCell)
                        })
                        .map(|cell| self.inline_spans(cell, Style::default()))
                        .collect(),
                ),
                _ => None,
            })
            .collect();
        let chars = if self.options.color {
            TableChars::unicode()
        } else {
            TableChars::ascii()
        };
        let prefix = self.emitter.prefix().to_vec();
        if let Some(lines) =
            table::render_table(&rows, &alignments, self.options.width, &prefix, &chars)
        {
            for line in lines {
                if line.border {
                    self.emitter.set_context(LineKind::TableBorder, Some(block));
                    self.emitter.push_line(&line.spans);
                } else {
                    self.emitter.set_context(LineKind::TableRow, Some(block));
                    self.emitter.push_line_with(
                        &line.spans,
                        line.cells,
                        line.continuation,
                    );
                }
            }
            return;
        }
        // Fallback: render cells as a paragraph line.
        self.emitter.set_context(LineKind::TableRow, Some(block));
        for (index, row) in rows.iter().enumerate() {
            if index > 0 {
                self.emitter.newline();
            }
            for (cell_index, cell) in row.iter().enumerate() {
                if cell_index > 0 {
                    self.emitter.push_text(Style::default().dim(), " | ");
                }
                self.emitter.push_spans(cell);
            }
        }
        self.emitter.end_line();
    }

    fn render_footnotes(&mut self, root: Node<'_>) {
        let mut definitions: Vec<(usize, Node<'_>)> = Vec::new();
        for node in root.descendants() {
            if let NodeValue::FootnoteDefinition(definition) = &node.data().value {
                let number = self
                    .footnote_numbers
                    .get(&definition.name)
                    .copied()
                    .unwrap_or(1);
                definitions.push((number, node));
            }
        }
        if definitions.is_empty() {
            return;
        }
        definitions.sort_by_key(|(number, _)| *number);
        self.emitter.blank();
        for (number, node) in definitions {
            self.emitter.blank();
            self.emitter.set_context(LineKind::Text, None);
            self.emitter
                .push_text(Style::default().fg(Color::Cyan), &format!("[{number}] "));
            let mut prefix = self.emitter.prefix().to_vec();
            prefix.push(Span::plain(" ".repeat(format!("[{number}] ").len())));
            let old = self.emitter.set_prefix(prefix);
            self.render_blocks(node, true);
            self.emitter.set_prefix(old);
        }
    }

    fn inline_spans(&self, node: Node<'_>, style: Style) -> Vec<Span> {
        let mut spans = Vec::new();
        for child in node.children() {
            self.collect_inline(child, style, &mut spans);
        }
        spans
    }

    fn collect_inline(&self, node: Node<'_>, style: Style, out: &mut Vec<Span>) {
        match &node.data().value {
            NodeValue::Text(text) => {
                out.push(Span::mapped(
                    style,
                    text.as_ref(),
                    self.literal_map(node, text.as_ref()),
                ));
            }
            NodeValue::SoftBreak => out.push(Span::plain(" ")),
            NodeValue::LineBreak => out.push(Span::plain("\n")),
            NodeValue::Code(code) => out.push(Span::atomic_mapped(
                style.fg(Color::BrightYellow),
                &code.literal,
                self.literal_map(node, &code.literal),
            )),
            NodeValue::Emph => self.collect_children(node, style.italic(), out),
            NodeValue::Strong => self.collect_children(node, style.bold(), out),
            NodeValue::Strikethrough => {
                self.collect_children(node, style.strikethrough(), out);
            }
            NodeValue::Highlight => {
                self.collect_children(node, style.bg(Color::Yellow), out)
            }
            NodeValue::Insert | NodeValue::Underline => {
                self.collect_children(node, style.underline(), out);
            }
            NodeValue::Superscript | NodeValue::Subscript => {
                self.collect_children(node, style.dim(), out);
            }
            NodeValue::Link(link) => {
                let link_style = style.fg(Color::BrightBlue).underline();
                self.collect_children(node, link_style, out);
                let text = collect_text(node);
                self.push_destination(node, style, &text, &link.url, out);
            }
            NodeValue::Image(link) => {
                self.collect_image(node, style, link, out);
            }
            NodeValue::FootnoteReference(reference) => {
                let number = self
                    .footnote_numbers
                    .get(&reference.name)
                    .copied()
                    .unwrap_or(reference.ix as usize);
                out.push(Span::atomic(style.fg(Color::Cyan), format!("[{number}]")));
            }
            NodeValue::HtmlInline(html) => {
                if !self.options.strip_html {
                    out.push(Span::atomic_mapped(
                        style.dim(),
                        html,
                        self.literal_map(node, html),
                    ));
                }
            }
            NodeValue::Math(math) => out.push(Span::atomic_mapped(
                style.fg(Color::Magenta),
                &math.literal,
                self.literal_map(node, &math.literal),
            )),
            NodeValue::Escaped | NodeValue::EscapedTag(_) => {}
            _ => self.collect_children(node, style, out),
        }
    }

    fn collect_children(&self, node: Node<'_>, style: Style, out: &mut Vec<Span>) {
        for child in node.children() {
            self.collect_inline(child, style, out);
        }
    }

    /// The source map for a node whose rendered text is `literal`.
    ///
    /// The literal is usually a verbatim slice of the node's source range;
    /// inline code is located inside its backticks and text that decodes
    /// escapes or entities is aligned character by character.
    fn literal_map(&self, node: Node<'_>, literal: &str) -> SourceMap {
        let range = self.source.span_range(node_span(node));
        let slice = self.source.slice(range.clone());
        if slice == literal {
            return SourceMap::Verbatim(range.start);
        }
        if let Some(offset) = slice.find(literal) {
            return SourceMap::Verbatim(range.start + offset);
        }
        SourceMap::PerChar(Arc::from(align_text(slice, literal)))
    }

    /// Appends a link destination after its text, if the link text is not
    /// the URL itself. The URL keeps its own source mapping so a reader can
    /// select or copy it.
    fn push_destination(
        &self,
        node: Node<'_>,
        style: Style,
        text: &str,
        url: &str,
        out: &mut Vec<Span>,
    ) {
        if url.is_empty() || text.trim() == url {
            return;
        }
        out.push(Span::plain(" "));
        out.push(self.destination_span(node, style, url));
    }

    fn collect_image(
        &self,
        node: Node<'_>,
        style: Style,
        link: &comrak::nodes::NodeLink,
        out: &mut Vec<Span>,
    ) {
        const PREFIX: &str = "image: ";
        let image_style = style.fg(Color::BrightBlue).underline();
        let alt = collect_text(node);
        let trimmed = alt.trim();
        let label = if trimmed.is_empty() {
            "image".to_string()
        } else {
            format!("{PREFIX}{trimmed}")
        };
        let mut ranges: Vec<Option<Range<usize>>> = Vec::new();
        ranges.push(None);
        if trimmed.is_empty() {
            ranges.extend(std::iter::repeat_n(None, "image".chars().count()));
        } else {
            ranges.extend(std::iter::repeat_n(None, PREFIX.chars().count()));
        }
        match self.locate_after(node, "![", trimmed) {
            SourceMap::Verbatim(start) => {
                for (index, ch) in trimmed.char_indices() {
                    ranges.push(Some(start + index..start + index + ch.len_utf8()));
                }
            }
            _ => ranges.extend(std::iter::repeat_n(None, trimmed.chars().count())),
        }
        ranges.push(None);
        out.push(Span::atomic_mapped(
            image_style,
            format!("[{label}]"),
            SourceMap::PerChar(Arc::from(ranges)),
        ));
        if !link.url.is_empty() {
            out.push(Span::plain(" "));
            out.push(self.destination_span(node, style, &link.url));
        }
    }

    /// Locates `literal` in the node's range after `marker`.
    fn locate_after(&self, node: Node<'_>, marker: &str, literal: &str) -> SourceMap {
        let range = self.source.span_range(node_span(node));
        let slice = self.source.slice(range.clone());
        let Some(start) = slice.find(marker) else {
            return SourceMap::None;
        };
        let tail = &slice[start + marker.len()..];
        tail.find(literal).map_or(SourceMap::None, |offset| {
            SourceMap::Verbatim(range.start + start + marker.len() + offset)
        })
    }

    /// The byte offset of a link or image destination inside its node.
    fn destination_offset(&self, node: Node<'_>, url: &str) -> Option<usize> {
        if url.is_empty() {
            return None;
        }
        let range = self.source.span_range(node_span(node));
        let slice = self.source.slice(range.clone());
        let start = slice.rfind("](").or_else(|| slice.rfind("]:"))?;
        let tail = &slice[start + 2..];
        tail.find(url)
            .map(|offset| range.start + start + 2 + offset)
    }

    /// An atomic `(url)` span, mapped so the URL can be selected or copied.
    fn destination_span(&self, node: Node<'_>, style: Style, url: &str) -> Span {
        let mut ranges: Vec<Option<Range<usize>>> = Vec::new();
        ranges.push(None);
        match self.destination_offset(node, url) {
            Some(offset) => {
                for (index, ch) in url.char_indices() {
                    ranges.push(Some(offset + index..offset + index + ch.len_utf8()));
                }
            }
            None => ranges.extend(std::iter::repeat_n(None, url.chars().count())),
        }
        ranges.push(None);
        Span::atomic_mapped(
            style.dim().fg(Color::BrightBlack),
            format!("({url})"),
            SourceMap::PerChar(Arc::from(ranges)),
        )
    }
}

/// The offset of `literal` inside a source line that may carry the block
/// indentation that `comrak` stripped.
///
/// Returns `None` when the literal cannot be located in the line, for example
/// when `comrak` expanded a tab while stripping indentation. Callers map such
/// lines without source information instead of pointing at the wrong bytes.
fn literal_line_offset(
    source: &Source,
    range: Range<usize>,
    literal: &str,
) -> Option<usize> {
    if literal.is_empty() {
        return Some(range.start);
    }
    let text = source.slice(range.clone());
    if text.ends_with(literal) {
        return Some(range.end - literal.len());
    }
    let trimmed = text.trim_end();
    if trimmed.ends_with(literal) {
        return Some(range.start + trimmed.len() - literal.len());
    }
    None
}

/// Aligns a decoded `literal` with the raw `source` slice it was parsed from,
/// returning a source range for every character of the literal.
///
/// Characters that match line up one to one; backslash escapes and character
/// entities map back to the character or entity they came from, and a run of
/// whitespace in the source (a folded soft break) maps to a single space.
fn align_text(raw: &str, literal: &str) -> Vec<Option<Range<usize>>> {
    let mut out = Vec::new();
    let mut source = raw.char_indices().peekable();
    for ch in literal.chars() {
        let Some((offset, source_ch)) = source.next() else {
            out.push(None);
            continue;
        };
        if source_ch == '&'
            && let Some(len) = entity_len(&raw[offset..])
        {
            while let Some((next_offset, _)) = source.peek().copied() {
                if next_offset >= offset + len {
                    break;
                }
                source.next();
            }
            out.push(Some(offset..offset + len));
            continue;
        }
        if ch == source_ch {
            out.push(Some(offset..offset + source_ch.len_utf8()));
            continue;
        }
        if source_ch == '\\'
            && let Some((next_offset, next_ch)) = source.peek().copied()
            && next_ch == ch
        {
            source.next();
            out.push(Some(next_offset..next_offset + next_ch.len_utf8()));
            continue;
        }
        if source_ch.is_whitespace() && ch.is_whitespace() {
            let start = offset;
            let mut end = offset + source_ch.len_utf8();
            while let Some((next_offset, next_ch)) = source.peek().copied() {
                if next_ch.is_whitespace() {
                    end = next_offset + next_ch.len_utf8();
                    source.next();
                } else {
                    break;
                }
            }
            out.push(Some(start..end));
            continue;
        }
        out.push(Some(offset..offset + source_ch.len_utf8()));
    }
    out
}

/// The byte length of a well-formed HTML character entity at the start of
/// `text`, including the leading `&` and the trailing `;`.
fn entity_len(text: &str) -> Option<usize> {
    if !text.starts_with('&') {
        return None;
    }
    let mut len = 1;
    for ch in text[1..].chars() {
        len += ch.len_utf8();
        if ch == ';' {
            return (len > 2).then_some(len);
        }
        if !(ch.is_ascii_alphanumeric() || ch == '#') {
            return None;
        }
    }
    None
}

fn heading_style(level: u8) -> (Style, usize) {
    match level {
        1 => (
            Style {
                bold: true,
                underline: true,
                ..Style::default()
            },
            1,
        ),
        2 => (Style::default().bold().fg(Color::BrightBlue), 2),
        3 => (Style::default().bold().fg(Color::BrightCyan), 3),
        other => (
            Style::default().bold().fg(Color::BrightYellow),
            usize::from(other),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term::model::Continuation;

    fn ansi(markdown: &str) -> String {
        render_terminal(markdown, Dialect::Gfm, &TermOptions::default())
    }

    fn plain(markdown: &str) -> String {
        render_terminal(markdown, Dialect::Gfm, &TermOptions::plain(60))
    }

    fn document(markdown: &str) -> Document {
        render_terminal_document(markdown, Dialect::Gfm, &TermOptions::default())
    }

    /// Plain layout with the same line structure as [`document`], but ASCII
    /// markers and raw HTML kept so assertions are flavour independent.
    fn plain_document(markdown: &str) -> Document {
        render_terminal_document(
            markdown,
            Dialect::Gfm,
            &TermOptions {
                color: false,
                strip_html: false,
                ..TermOptions::plain(60)
            },
        )
    }

    fn code_line_span<'a>(line: &'a Line, text: &str) -> &'a Span {
        line.spans
            .iter()
            .find(|span| span.text == text)
            .unwrap_or_else(|| {
                panic!("missing code span {text:?} in {:?}", line.text())
            })
    }

    #[test]
    fn strips_ansi_for_non_tty() {
        let output = plain("# Title\n\nHello *world*.\n");
        assert!(!output.contains('\u{1b}'));
        assert!(output.contains("# Title"));
        assert!(output.contains("Hello world."));
    }

    #[test]
    fn renders_lists_and_tasks() {
        let output = plain("- one\n- [x] two\n");
        assert!(output.contains("- one"));
        assert!(output.contains("[x] two"));
    }

    #[test]
    fn renders_code_with_indent() {
        let output = plain("```\nlet x = 1;\n```\n");
        assert!(output.contains("    let x = 1;"));
    }

    #[test]
    fn renders_tables() {
        let output = plain("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(output.contains("| a"));
        assert!(output.contains("+--"));
    }

    #[test]
    fn renders_links_with_destination() {
        let output = plain("[text](https://example.com)\n");
        assert!(output.contains("text (https://example.com)"));
    }

    #[test]
    fn renders_footnotes_at_the_end() {
        let output = plain("note[^a]\n\n[^a]: body\n");
        assert!(output.contains("note[1]"));
        assert!(output.contains("[1] body"));
    }

    #[test]
    fn ansi_contains_escape_sequences() {
        let output = ansi("**bold**\n");
        assert!(output.contains("\u{1b}[1m"));
    }

    #[test]
    fn headings_carry_their_level_and_source() {
        let doc = document("# Title\n");
        let line = &doc.lines[0];
        assert_eq!(line.kind, LineKind::Heading(1));
        assert_eq!(line.text(), "Title");
        let span = &line.spans[0];
        assert_eq!(span.text, "Title");
        assert_eq!(span.source, SourceMap::Verbatim(2));
        assert_eq!(doc.blocks[0].kind, BlockKind::Heading(1));
        assert_eq!(doc.blocks[0].range, 0..7);
    }

    #[test]
    fn paragraphs_map_through_inline_markup() {
        let doc = document("Hello **bold** and `code`.\n");
        let spans = &doc.lines[0].spans;
        let text: Vec<(&str, &SourceMap)> = spans
            .iter()
            .map(|span| (span.text.as_str(), &span.source))
            .collect();
        assert_eq!(
            text,
            vec![
                ("Hello", &SourceMap::Verbatim(0)),
                (" ", &SourceMap::None),
                ("bold", &SourceMap::Verbatim(8)),
                (" ", &SourceMap::None),
                ("and", &SourceMap::Verbatim(15)),
                (" ", &SourceMap::None),
                ("code", &SourceMap::Verbatim(20)),
                (".", &SourceMap::Verbatim(25)),
            ]
        );
    }

    #[test]
    fn escapes_use_per_character_maps() {
        let doc = document("a\\*b\n");
        let line = &doc.lines[0];
        assert_eq!(line.text(), "a*b");
        let span = line
            .spans
            .iter()
            .find(|span| span.text == "a*b")
            .expect("text span");
        assert_eq!(
            span.source,
            SourceMap::PerChar(Arc::from(vec![Some(0..1), Some(2..3), Some(3..4)]))
        );
    }

    #[test]
    fn wrapped_lines_keep_offsets_and_continuation() {
        let options = TermOptions {
            width: 10,
            ..TermOptions::default()
        };
        let doc =
            render_terminal_document("one two three four\n", Dialect::Gfm, &options);
        assert!(doc.lines.len() > 1);
        assert_eq!(doc.lines[0].continuation, Continuation::None);
        assert_eq!(doc.lines[1].continuation, Continuation::Space);
        assert_eq!(doc.lines[0].spans[0].source, SourceMap::Verbatim(0));
        assert_eq!(doc.lines[1].spans[0].text, "three");
        assert_eq!(doc.lines[1].spans[0].source, SourceMap::Verbatim(8));
    }

    #[test]
    fn code_lines_map_to_the_content_after_the_fence() {
        let doc = document("```rust\nlet x = 1;\n  indented\n```\n");
        let code: Vec<&Line> = doc
            .lines
            .iter()
            .filter(|line| line.kind == LineKind::Code)
            .collect();
        assert_eq!(code.len(), 2);
        let first = code_line_span(code[0], "let x = 1;");
        assert_eq!(first.source, SourceMap::Verbatim(8));
        let second = code_line_span(code[1], "  indented");
        assert_eq!(second.source, SourceMap::Verbatim(19));
        assert_eq!(doc.blocks[0].kind, BlockKind::Code);
        assert_eq!(doc.blocks[0].range, 0..33);
    }

    #[test]
    fn indented_code_strips_the_block_indentation() {
        let doc = document("    let x = 1;\n");
        let line = doc
            .lines
            .iter()
            .find(|line| line.kind == LineKind::Code)
            .expect("code line");
        let span = code_line_span(line, "let x = 1;");
        assert_eq!(span.source, SourceMap::Verbatim(4));
    }

    #[test]
    fn code_blocks_preserve_blank_lines() {
        let doc = document("```\na\n\n\nb\n```\n");
        let code: Vec<&Line> = doc
            .lines
            .iter()
            .filter(|line| line.kind == LineKind::Code)
            .collect();
        assert_eq!(code.len(), 4);
        assert!(code[1].spans.is_empty());
        assert!(code[2].spans.is_empty());
        assert_eq!(code[1].block, code[0].block);
    }

    #[test]
    fn table_rows_expose_cells_and_header() {
        let doc = document("| a | b |\n|---|---|\n| 1 | 2 |\n");
        let rows: Vec<&Line> = doc
            .lines
            .iter()
            .filter(|line| line.kind == LineKind::TableRow)
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].cells.len(), 2);
        assert!(rows[0].cells[0].header);
        assert!(!rows[1].cells[0].header);
        assert_eq!(rows[0].cells[0].columns, 1..6);
        assert_eq!(rows[0].cells[1].columns, 7..12);
        let cell: String = rows[0]
            .spans
            .iter()
            .filter(|span| span.text.trim() == "a")
            .map(|span| span.text.as_str())
            .collect();
        assert_eq!(cell, "a");
        let text_span = rows[0]
            .spans
            .iter()
            .find(|span| span.text == "a")
            .expect("cell text");
        assert_eq!(text_span.source, SourceMap::Verbatim(2));
    }

    #[test]
    fn front_matter_is_skipped_but_offsets_stay() {
        let doc = document("---\ntitle: x\n---\n\n# Hi\n");
        assert_eq!(doc.lines[0].text(), "Hi");
        assert_eq!(doc.lines[0].spans[0].source, SourceMap::Verbatim(20));
    }

    #[test]
    fn list_items_map_to_their_text() {
        let doc = document("- one\n- two\n");
        assert_eq!(doc.lines[0].text(), "• one");
        let text = doc.lines[0]
            .spans
            .iter()
            .find(|span| span.text == "one")
            .expect("item text");
        assert_eq!(text.source, SourceMap::Verbatim(2));
    }

    #[test]
    fn link_destinations_are_mapped() {
        let doc = document("[text](https://example.com)\n");
        let span = doc.lines[0]
            .spans
            .iter()
            .find(|span| span.text == "(https://example.com)")
            .expect("destination span");
        assert!(span.atomic);
        let url = "https://example.com";
        assert_eq!(span.source.char_range(&span.text, 0), None);
        assert_eq!(
            span.source.range_of(&span.text, 1..1 + url.chars().count()),
            Some(7..26)
        );
        assert_eq!(
            span.source.char_range(&span.text, 1 + url.chars().count()),
            None
        );
    }

    #[test]
    fn align_text_maps_escapes_and_entities() {
        assert_eq!(
            align_text("a\\*b", "a*b"),
            vec![Some(0..1), Some(2..3), Some(3..4)]
        );
        assert_eq!(
            align_text("a &amp; b", "a & b"),
            vec![Some(0..1), Some(1..2), Some(2..7), Some(7..8), Some(8..9),]
        );
        assert_eq!(
            align_text("a\nb", "a b"),
            vec![Some(0..1), Some(1..2), Some(2..3)]
        );
    }

    #[test]
    fn literal_line_offset_handles_indentation() {
        let source = Source::new("    let x = 1;\n".to_string());
        let range = source.line_range(0);
        assert_eq!(literal_line_offset(&source, range, "let x = 1;"), Some(4));
        let source = Source::new("  let x = 1;  \n".to_string());
        let range = source.line_range(0);
        assert_eq!(literal_line_offset(&source, range, "let x = 1;"), Some(2));
        // A tab expanded by comrak cannot be located; the line stays unmapped.
        let source = Source::new("> \t\tfoo\n".to_string());
        let range = source.line_range(0);
        assert_eq!(literal_line_offset(&source, range, "  foo"), None);
    }

    #[test]
    fn code_blocks_keep_a_leading_blank_line() {
        let doc = document("# Title\n\n```rust\n\nlet x = 1;\n```\n");
        let code: Vec<&Line> = doc
            .lines
            .iter()
            .filter(|line| line.kind == LineKind::Code)
            .collect();
        // The leading blank line is part of the code block, not only the
        // block separator before it.
        assert_eq!(code.len(), 2);
        assert!(code[0].spans.is_empty());
        assert_eq!(code[0].block, code[1].block);
        assert!(code[1].text().contains("let x = 1;"));
        let first = doc
            .lines
            .iter()
            .position(|line| line.kind == LineKind::Code)
            .expect("code");
        assert_eq!(doc.lines[first - 1].kind, LineKind::Blank);
        assert_eq!(doc.lines[first - 1].block, None);
    }

    #[test]
    fn html_block_blank_lines_stay_empty() {
        let doc = plain_document("> <pre>\n>\n> x\n> </pre>\n");
        let texts: Vec<String> = doc.lines.iter().map(Line::text).collect();
        assert!(texts.iter().any(|text| text == "> x"), "{texts:?}");
        assert!(
            !texts.iter().any(|text| text == "> "),
            "empty HTML lines must not gain the container prefix: {texts:?}"
        );
    }

    #[test]
    fn nested_list_marker_has_no_stale_context() {
        let doc = plain_document("# H\n\n- - x\n");
        let marker = doc
            .lines
            .iter()
            .find(|line| line.text() == "-")
            .expect("outer marker line");
        assert_eq!(marker.kind, LineKind::Text);
        assert_eq!(marker.block, None);
    }

    #[test]
    fn image_labels_map_every_character() {
        for markdown in ["![alt](i.png)\n", "![中文说明](i.png)\n", "![](i.png)\n"]
        {
            let source = Source::new(markdown.to_string());
            let doc = document(markdown);
            for line in &doc.lines {
                for span in &line.spans {
                    if let SourceMap::PerChar(ranges) = &span.source {
                        assert_eq!(
                            ranges.len(),
                            span.text.chars().count(),
                            "{:?}",
                            span.text
                        );
                        for range in ranges.iter().flatten() {
                            assert!(range.end <= source.text().len());
                            assert!(source.text().is_char_boundary(range.start));
                            assert!(source.text().is_char_boundary(range.end));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn multibyte_maps_stay_in_bounds() {
        let markdown = "中文 &amp; 文字\n";
        let source = Source::new(markdown.to_string());
        let doc = document(markdown);
        for line in &doc.lines {
            for span in &line.spans {
                match &span.source {
                    SourceMap::Verbatim(start) => {
                        let end = start + span.text.len();
                        assert!(end <= source.text().len());
                        assert_eq!(&source.text()[*start..end], span.text);
                    }
                    SourceMap::PerChar(ranges) => {
                        assert_eq!(ranges.len(), span.text.chars().count());
                        for range in ranges.iter().flatten() {
                            assert!(range.end <= source.text().len());
                        }
                    }
                    SourceMap::None => {}
                }
            }
        }
    }

    #[test]
    fn verbatim_maps_match_the_source_in_repository_docs() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = vec![root.join("README.md")];
        for directory in [root.join("docs"), root.join("crates/mdk-fmt/tests/corpus")] {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|extension| extension == "md") {
                    files.push(path);
                }
            }
        }
        for file in files {
            let Ok(markdown) = std::fs::read_to_string(&file) else {
                continue;
            };
            let source = Source::new(markdown.clone());
            let doc = render_terminal_document(
                &markdown,
                Dialect::Gfm,
                &TermOptions::default(),
            );
            for line in &doc.lines {
                for span in &line.spans {
                    if let SourceMap::Verbatim(start) = &span.source {
                        let end = start + span.text.len();
                        assert!(
                            end <= source.text().len()
                                && source.text().is_char_boundary(*start)
                                && source.text().is_char_boundary(end),
                            "{}: span {:?} has invalid range {start}..{end}",
                            file.display(),
                            span.text
                        );
                        assert_eq!(
                            &source.text()[*start..end],
                            span.text,
                            "{}: Verbatim slice mismatch for {:?}",
                            file.display(),
                            span.text
                        );
                    }
                }
            }
        }
    }
}
