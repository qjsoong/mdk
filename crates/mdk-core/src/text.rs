//! Small text helpers shared by lint, format and render stages.

use unicode_width::UnicodeWidthChar;

/// The display width of `text` in terminal columns.
///
/// Tabs count as one column; control characters count as zero. East Asian
/// wide characters and emoji count as two columns where `unicode-width`
/// reports them as wide.
#[must_use]
pub fn display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| match ch {
            '\t' => 1,
            _ => ch.width().unwrap_or(0),
        })
        .sum()
}

/// Whether `line` consists solely of whitespace (or is empty).
#[must_use]
pub fn is_blank(line: &str) -> bool {
    line.chars().all(char::is_whitespace)
}

/// The leading whitespace of `line`.
#[must_use]
pub fn leading_whitespace(line: &str) -> &str {
    let end = line
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map_or(line.len(), |(index, _)| index);
    &line[..end]
}

/// The byte length of the leading whitespace of `line`.
#[must_use]
pub fn leading_whitespace_len(line: &str) -> usize {
    leading_whitespace(line).len()
}

/// The indentation width of `line` in columns, expanding tabs to the next
/// four-column tab stop, as CommonMark does.
#[must_use]
pub fn indent_width(line: &str) -> usize {
    let mut width = 0;
    for ch in line.chars() {
        match ch {
            ' ' => width += 1,
            '\t' => width += 4 - (width % 4),
            _ => break,
        }
    }
    width
}

/// `line` without trailing whitespace.
#[must_use]
pub fn trim_end(line: &str) -> &str {
    line.trim_end_matches([' ', '\t', '\r', '\x0b', '\x0c'])
}

/// `line` without leading and trailing whitespace.
#[must_use]
pub fn trim(line: &str) -> &str {
    trim_end(line.trim_start_matches([' ', '\t', '\x0b', '\x0c']))
}

/// Adds a visual column offset to every line of a multi-line span.
///
/// Used by renderers that prefix blockquote markers or list bullets.
#[must_use]
pub fn pad_lines(text: &str, width: usize) -> String {
    let pad = " ".repeat(width);
    let mut out = String::with_capacity(text.len() + width);
    for chunk in text.split_inclusive('\n') {
        let (line, newline) = match chunk.strip_suffix('\n') {
            Some(line) => (line, "\n"),
            None => (chunk, ""),
        };
        out.push_str(&pad);
        out.push_str(line);
        out.push_str(newline);
    }
    if out.is_empty() {
        out.push_str(&pad);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_count_cjk_as_two() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width("世界"), 4);
        assert_eq!(display_width("café"), 4);
        assert_eq!(display_width("a\tb"), 3);
    }

    #[test]
    fn indentation_expands_tabs() {
        assert_eq!(indent_width("    x"), 4);
        assert_eq!(indent_width("\tx"), 4);
        assert_eq!(indent_width(" \tx"), 4);
        assert_eq!(indent_width("x"), 0);
    }

    #[test]
    fn blank_lines() {
        assert!(is_blank(""));
        assert!(is_blank("  \t"));
        assert!(!is_blank(" x "));
    }

    #[test]
    fn pads_every_line() {
        assert_eq!(pad_lines("a\nb\n", 2), "  a\n  b\n");
        assert_eq!(pad_lines("", 2), "  ");
    }
}
