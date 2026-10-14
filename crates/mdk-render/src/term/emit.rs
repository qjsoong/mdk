//! Text styling and the line emitter used by the terminal renderer.

use std::ops::Range;

use mdk_core::text::display_width;

use super::model::{Cell, Continuation, Line, LineKind, Span, Style};

/// Wraps spans to `limit` columns, returning lines of spans.
pub(crate) fn wrap_spans(spans: &[Span], limit: usize) -> Vec<Vec<Span>> {
    let limit = limit.max(1);
    let mut lines: Vec<Vec<Span>> = Vec::new();
    let mut current: Vec<Span> = Vec::new();
    let mut width = 0usize;

    let mut space_before = false;
    for span in spans {
        for (kind, chars) in tokens(span) {
            match kind {
                TokenKind::Space => space_before = true,
                TokenKind::Newline => {
                    lines.push(std::mem::take(&mut current));
                    width = 0;
                    space_before = false;
                }
                TokenKind::Word => {
                    let word = span.slice(chars);
                    let word_width = word.width();
                    let separator = usize::from(space_before && !current.is_empty());
                    if current.is_empty() {
                        if word_width > limit {
                            push_split_word(
                                &mut lines,
                                &mut current,
                                &word,
                                limit,
                                &mut width,
                            );
                        } else {
                            current.push(word);
                            width = word_width;
                        }
                    } else if width + separator + word_width <= limit {
                        if space_before {
                            current.push(Span::plain(" "));
                        }
                        current.push(word);
                        width += separator + word_width;
                    } else {
                        lines.push(std::mem::take(&mut current));
                        if word_width > limit {
                            push_split_word(
                                &mut lines,
                                &mut current,
                                &word,
                                limit,
                                &mut width,
                            );
                        } else {
                            current.push(word);
                            width = word_width;
                        }
                    }
                    space_before = false;
                }
            }
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

/// Splits a word wider than `limit` across lines, leaving the final chunk in
/// `current`. Used for table cells, where a single long token must still fit
/// the column. Chunks keep the word's source mapping through `Span::slice`.
fn push_split_word(
    lines: &mut Vec<Vec<Span>>,
    current: &mut Vec<Span>,
    word: &Span,
    limit: usize,
    width: &mut usize,
) {
    let total = word.text.chars().count();
    let mut start = 0usize;
    let mut chunk_width = 0usize;
    for (index, ch) in word.text.chars().enumerate() {
        let char_width = display_width(&ch.to_string());
        if chunk_width > 0 && chunk_width + char_width > limit {
            lines.push(vec![word.slice(start..index)]);
            start = index;
            chunk_width = 0;
        }
        chunk_width += char_width;
    }
    current.push(word.slice(start..total));
    *width = chunk_width;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Space,
    Newline,
    Word,
}

/// Splits a span into word, whitespace and newline tokens, returning the
/// character range of each token within the span text.
fn tokens(span: &Span) -> Vec<(TokenKind, Range<usize>)> {
    let characters = span.text.chars().count();
    if span.atomic {
        return vec![(TokenKind::Word, 0..characters)];
    }
    let mut out: Vec<(TokenKind, Range<usize>)> = Vec::new();
    let mut word_start: Option<usize> = None;
    let mut index = 0usize;
    for ch in span.text.chars() {
        match ch {
            '\n' => {
                if let Some(start) = word_start.take() {
                    out.push((TokenKind::Word, start..index));
                }
                out.push((TokenKind::Newline, index..index + 1));
            }
            ch if ch.is_whitespace() => {
                if let Some(start) = word_start.take() {
                    out.push((TokenKind::Word, start..index));
                }
                if !matches!(out.last(), Some((TokenKind::Space, _))) {
                    out.push((TokenKind::Space, index..index + 1));
                }
            }
            _ => {
                if word_start.is_none() {
                    word_start = Some(index);
                }
            }
        }
        index += 1;
    }
    if let Some(start) = word_start {
        out.push((TokenKind::Word, start..index));
    }
    out
}

/// Accumulates styled lines with a shared prefix.
pub(crate) struct Emitter {
    lines: Vec<Line>,
    current: Vec<Span>,
    width: usize,
    has_content: bool,
    max_width: usize,
    prefix: Vec<Span>,
    prefix_width: usize,
    pending_space: bool,
    kind: LineKind,
    block: Option<usize>,
    continuation: Continuation,
    next_continuation: Continuation,
}

impl Emitter {
    pub fn new(max_width: usize, prefix: Vec<Span>) -> Self {
        let prefix_width = columns(&prefix);
        Self {
            lines: Vec::new(),
            current: Vec::new(),
            width: 0,
            has_content: false,
            max_width: max_width.max(1),
            prefix,
            prefix_width,
            pending_space: false,
            kind: LineKind::Text,
            block: None,
            continuation: Continuation::None,
            next_continuation: Continuation::None,
        }
    }

    pub fn set_prefix(&mut self, prefix: Vec<Span>) -> Vec<Span> {
        let old = std::mem::replace(&mut self.prefix, prefix);
        self.prefix_width = columns(&self.prefix);
        old
    }

    pub fn prefix(&self) -> &[Span] {
        &self.prefix
    }

    pub fn content_width(&self) -> usize {
        self.max_width.saturating_sub(self.prefix_width)
    }

    /// Sets the block metadata stamped onto the lines that follow. The
    /// metadata applies to the line being built as well, so a list marker
    /// pushed before its paragraph belongs to the paragraph's block.
    pub fn set_context(&mut self, kind: LineKind, block: Option<usize>) {
        self.kind = kind;
        self.block = block;
    }

    /// Appends wrapped, whitespace-separated text.
    pub fn push_text(&mut self, style: Style, text: &str) {
        self.push_spans(&[Span::new(style, text)]);
    }

    /// Appends spans, wrapping at word boundaries.
    pub fn push_spans(&mut self, spans: &[Span]) {
        for span in spans {
            for (kind, chars) in tokens(span) {
                match kind {
                    TokenKind::Space => self.pending_space = true,
                    TokenKind::Newline => self.newline(),
                    TokenKind::Word => self.push_word(&span.slice(chars)),
                }
            }
        }
    }

    fn push_word(&mut self, word: &Span) {
        self.start_line();
        let word_width = word.width();
        let separator = usize::from(self.pending_space && self.has_content);
        if self.has_content && self.width + separator + word_width > self.max_width {
            self.next_continuation = if self.pending_space {
                Continuation::Space
            } else {
                Continuation::Tight
            };
            self.newline();
            self.start_line();
        }
        if self.pending_space && self.has_content {
            self.append(Span::plain(" "));
        }
        self.pending_space = false;
        self.append(word.clone());
        self.has_content = true;
    }

    /// Appends one line verbatim, without wrapping. Empty lines are kept.
    pub fn push_line(&mut self, spans: &[Span]) {
        self.push_line_with(spans, Vec::new(), Continuation::None);
    }

    /// Appends one line verbatim, recording how it continues the previous
    /// line and the display columns of its table cells.
    pub fn push_line_with(
        &mut self,
        spans: &[Span],
        cells: Vec<Cell>,
        continuation: Continuation,
    ) {
        if spans.is_empty() {
            self.lines.push(Line {
                spans: Vec::new(),
                kind: self.kind,
                block: self.block,
                continuation,
                cells,
            });
            self.reset_line();
            return;
        }
        self.start_line();
        for span in spans {
            self.append(span.clone());
        }
        self.has_content = true;
        self.pending_space = false;
        self.continuation = continuation;
        self.flush_line_with(cells);
        self.next_continuation = Continuation::None;
        self.reset_line();
    }

    /// Ends the current line, inserting a blank line when it was already
    /// ended. Used by soft structural breaks such as inline hard breaks.
    pub fn newline(&mut self) {
        if !self.current.is_empty() {
            self.flush_line();
        } else if !self.lines.is_empty()
            && !self.lines.last().is_some_and(Line::is_blank)
        {
            self.lines.push(Line::blank());
        }
        self.reset_line();
    }

    /// Ends the current line if it has content; a no-op for an empty line.
    pub fn end_line(&mut self) {
        if !self.current.is_empty() {
            self.flush_line();
        }
        self.reset_line();
    }

    /// Ensures exactly one blank line before the next content.
    pub fn blank(&mut self) {
        if !self.current.is_empty() {
            self.flush_line();
        }
        if !self.lines.is_empty() && !self.lines.last().is_some_and(Line::is_blank) {
            self.lines.push(Line::blank());
        }
        self.reset_line();
    }

    fn start_line(&mut self) {
        if !self.current.is_empty() {
            return;
        }
        self.continuation = std::mem::take(&mut self.next_continuation);
        self.current = self.prefix.clone();
        self.width = self.prefix_width;
    }

    fn flush_line(&mut self) {
        self.flush_line_with(Vec::new());
    }

    fn flush_line_with(&mut self, cells: Vec<Cell>) {
        self.lines.push(Line {
            spans: std::mem::take(&mut self.current),
            kind: self.kind,
            block: self.block,
            continuation: self.continuation,
            cells,
        });
    }

    fn append(&mut self, span: Span) {
        if let Some(last) = self.current.last_mut()
            && last.style == span.style
            && last.atomic == span.atomic
            && last.source.try_concat(last.text.len(), &span.source)
        {
            last.text.push_str(&span.text);
            self.add_width(&span.text);
            return;
        }
        self.add_width(&span.text);
        self.current.push(span);
    }

    fn add_width(&mut self, text: &str) {
        self.width += display_width(text);
    }

    fn reset_line(&mut self) {
        self.width = 0;
        self.has_content = false;
        self.pending_space = false;
        self.continuation = Continuation::None;
    }

    pub fn finish(mut self) -> Vec<Line> {
        if !self.current.is_empty() {
            self.flush_line();
        }
        while self.lines.last().is_some_and(Line::is_blank) {
            self.lines.pop();
        }
        self.lines
    }
}

fn columns(spans: &[Span]) -> usize {
    spans.iter().map(Span::width).sum()
}

/// Encodes rendered lines as ANSI-coloured or plain text.
/// Encodes rendered lines to text, adding ANSI escapes when `color` is set.
///
/// [`render_terminal`](super::render_terminal) re-parses the Markdown and
/// calls this; consumers that already hold a [`Document`](super::Document)
/// can encode it directly.
#[must_use]
pub fn encode(lines: &[Line], color: bool) -> String {
    let mut output = String::new();
    for line in lines {
        for span in &line.spans {
            output.push_str(&encode_span(span, color));
        }
        output.push('\n');
    }
    output
}

fn encode_span(span: &Span, color: bool) -> String {
    if !color || span.style.is_plain() || span.text.is_empty() {
        return span.text.clone();
    }
    let mut codes: Vec<String> = Vec::new();
    if span.style.bold {
        codes.push("1".to_string());
    }
    if span.style.dim {
        codes.push("2".to_string());
    }
    if span.style.italic {
        codes.push("3".to_string());
    }
    if span.style.underline {
        codes.push("4".to_string());
    }
    if span.style.strike {
        codes.push("9".to_string());
    }
    if let Some(fg) = span.style.fg {
        codes.push(fg.fg_code().to_string());
    }
    if let Some(bg) = span.style.bg {
        codes.push(bg.bg_code().to_string());
    }
    format!("\u{1b}[{}m{}\u{1b}[0m", codes.join(";"), span.text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term::model::SourceMap;

    fn text_of(spans: &[Span]) -> String {
        spans.iter().map(|span| span.text.as_str()).collect()
    }

    #[test]
    fn wraps_spans_at_word_boundaries() {
        let spans = [Span::new(Style::default(), "hello world again")];
        let lines = wrap_spans(&spans, 11);
        assert_eq!(lines.len(), 2);
        assert_eq!(text_of(&lines[0]), "hello world");
        assert_eq!(text_of(&lines[1]), "again");
    }

    #[test]
    fn wrapping_keeps_source_offsets() {
        let spans = [Span::mapped(
            Style::default(),
            "hello world",
            SourceMap::Verbatim(100),
        )];
        let lines = wrap_spans(&spans, 5);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0][0].source, SourceMap::Verbatim(100));
        assert_eq!(lines[1][0].source, SourceMap::Verbatim(106));
    }

    #[test]
    fn wrapping_keeps_per_char_maps() {
        let map = SourceMap::PerChar(std::sync::Arc::from(vec![
            Some(0..1),
            Some(1..2),
            Some(2..3),
            Some(3..4),
            Some(4..5),
        ]));
        let spans = [Span::mapped(Style::default(), "ab cd", map)];
        let lines = wrap_spans(&spans, 2);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[1][0].source,
            SourceMap::PerChar(std::sync::Arc::from(vec![Some(3..4), Some(4..5)]))
        );
    }

    #[test]
    fn atomic_spans_prefer_their_own_line() {
        let spans = [Span::atomic(Style::default(), "a b c d e")];
        // Wide enough: the span is kept together.
        assert_eq!(wrap_spans(&spans, 10).len(), 1);
        // Too narrow: the span is split so lines still respect the limit.
        let lines = wrap_spans(&spans, 4);
        assert!(lines.len() > 1);
        for line in &lines {
            let width: usize = line.iter().map(Span::width).sum();
            assert!(width <= 4, "line too wide: {width}");
        }
    }

    #[test]
    fn emitter_encodes_ansi() {
        let mut emitter = Emitter::new(40, Vec::new());
        emitter.push_text(Style::default().bold(), "hi");
        let output = encode(&emitter.finish(), true);
        assert_eq!(output, "\u{1b}[1mhi\u{1b}[0m\n");
    }

    #[test]
    fn prefix_is_repeated_on_wrapped_lines() {
        let mut emitter = Emitter::new(10, vec![Span::plain("> ")]);
        emitter.push_text(Style::default(), "one two three four");
        let output = encode(&emitter.finish(), false);
        assert!(output.starts_with("> one two"));
        assert!(output.contains("\n> "));
    }

    #[test]
    fn wrapped_lines_are_continuations() {
        let mut emitter = Emitter::new(10, Vec::new());
        emitter.push_text(Style::default(), "one two three four");
        let lines = emitter.finish();
        assert_eq!(lines[0].continuation, Continuation::None);
        assert_eq!(lines[1].continuation, Continuation::Space);
    }

    #[test]
    fn a_wrap_without_a_space_is_tight() {
        let mut emitter = Emitter::new(6, Vec::new());
        emitter.push_spans(&[
            Span::mapped(Style::default(), "abcd", SourceMap::Verbatim(0)),
            Span::mapped(Style::default(), ",ef", SourceMap::Verbatim(4)),
        ]);
        let lines = emitter.finish();
        assert_eq!(lines[0].text(), "abcd");
        assert_eq!(lines[1].text(), ",ef");
        assert_eq!(lines[1].continuation, Continuation::Tight);
    }

    #[test]
    fn structural_breaks_are_not_continuations() {
        let mut emitter = Emitter::new(10, Vec::new());
        emitter.push_text(Style::default(), "first");
        emitter.blank();
        emitter.push_line(&[Span::plain("second")]);
        let lines = emitter.finish();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].kind, LineKind::Blank);
        assert_eq!(lines[2].continuation, Continuation::None);
    }

    #[test]
    fn empty_lines_keep_their_context() {
        let mut emitter = Emitter::new(10, vec![Span::plain("│ ")]);
        emitter.set_context(LineKind::Code, Some(2));
        emitter.push_line(&[Span::plain("let x = 1;")]);
        emitter.push_line(&[]);
        emitter.push_line(&[Span::plain("let y = 2;")]);
        let lines = emitter.finish();
        assert_eq!(lines.len(), 3);
        assert!(lines[1].spans.is_empty());
        assert_eq!(lines[1].kind, LineKind::Code);
        assert_eq!(lines[1].block, Some(2));
        assert_eq!(lines[2].text(), "│ let y = 2;");
    }

    #[test]
    fn adjacent_verbatim_spans_merge() {
        let mut emitter = Emitter::new(40, Vec::new());
        emitter.push_spans(&[
            Span::mapped(Style::default(), "foo", SourceMap::Verbatim(10)),
            Span::mapped(Style::default(), "bar", SourceMap::Verbatim(13)),
        ]);
        let lines = emitter.finish();
        assert_eq!(lines[0].spans.len(), 1);
        assert_eq!(lines[0].spans[0].text, "foobar");
        assert_eq!(lines[0].spans[0].source, SourceMap::Verbatim(10));
    }
}
