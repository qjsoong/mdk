//! The structured terminal render model.
//!
//! [`render_terminal_document`](crate::render_terminal_document) produces a
//! [`Document`]: styled lines whose [`Span`]s know which bytes of the Markdown
//! source they came from. `mdk render` encodes the same document to ANSI or
//! plain text; interactive tools can use the same document to implement
//! selection, copy and navigation that is independent of the visual layout.
//!
//! ```rust
//! use mdk_core::Dialect;
//! use mdk_render::TermOptions;
//! use mdk_render::term::LineKind;
//!
//! let document = mdk_render::render_terminal_document(
//!     "# Title\n\nSome *text*.\n",
//!     Dialect::Commonmark,
//!     &TermOptions::default(),
//! );
//! let heading = &document.lines[0];
//! assert_eq!(heading.kind, LineKind::Heading(1));
//! assert_eq!(heading.text(), "Title");
//! ```

use std::ops::Range;
use std::sync::Arc;

use mdk_core::text::display_width;

/// A byte range in the source document.
pub type SrcRange = Range<usize>;

/// A terminal colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Green, used for completed task list items.
    Green,
    /// Yellow, used as a highlight background.
    Yellow,
    /// Magenta, used for math.
    Magenta,
    /// Cyan, used for footnote references.
    Cyan,
    /// Bright black, used for dimmed secondary text.
    BrightBlack,
    /// Bright yellow, used for inline code and low-level headings.
    BrightYellow,
    /// Bright blue, used for links and second-level headings.
    BrightBlue,
    /// Bright cyan, used for third-level headings.
    BrightCyan,
}

impl Color {
    pub(crate) fn fg_code(self) -> &'static str {
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

    pub(crate) fn bg_code(self) -> &'static str {
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
pub struct Style {
    /// Bold text.
    pub bold: bool,
    /// Faint text.
    pub dim: bool,
    /// Italic text.
    pub italic: bool,
    /// Underlined text.
    pub underline: bool,
    /// Struck-through text.
    pub strike: bool,
    /// Foreground colour.
    pub fg: Option<Color>,
    /// Background colour.
    pub bg: Option<Color>,
}

impl Style {
    /// Returns the style with bold enabled.
    #[must_use]
    pub fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    /// Returns the style with faint text enabled.
    #[must_use]
    pub fn dim(self) -> Self {
        Self { dim: true, ..self }
    }

    /// Returns the style with italics enabled.
    #[must_use]
    pub fn italic(self) -> Self {
        Self {
            italic: true,
            ..self
        }
    }

    /// Returns the style with strikethrough enabled.
    #[must_use]
    pub fn strikethrough(self) -> Self {
        Self {
            strike: true,
            ..self
        }
    }

    /// Returns the style with an underline enabled.
    #[must_use]
    pub fn underline(self) -> Self {
        Self {
            underline: true,
            ..self
        }
    }

    /// Returns the style with the given foreground colour.
    #[must_use]
    pub fn fg(self, color: Color) -> Self {
        Self {
            fg: Some(color),
            ..self
        }
    }

    /// Returns the style with the given background colour.
    #[must_use]
    pub fn bg(self, color: Color) -> Self {
        Self {
            bg: Some(color),
            ..self
        }
    }

    pub(crate) fn is_plain(self) -> bool {
        self == Self::default()
    }
}

/// Maps the text of a [`Span`] back to the Markdown source.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SourceMap {
    /// The renderer generated the text; it has no source counterpart. Used for
    /// list bullets, table borders, padding and other decorations.
    #[default]
    None,
    /// The text is a verbatim slice of the source starting at this byte
    /// offset. Most prose and code falls into this case.
    Verbatim(usize),
    /// One source range per rendered character: `Some` when the character
    /// exists in the source, `None` when the renderer generated it. Used for
    /// text that decodes backslash escapes or character entities.
    PerChar(Arc<[Option<SrcRange>]>),
}

impl SourceMap {
    /// Whether the map carries no source information at all.
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// The source range of the character at `index`, counted in characters.
    #[must_use]
    pub fn char_range(&self, text: &str, index: usize) -> Option<SrcRange> {
        match self {
            Self::None => None,
            Self::Verbatim(start) => {
                let offset =
                    text.char_indices().nth(index).map(|(offset, _)| offset)?;
                let ch = text[offset..].chars().next()?;
                Some(start + offset..start + offset + ch.len_utf8())
            }
            Self::PerChar(ranges) => ranges.get(index).cloned().flatten(),
        }
    }

    /// The source range of the character at `offset`, a byte offset into
    /// `text` that must lie on a character boundary.
    #[must_use]
    pub fn char_at(&self, text: &str, offset: usize) -> Option<SrcRange> {
        match self {
            Self::None => None,
            Self::Verbatim(start) => {
                let ch = text.get(offset..)?.chars().next()?;
                Some(start + offset..start + offset + ch.len_utf8())
            }
            Self::PerChar(ranges) => {
                let index = text.get(..offset)?.chars().count();
                ranges.get(index).cloned().flatten()
            }
        }
    }

