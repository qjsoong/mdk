//! Source documents with line, column and byte-offset mapping.

use std::borrow::Cow;
use std::ops::Range;

/// The dominant line terminator of a source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix line endings (`\n`).
    Lf,
    /// Windows line endings (`\r\n`).
    Crlf,
    /// A mixture of CR, LF and CRLF line endings.
    Mixed,
    /// The document has no line terminator at all.
    None,
}

impl LineEnding {
    /// The byte sequence used by this line ending, if any.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
            Self::Mixed => "\n",
            Self::None => "",
        }
    }
}

/// A single physical line of a [`Source`].
///
/// Offsets are byte indices into the original text. `end` excludes the line
/// terminator, while `full_end` includes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    start: usize,
    end: usize,
    newline_len: u8,
}

impl Line {
    /// Byte offset of the first character of the line.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Byte offset one past the last character of the line content.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    /// Byte offset one past the line terminator (if any).
    #[must_use]
    pub const fn full_end(self) -> usize {
        self.end + self.newline_len as usize
    }

    /// Byte range of the line content, excluding the terminator.
    #[must_use]
    pub const fn range(self) -> Range<usize> {
        self.start..self.end
    }

    /// Number of bytes of line content.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end - self.start
    }

    /// Whether the line is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Length of this line's terminator in bytes.
    #[must_use]
    pub const fn newline_len(self) -> u8 {
        self.newline_len
    }
}

/// A 1-based, character-based position in a source document.
///
/// Columns count Unicode scalar values, mirroring `comrak`'s
/// `sourcepos_chars` mode, so editors on wide characters and CJK text report
/// intuitive columns.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Position {
    /// 1-based line number.
    pub line: usize,
    /// 1-based character column.
    pub column: usize,
}

impl Position {
    /// Creates a position from 1-based line and character column.
    #[must_use]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A start/end position pair. Both positions are inclusive.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
)]
pub struct Span {
    /// First character of the span.
    pub start: Position,
    /// Last character of the span.
    pub end: Position,
}

impl Span {
    /// Creates a span covering a single position.
    #[must_use]
    pub const fn point(position: Position) -> Self {
        Self {
            start: position,
            end: position,
        }
    }

    /// Creates a span from start and end positions.
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

/// An owned Markdown document with efficient line and offset lookups.
///
/// `Source` never borrows, so callers can move documents across threads or
/// store them next to parsed data owned elsewhere. A leading YAML (`---`) or
/// TOML (`+++`) front matter block is recognised and masked out before
/// parsing; [`Source::front_matter`] exposes it for lint and format stages.
#[derive(Debug, Clone)]
pub struct Source {
    text: String,
    lines: Vec<Line>,
    line_ending: LineEnding,
    front_matter: Option<Range<usize>>,
}

impl Source {
    /// Creates a source document from owned text.
    #[must_use]
    pub fn new(text: String) -> Self {
        let (lines, line_ending) = split_lines(&text);
        let front_matter = detect_front_matter(&text, &lines);
        Self {
            text,
            lines,
            line_ending,
            front_matter,
        }
    }

    /// The original document text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The dominant line terminator of the document.
    #[must_use]
    pub const fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// The number of physical lines, ignoring a trailing empty line after a
    /// final terminator.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns the physical line at `index` (0-based), if any.
    #[must_use]
    pub fn get_line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(|line| &self.text[line.range()])
    }

    /// Returns the physical line at `index` (0-based).
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    #[must_use]
    pub fn line(&self, index: usize) -> &str {
        &self.text[self.lines[index].range()]
    }

    /// All lines as string slices.
    pub fn lines(&self) -> impl ExactSizeIterator<Item = &str> {
        self.lines.iter().map(|line| &self.text[line.range()])
    }

    /// The byte range of a line, excluding its terminator.
    #[must_use]
    pub fn line_range(&self, index: usize) -> Range<usize> {
        self.lines.get(index).map_or(0..0, |line| line.range())
    }

    /// The `Line` metadata for `index` (0-based), if any.
    #[must_use]
    pub fn line_info(&self, index: usize) -> Option<Line> {
        self.lines.get(index).copied()
    }

    /// The 0-based line index containing `offset`.
    #[must_use]
    pub fn line_index_of(&self, offset: usize) -> usize {
        let idx = self.lines.partition_point(|line| line.full_end() <= offset);
        idx.min(self.lines.len().saturating_sub(1))
    }

