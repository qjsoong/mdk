//! Text styling and the line emitter used by the terminal renderer.

use mdk_core::text::display_width;

/// A terminal colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Color {
    Green,
    Yellow,
    Magenta,
    Cyan,
    BrightBlack,
    BrightYellow,
    BrightBlue,
    BrightCyan,
}

impl Color {
    fn fg_code(self) -> &'static str {
        match self {
            Self::Green => "32",
            Self::Yellow => "33",
            Self::Magenta => "35",
            Self::Cyan => "36",
            Self::BrightBlack => "90",
            Self::BrightYellow => "93",
            Self::BrightBlue => "94",
            Self::BrightCyan => "96",
        }
    }

    fn bg_code(self) -> &'static str {
        match self {
            Self::Green => "42",
            Self::Yellow => "43",
            Self::Magenta => "45",
            Self::Cyan => "46",
            Self::BrightBlack => "100",
            Self::BrightYellow => "103",
            Self::BrightBlue => "104",
            Self::BrightCyan => "106",
        }
    }
}

/// A text style; combining two styles unions the attributes and lets the
/// inner style win for colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Style {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
}

impl Style {
    pub fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    pub fn dim(self) -> Self {
        Self { dim: true, ..self }
    }

    pub fn italic(self) -> Self {
        Self {
            italic: true,
            ..self
        }
    }

    pub fn strikethrough(self) -> Self {
        Self {
            strike: true,
            ..self
        }
    }

    pub fn underline(self) -> Self {
        Self {
            underline: true,
            ..self
        }
    }

    pub fn fg(self, color: Color) -> Self {
        Self {
            fg: Some(color),
            ..self
        }
    }

    pub fn bg(self, color: Color) -> Self {
        Self {
            bg: Some(color),
            ..self
        }
    }

    fn is_plain(self) -> bool {
        self == Self::default()
    }
}

/// A styled piece of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Span {
    pub style: Style,
    pub text: String,
    /// Atomic spans are never split at their internal spaces.
    pub atomic: bool,
}

impl Span {
    pub fn new(style: Style, text: impl Into<String>) -> Self {
        Self {
            style,
            text: text.into(),
            atomic: false,
        }
    }