    /// The smallest source range covering the characters in `chars`, if any
    /// character in that range is mapped.
    #[must_use]
    pub fn range_of(&self, text: &str, chars: Range<usize>) -> Option<SrcRange> {
        let characters = text.chars().count();
        let end = chars.end.min(characters);
        let mut found: Option<SrcRange> = None;
        for index in chars.start..end {
            if let Some(range) = self.char_range(text, index) {
                found = Some(match found {
                    Some(current) => {
                        current.start.min(range.start)..current.end.max(range.end)
                    }
                    None => range,
                });
            }
        }
        found
    }

    /// The map for `text[chars]`, used when a renderer splits a span. The
    /// character range is clamped to the text.
    #[must_use]
    pub fn slice(&self, text: &str, chars: Range<usize>) -> Self {
        match self {
            Self::None => Self::None,
            Self::Verbatim(start) => {
                let offset = text
                    .char_indices()
                    .nth(chars.start)
                    .map_or(text.len(), |(offset, _)| offset);
                Self::Verbatim(start + offset)
            }
            Self::PerChar(ranges) => {
                let end = chars.end.min(ranges.len());
                let start = chars.start.min(end);
                Self::PerChar(Arc::from(&ranges[start..end]))
            }
        }
    }

    /// Concatenates `next` onto `self`. `text_len` is the byte length of the
    /// text `self` describes. Returns whether the concatenation represents the
    /// combined text exactly; when it returns false the spans must be kept
    /// apart.
    pub(crate) fn try_concat(&mut self, text_len: usize, next: &Self) -> bool {
        match (&mut *self, next) {
            (Self::None, Self::None) => true,
            (Self::Verbatim(start), Self::Verbatim(next_start)) => {
                *start + text_len == *next_start
            }
            (Self::PerChar(current), Self::PerChar(next)) => {
                let mut combined = Vec::with_capacity(current.len() + next.len());
                combined.extend_from_slice(current);
                combined.extend_from_slice(next);
                *current = Arc::from(combined);
                true
            }
            _ => false,
        }
    }
}

/// A styled piece of text on a rendered line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// The text to display.
    pub text: String,
    /// How to style it.
    pub style: Style,
    /// Where the text came from in the Markdown source.
    pub source: SourceMap,
    /// Atomic spans are never split at their internal spaces when wrapping.
    pub atomic: bool,
}

impl Span {
    /// Creates a span of generated text with no source counterpart.
    #[must_use]
    pub fn new(style: Style, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style,
            source: SourceMap::None,
            atomic: false,
        }
    }

    /// Creates a span that is never wrapped at its internal spaces.
    #[must_use]
    pub fn atomic(style: Style, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style,
            source: SourceMap::None,
            atomic: true,
        }
    }

    /// Creates an atomic span whose text maps to the source.
    #[must_use]
    pub fn atomic_mapped(
        style: Style,
        text: impl Into<String>,
        source: SourceMap,
    ) -> Self {
        Self {
            text: text.into(),
            style,
            source,
            atomic: true,
        }
    }

    /// Creates an unstyled span of generated text.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(Style::default(), text)
    }

    /// Creates a span whose text maps to the source.
    #[must_use]
    pub fn mapped(style: Style, text: impl Into<String>, source: SourceMap) -> Self {
        Self {
            text: text.into(),
            style,
            source,
            atomic: false,
        }
    }

    /// The display width of the span in terminal columns.
    #[must_use]
    pub fn width(&self) -> usize {
        display_width(&self.text)
    }

    /// The span for the characters in `chars`, keeping style and mapping.
    #[must_use]
    pub fn slice(&self, chars: Range<usize>) -> Self {
        Self {
            text: self
                .text
                .chars()
                .skip(chars.start)
                .take(chars.end.saturating_sub(chars.start))
                .collect(),
            source: self.source.slice(&self.text, chars.clone()),
            style: self.style,
            atomic: self.atomic,
        }
    }
}

/// A cell of a table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Display columns covered by the cell, excluding the column separators.
    pub columns: Range<usize>,
    /// Whether the cell belongs to the header row.
    pub header: bool,
}

/// How a rendered line relates to the line before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Continuation {
    /// The line starts a new logical line: copying joins it with a newline.
    #[default]
    None,
    /// A soft wrap split the line after a space; copying joins the two with a
    /// single space.
    Space,
    /// A soft wrap split the line without consuming anything; copying joins
    /// the two directly.
    Tight,
}

/// What kind of Markdown construct produced a rendered line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineKind {
    /// Running prose: paragraph text, list items, block quote text.
    #[default]
    Text,
    /// A heading; the value is the heading level.
    Heading(u8),
    /// A line of a fenced or indented code block.
    Code,
    /// A body row of a table.
    TableRow,
    /// A table border line.
    TableBorder,
    /// A thematic break.
    Rule,
    /// Raw HTML from the document.
    Html,
    /// A structural blank line between blocks.
    Blank,
}