    /// Converts a byte offset to a 1-based line/column position.
    #[must_use]
    pub fn position_of(&self, offset: usize) -> Position {
        let offset = offset.min(self.text.len());
        let index = self.line_index_of(offset);
        let Some(line) = self.lines.get(index) else {
            return Position::new(1, 1);
        };
        let mut end = offset.clamp(line.start(), line.end());
        while end > line.start() && !self.text.is_char_boundary(end) {
            end -= 1;
        }
        let column = self.text[line.start()..end].chars().count() + 1;
        Position::new(index + 1, column)
    }

    /// Converts a 1-based line and character column to a byte offset.
    ///
    /// Returns `None` when the line does not exist. A column past the end of
    /// the line is clamped to the end of the line content.
    #[must_use]
    pub fn offset_of(&self, position: Position) -> Option<usize> {
        let line = self.lines.get(position.line.checked_sub(1)?)?;
        if position.column <= 1 {
            return Some(line.start());
        }
        let content = &self.text[line.range()];
        let mut offset = line.end();
        for (seen, (byte, _)) in content.char_indices().enumerate() {
            if seen + 1 >= position.column {
                offset = line.start() + byte;
                return Some(offset);
            }
        }
        Some(offset)
    }

    /// The byte range for a span, clamped to the document.
    #[must_use]
    pub fn span_range(&self, span: Span) -> Range<usize> {
        let start = self.offset_of(span.start).unwrap_or(0);
        let end = self.offset_of(span.end).map_or(self.text.len(), |offset| {
            self.text[offset..]
                .chars()
                .next()
                .map_or(offset, |ch| offset + ch.len_utf8())
        });
        start.min(self.text.len())..end.min(self.text.len())
    }

    /// A subslice of the original text.
    #[must_use]
    pub fn slice(&self, range: Range<usize>) -> &str {
        &self.text[range]
    }

    /// Converts a half-open byte range to an inclusive span.
    ///
    /// The end position points at the start of the last character covered by
    /// the range, so multi-byte characters never split a column.
    #[must_use]
    pub fn span_of(&self, range: Range<usize>) -> Span {
        if range.is_empty() {
            return Span::point(self.position_of(range.start));
        }
        let start = range.start.min(self.text.len());
        let end = range.end.min(self.text.len());
        let mut last = end;
        while last > start && !self.text.is_char_boundary(last) {
            last -= 1;
        }
        // Step back one character so the end position is that of the last
        // covered character, not the byte after it.
        if last > start {
            let mut previous = last - 1;
            while previous > start && !self.text.is_char_boundary(previous) {
                previous -= 1;
            }
            last = previous;
        }
        Span::new(self.position_of(start), self.position_of(last))
    }

    /// The front matter block, if the document starts with one.
    #[must_use]
    pub fn front_matter(&self) -> Option<&str> {
        self.front_matter
            .as_ref()
            .map(|range| &self.text[range.clone()])
    }

    /// The byte range of the front matter block, if any.
    #[must_use]
    pub fn front_matter_range(&self) -> Option<Range<usize>> {
        self.front_matter.clone()
    }

    /// The value of a top-level scalar key in the front matter block.
    ///
    /// Both front matter syntaxes are understood: `key: value` (YAML) and
    /// `key = value` (TOML). A key must start in column 1, the value is
    /// trimmed, and one matching pair of surrounding quotes is removed.
    /// Nothing else is interpreted, so escape sequences, comments, nested
    /// keys, lists and multi-line scalars are returned as written.
    ///
    /// Returns `None` when the document has no front matter or the key is
    /// absent. A key with an empty value is `Some("")`.
    #[must_use]
    pub fn front_matter_value(&self, key: &str) -> Option<&str> {
        self.front_matter()?.lines().find_map(|line| {
            let (line_key, value) = split_front_matter_line(line)?;
            (line_key == key).then_some(value)
        })
    }

    /// The text to hand to a parser: identical to [`Source::text`] except
    /// that front matter bytes are replaced by spaces, keeping every byte
    /// offset stable.
    #[must_use]
    pub fn parse_text(&self) -> Cow<'_, str> {
        let Some(range) = &self.front_matter else {
            return Cow::Borrowed(&self.text);
        };
        let mut masked = String::with_capacity(self.text.len());
        masked.push_str(&self.text[..range.start]);
        for ch in self.text[range.clone()].chars() {
            if ch == '\n' || ch == '\r' {
                masked.push(ch);
            } else {
                for _ in 0..ch.len_utf8() {
                    masked.push(' ');
                }
            }
        }
        masked.push_str(&self.text[range.end..]);
        Cow::Owned(masked)
    }

    /// Whether a 1-based line number is inside the front matter block.
    #[must_use]
    pub fn is_front_matter_line(&self, line: usize) -> bool {
        let Some(range) = &self.front_matter else {
            return false;
        };
        let offset = match self.offset_of(Position::new(line, 1)) {
            Some(offset) => offset,
            None => return false,
        };
        offset < range.end
    }
}