    pub fn atomic(style: Style, text: impl Into<String>) -> Self {
        Self {
            style,
            text: text.into(),
            atomic: true,
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(Style::default(), text)
    }

    pub fn width(&self) -> usize {
        display_width(&self.text)
    }
}

/// Wraps spans to `limit` columns, returning lines of spans.
pub(crate) fn wrap_spans(spans: &[Span], limit: usize) -> Vec<Vec<Span>> {
    let limit = limit.max(1);
    let mut lines: Vec<Vec<Span>> = Vec::new();
    let mut current: Vec<Span> = Vec::new();
    let mut width = 0usize;

    let mut space_before = false;
    for span in spans {
        for (kind, token) in tokens(span) {
            match kind {
                TokenKind::Space => space_before = true,
                TokenKind::Newline => {
                    lines.push(std::mem::take(&mut current));
                    width = 0;
                    space_before = false;
                }
                TokenKind::Word => {
                    let word = Span {
                        style: span.style,
                        text: token,
                        atomic: span.atomic,
                    };
                    let word_width = word.width();
                    let separator = usize::from(space_before && !current.is_empty());
                    if current.is_empty() {
                        current.push(word);
                        width = word_width;
                    } else if width + separator + word_width <= limit {
                        if space_before {
                            current.push(Span::plain(" "));
                        }
                        current.push(word);
                        width += separator + word_width;
                    } else {
                        lines.push(std::mem::take(&mut current));
                        current.push(word);
                        width = word_width;
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

enum TokenKind {
    Space,
    Newline,
    Word,
}

fn tokens(span: &Span) -> Vec<(TokenKind, String)> {
    if span.atomic {
        return vec![(TokenKind::Word, span.text.clone())];
    }
    let mut out = Vec::new();
    let mut word = String::new();
    for ch in span.text.chars() {
        match ch {
            '\n' => {
                if !word.is_empty() {
                    out.push((TokenKind::Word, std::mem::take(&mut word)));
                }
                out.push((TokenKind::Newline, String::new()));
            }
            ch if ch.is_whitespace() => {
                if !word.is_empty() {
                    out.push((TokenKind::Word, std::mem::take(&mut word)));
                }
                if !matches!(out.last(), Some((TokenKind::Space, _))) {
                    out.push((TokenKind::Space, String::new()));
                }
            }
            _ => word.push(ch),
        }
    }
    if !word.is_empty() {
        out.push((TokenKind::Word, word));
    }
    out
}

/// Accumulates styled lines with a shared prefix.
pub(crate) struct Emitter {
    lines: Vec<Vec<Span>>,
    current: Vec<Span>,
    width: usize,
    has_content: bool,
    max_width: usize,
    color: bool,
    prefix: Vec<Span>,
    prefix_width: usize,
    pending_space: bool,
}

impl Emitter {
    pub fn new(max_width: usize, color: bool, prefix: Vec<Span>) -> Self {
        let prefix_width = columns(&prefix);
        Self {
            lines: Vec::new(),
            current: Vec::new(),
            width: 0,
            has_content: false,
            max_width: max_width.max(1),
            color,
            prefix,
            prefix_width,
            pending_space: false,
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

    /// Appends wrapped, whitespace-separated text.
    pub fn push_text(&mut self, style: Style, text: &str) {
        self.push_spans(&[Span::new(style, text)]);
    }

    /// Appends spans, wrapping at word boundaries.
    pub fn push_spans(&mut self, spans: &[Span]) {
        for span in spans {
            for (kind, token) in tokens(span) {
                match kind {
                    TokenKind::Space => self.pending_space = true,
                    TokenKind::Newline => {
                        self.newline();
                    }
                    TokenKind::Word => {
                        let word = Span {
                            style: span.style,
                            text: token,
                            atomic: span.atomic,
                        };
                        self.push_word(&word);
                    }
                }
            }
        }
    }

    fn push_word(&mut self, word: &Span) {
        self.start_line();
        let word_width = word.width();
        let separator = usize::from(self.pending_space && self.has_content);
        if self.has_content && self.width + separator + word_width > self.max_width {
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

    /// Appends spans verbatim, without wrapping.
    pub fn push_line(&mut self, spans: &[Span]) {
        self.start_line();
        for span in spans {
            self.append(span.clone());
        }
        self.has_content = true;
        self.pending_space = false;
        self.newline();
    }

    /// Appends raw text; newlines are honoured and no wrapping happens.
    pub fn push_raw(&mut self, style: Style, text: &str) {
        for (index, part) in text.split('\n').enumerate() {
            if index > 0 {
                self.force_newline();
            }
            if part.is_empty() {
                continue;
            }
            self.start_line();
            self.append(Span::atomic(style, part));
            self.has_content = true;
            self.pending_space = false;
        }
    }

    /// Ends the current line, inserting a blank line when it was already
    /// ended. Used by soft structural breaks such as inline hard breaks.
    pub fn newline(&mut self) {
        if !self.current.is_empty() {
            self.lines.push(std::mem::take(&mut self.current));
        } else if !self.lines.is_empty()
            && !is_blank_line(self.lines.last().expect("line exists"))
        {
            self.lines.push(Vec::new());
        }
        self.reset_line();
    }

    /// Ends the current line if it has content; a no-op for an empty line.
    pub fn end_line(&mut self) {
        if !self.current.is_empty() {
            self.lines.push(std::mem::take(&mut self.current));
        }
        self.reset_line();
    }

    /// Ends the current line, always starting a fresh one afterwards. Used
    /// between the lines of a code block so blank lines survive.
    fn force_newline(&mut self) {
        if !self.current.is_empty() {
            self.lines.push(std::mem::take(&mut self.current));
        } else {
            self.lines.push(Vec::new());
        }
        self.reset_line();
    }

    fn reset_line(&mut self) {
        self.width = 0;
        self.has_content = false;
        self.pending_space = false;
    }

    /// Ensures exactly one blank line before the next content.
    pub fn blank(&mut self) {
        if !self.current.is_empty() {
            self.lines.push(std::mem::take(&mut self.current));
        }
        if !self.lines.is_empty()
            && !is_blank_line(self.lines.last().expect("line exists"))
        {
            self.lines.push(Vec::new());
        }
        self.width = 0;
        self.has_content = false;
        self.pending_space = false;
    }

    fn start_line(&mut self) {
        if !self.current.is_empty() {
            return;
        }
        self.current = self.prefix.clone();
        self.width = self.prefix_width;
    }

    fn append(&mut self, span: Span) {
        if let Some(last) = self.current.last_mut()
            && last.style == span.style
            && last.atomic == span.atomic
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

    pub fn finish(mut self) -> String {
        if !self.current.is_empty() {
            self.lines.push(std::mem::take(&mut self.current));
        }
        while self.lines.last().is_some_and(|line| is_blank_line(line)) {
            self.lines.pop();
        }
        let mut output = String::new();
        for line in &self.lines {
            for span in line {
                output.push_str(&encode(span, self.color));
            }
            output.push('\n');
        }
        output
    }
}

fn columns(spans: &[Span]) -> usize {
    spans.iter().map(Span::width).sum()
}

fn is_blank_line(line: &[Span]) -> bool {
    line.iter().all(|span| span.text.trim().is_empty())
}

fn encode(span: &Span, color: bool) -> String {
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

    #[test]
    fn wraps_spans_at_word_boundaries() {
        let spans = [Span::new(Style::default(), "hello world again")];
        let lines = wrap_spans(&spans, 11);
        assert_eq!(lines.len(), 2);
        let text: String = lines[0].iter().map(|span| span.text.as_str()).collect();
        assert_eq!(text, "hello world");
    }

    #[test]
    fn atomic_spans_are_not_split() {
        let spans = [Span::atomic(Style::default(), "a b c d e")];
        let lines = wrap_spans(&spans, 4);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn emitter_encodes_ansi() {
        let mut emitter = Emitter::new(40, true, Vec::new());
        emitter.push_text(Style::default().bold(), "hi");
        let output = emitter.finish();
        assert_eq!(output, "\u{1b}[1mhi\u{1b}[0m\n");
    }

    #[test]
    fn prefix_is_repeated_on_wrapped_lines() {
        let mut emitter = Emitter::new(10, false, vec![Span::plain("> ")]);
        emitter.push_text(Style::default(), "one two three four");
        let output = emitter.finish();
        assert!(output.starts_with("> one two"));
        assert!(output.contains("\n> "));
    }
}