/// A rendered line of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The styled spans of the line, in display order.
    pub spans: Vec<Span>,
    /// The Markdown construct this line came from.
    pub kind: LineKind,
    /// Index into [`Document::blocks`] of the block that produced the line.
    pub block: Option<usize>,
    /// How the line continues the previous one when its text is copied.
    pub continuation: Continuation,
    /// The cells of a table row; empty for every other line kind.
    pub cells: Vec<Cell>,
}

impl Line {
    /// An empty structural line.
    #[must_use]
    pub fn blank() -> Self {
        Self {
            spans: Vec::new(),
            kind: LineKind::Blank,
            block: None,
            continuation: Continuation::None,
            cells: Vec::new(),
        }
    }

    /// The visible text of the line, without styling.
    #[must_use]
    pub fn text(&self) -> String {
        let mut text = String::new();
        for span in &self.spans {
            text.push_str(&span.text);
        }
        text
    }

    /// The display width of the line in terminal columns.
    #[must_use]
    pub fn width(&self) -> usize {
        self.spans.iter().map(Span::width).sum()
    }

    /// Whether the line has no visible content.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.spans.iter().all(|span| span.text.trim().is_empty())
    }
}

/// What kind of Markdown block a [`Block`] describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A paragraph.
    Paragraph,
    /// A heading; the value is the heading level.
    Heading(u8),
    /// A code block, including its fence or indentation.
    Code,
    /// A table.
    Table,
    /// A raw HTML block.
    Html,
    /// A thematic break.
    Rule,
}

/// A Markdown block and the byte range it covers in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The kind of block.
    pub kind: BlockKind,
    /// The byte range of the block in the source document, excluding its
    /// trailing blank lines.
    pub range: SrcRange,
}

/// A rendered Markdown document with source mapping.
///
/// Byte ranges refer to the Markdown text passed to
/// [`render_terminal_document`](crate::render_terminal_document); the caller
/// keeps that text to turn ranges into strings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    /// The rendered lines, in reading order.
    pub lines: Vec<Line>,
    /// The blocks of the document, indexed by [`Line::block`].
    pub blocks: Vec<Block>,
}

impl Document {
    /// The full rendered text, with one `\n` per line.
    #[must_use]
    pub fn text(&self) -> String {
        let mut text = String::new();
        for line in &self.lines {
            text.push_str(&line.text());
            text.push('\n');
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_maps_characters_in_place() {
        let map = SourceMap::Verbatim(10);
        assert_eq!(map.char_range("abc", 1), Some(11..12));
        assert_eq!(map.char_at("abc", 2), Some(12..13));
        assert_eq!(map.range_of("abc", 0..2), Some(10..12));
        assert_eq!(map.range_of("abc", 1..3), Some(11..13));
        assert_eq!(map.range_of("abc", 3..3), None);
    }

    #[test]
    fn verbatim_slice_moves_the_start() {
        let map = SourceMap::Verbatim(10);
        assert_eq!(map.slice("hello world", 6..11), SourceMap::Verbatim(16));
        assert_eq!(map.slice("hello", 5..5), SourceMap::Verbatim(15));
    }

    #[test]
    fn per_char_ranges_use_their_own_offsets() {
        let map = SourceMap::PerChar(Arc::from(vec![Some(0..1), None, Some(5..10)]));
        assert_eq!(map.char_range("a b", 0), Some(0..1));
        assert_eq!(map.char_range("a b", 1), None);
        assert_eq!(map.char_range("a b", 2), Some(5..10));
        assert_eq!(map.range_of("a b", 0..3), Some(0..10));
        assert_eq!(
            map.slice("a b", 2..3),
            SourceMap::PerChar(Arc::from(vec![Some(5..10)]))
        );
    }

    #[test]
    fn concatenation_requires_contiguity() {
        let mut map = SourceMap::Verbatim(4);
        assert!(map.try_concat(3, &SourceMap::Verbatim(7)));
        assert_eq!(map, SourceMap::Verbatim(4));
        let mut map = SourceMap::Verbatim(4);
        assert!(!map.try_concat(3, &SourceMap::Verbatim(9)));
        let mut map = SourceMap::None;
        assert!(map.try_concat(1, &SourceMap::None));
        assert!(!map.try_concat(1, &SourceMap::Verbatim(2)));
    }

    #[test]
    fn span_slice_keeps_style_and_mapping() {
        let span = Span::mapped(
            Style::default().bold(),
            "hello world",
            SourceMap::Verbatim(100),
        );
        let sliced = span.slice(6..11);
        assert_eq!(sliced.text, "world");
        assert_eq!(sliced.style, Style::default().bold());
        assert_eq!(sliced.source, SourceMap::Verbatim(106));
    }

    #[test]
    fn line_text_and_width() {
        let line = Line {
            spans: vec![Span::plain("ab"), Span::plain("世界")],
            ..Line::blank()
        };
        assert_eq!(line.text(), "ab世界");
        assert_eq!(line.width(), 6);
    }
}