fn split_lines(text: &str) -> (Vec<Line>, LineEnding) {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let mut saw_lf = false;
    let mut saw_crlf = false;
    let mut saw_cr = false;

    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                lines.push(Line {
                    start,
                    end: index,
                    newline_len: 1,
                });
                saw_lf = true;
                index += 1;
                start = index;
            }
            b'\r' => {
                let len: usize = if bytes.get(index + 1) == Some(&b'\n') {
                    saw_crlf = true;
                    2
                } else {
                    saw_cr = true;
                    1
                };
                lines.push(Line {
                    start,
                    end: index,
                    newline_len: len as u8,
                });
                index += len;
                start = index;
            }
            _ => index += 1,
        }
    }

    if start < bytes.len() {
        lines.push(Line {
            start,
            end: bytes.len(),
            newline_len: 0,
        });
    }

    let distinct = [saw_lf, saw_crlf, saw_cr]
        .iter()
        .filter(|seen| **seen)
        .count();
    let line_ending = match distinct {
        0 => LineEnding::None,
        1 if saw_lf => LineEnding::Lf,
        1 if saw_crlf => LineEnding::Crlf,
        _ => LineEnding::Mixed,
    };

    (lines, line_ending)
}

/// Splits a front matter line into its top-level key and scalar value.
///
/// Returns `None` for lines that do not start with a key, so indented keys,
/// list items and plain text never match.
fn split_front_matter_line(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once([':', '='])?;
    if key.is_empty() || key.starts_with(char::is_whitespace) {
        return None;
    }
    Some((key.trim_end(), unquote(value.trim())))
}

/// Removes one matching pair of surrounding quotes from a value.
fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

fn detect_front_matter(text: &str, lines: &[Line]) -> Option<Range<usize>> {
    let first = lines.first()?;
    let opener = text[first.range()].trim_end();
    let closer = match opener {
        "---" => "---",
        "+++" => "+++",
        _ => return None,
    };
    // A blank line directly after the opening delimiter is not valid YAML or
    // TOML front matter, and treating it as such would swallow a document
    // that starts with two thematic breaks.
    let second = lines.get(1)?;
    let second_text = text[second.range()].trim_end();
    if second_text != closer && second_text.is_empty() {
        return None;
    }
    for line in &lines[1..] {
        let candidate = text[line.range()].trim_end();
        if candidate == closer {
            return Some(0..line.end());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_unix_lines() {
        let source = Source::new("a\nb\n".to_string());
        assert_eq!(source.line_count(), 2);
        assert_eq!(source.line(0), "a");
        assert_eq!(source.line(1), "b");
        assert_eq!(source.line_ending(), LineEnding::Lf);
    }

    #[test]
    fn splits_without_trailing_newline() {
        let source = Source::new("a\nb".to_string());
        assert_eq!(source.line_count(), 2);
        assert_eq!(source.line(1), "b");
    }

    #[test]
    fn splits_crlf() {
        let source = Source::new("a\r\nb\r\n".to_string());
        assert_eq!(source.line_count(), 2);
        assert_eq!(source.line(1), "b");
        assert_eq!(source.line_ending(), LineEnding::Crlf);
    }

    #[test]
    fn splits_bare_cr() {
        let source = Source::new("a\rb".to_string());
        assert_eq!(source.line_count(), 2);
        assert_eq!(source.line(1), "b");
    }

    #[test]
    fn empty_document_has_no_lines() {
        let source = Source::new(String::new());
        assert_eq!(source.line_count(), 0);
        assert_eq!(source.line_ending(), LineEnding::None);
    }

    #[test]
    fn position_round_trips_across_wide_chars() {
        let source = Source::new("héllo 世界\nnext\n".to_string());
        let offset = source.offset_of(Position::new(1, 7)).unwrap();
        assert_eq!(&source.text()[offset..offset + 3], "世");
        assert_eq!(source.position_of(offset), Position::new(1, 7));
        assert_eq!(source.position_of(0), Position::new(1, 1));
    }

    #[test]
    fn span_conversion_handles_multibyte_characters() {
        let source = Source::new("你好世界\n".to_string());
        // Bytes 0..9 cover the first three characters; the last character
        // spans bytes 6..9.
        let span = source.span_of(0..9);
        assert_eq!(span.start, Position::new(1, 1));
        assert_eq!(span.end, Position::new(1, 3));
        assert_eq!(source.span_range(span), 0..9);
    }

    #[test]
    fn position_of_clamps_to_character_boundaries() {
        let source = Source::new("你好\n".to_string());
        assert_eq!(source.position_of(4), Position::new(1, 2));
    }

    #[test]
    fn span_range_is_inclusive() {
        let source = Source::new("hello\n".to_string());
        let range =
            source.span_range(Span::new(Position::new(1, 1), Position::new(1, 3)));
        assert_eq!(source.slice(range), "hel");
    }

    #[test]
    fn detects_yaml_front_matter_and_masks_it() {
        let source = Source::new("---\ntitle: x\n---\n# Hi\n".to_string());
        assert_eq!(source.front_matter(), Some("---\ntitle: x\n---"));
        assert!(source.is_front_matter_line(2));
        assert!(!source.is_front_matter_line(4));
        let masked = source.parse_text();
        assert_eq!(masked.len(), source.text().len());
        assert!(masked.starts_with("   \n"));
        assert!(masked.ends_with("# Hi\n"));
    }

    #[test]
    fn detects_toml_front_matter() {
        let source = Source::new("+++\ntitle = \"x\"\n+++\nbody\n".to_string());
        assert!(source.front_matter().is_some());
    }

    #[test]
    fn unterminated_front_matter_is_not_front_matter() {
        let source = Source::new("---\ntitle: x\n\nbody\n".to_string());
        assert!(source.front_matter().is_none());
    }

    #[test]
    fn thematic_breaks_are_not_front_matter() {
        // Three horizontal rules must not be mistaken for front matter just
        // because two of them use the same delimiter.
        let source = Source::new("---\n\n---\n\n---\n\ntext\n".to_string());
        assert!(source.front_matter().is_none());
    }

    #[test]
    fn empty_front_matter_is_recognised() {
        let source = Source::new("---\n---\ntext\n".to_string());
        assert!(source.front_matter().is_some());
    }

    #[test]
    fn reads_yaml_and_toml_front_matter_values() {
        let yaml = Source::new(
            "---\ntitle: Hello\n  indented: no\nsubtitle: Other\nurl: https://x/y\n---\nbody\n"
                .to_string(),
        );
        assert_eq!(yaml.front_matter_value("title"), Some("Hello"));
        assert_eq!(yaml.front_matter_value("url"), Some("https://x/y"));
        assert_eq!(yaml.front_matter_value("subtitle"), Some("Other"));
        assert_eq!(yaml.front_matter_value("missing"), None);

        let toml = Source::new(
            "+++\ntitle = \"Hello\"\ndraft=true\ndate = 2026-10-14\n+++\nbody\n"
                .to_string(),
        );
        assert_eq!(toml.front_matter_value("title"), Some("Hello"));
        assert_eq!(toml.front_matter_value("draft"), Some("true"));
        assert_eq!(toml.front_matter_value("date"), Some("2026-10-14"));
    }

    #[test]
    fn front_matter_values_are_trimmed_and_unquoted() {
        let source = Source::new(
            "---\ntitle:   'Spaced out'  \nnote: a value with: a colon\nempty:\nquoted: \"\"\n---\n"
                .to_string(),
        );
        assert_eq!(source.front_matter_value("title"), Some("Spaced out"));
        assert_eq!(
            source.front_matter_value("note"),
            Some("a value with: a colon")
        );
        assert_eq!(source.front_matter_value("empty"), Some(""));
        assert_eq!(source.front_matter_value("quoted"), Some(""));
    }

    #[test]
    fn only_top_level_keys_are_read() {
        let source = Source::new(
            "---\nauthor:\n  name: Nested\n- title: list item\ntitle: Top\n---\n"
                .to_string(),
        );
        assert_eq!(source.front_matter_value("title"), Some("Top"));
        assert_eq!(source.front_matter_value("name"), None);
        assert_eq!(
            Source::new("# title: not front matter\n".to_string())
                .front_matter_value("title"),
            None
        );
    }
}
