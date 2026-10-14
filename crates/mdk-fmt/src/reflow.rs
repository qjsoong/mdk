//! Inline-aware paragraph reflow.
//!
//! The wrapper only ever replaces existing whitespace with line breaks: it
//! never inserts spaces that were not in the source and never breaks inside
//! code spans, autolinks, raw HTML, entities, link destinations or reference
//! labels. As a result, reflowed paragraphs parse to the same inlines.

use mdk_core::text::display_width;

use crate::options::HardBreakStyle;

/// A unit of wrapped text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// An unbreakable word, possibly containing protected whitespace.
    Word(String),
    /// A hard line break; `true` means a two-space break.
    HardBreak(bool),
}

/// Wraps `text` to the available widths, returning output lines without any
/// container prefix.
///
/// `first_avail` is the width available to the first output line and
/// `cont_avail` the width for subsequent lines.
pub(crate) fn wrap(
    text: &str,
    first_avail: usize,
    cont_avail: usize,
    style: HardBreakStyle,
) -> Vec<String> {
    let first_avail = first_avail.max(1);
    let cont_avail = cont_avail.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut width = 0usize;
    let mut first = true;

    for token in tokenize(text, style) {
        match token {
            Token::Word(word) => {
                let word_width = display_width(&word);
                let available = if first { first_avail } else { cont_avail };
                if current.is_empty() {
                    current = word;
                    width = word_width;
                } else if line_start_sensitive(&word)
                    || width + 1 + word_width <= available
                {
                    // Block markers must never begin a reflowed line, because
                    // that would change how the paragraph is parsed.
                    current.push(' ');
                    current.push_str(&word);
                    width += 1 + word_width;
                } else {
                    lines.push(std::mem::take(&mut current));
                    first = false;
                    current = word;
                    width = word_width;
                }
            }
            Token::HardBreak(spaces) => {
                if !current.is_empty() {
                    if spaces {
                        current.push_str("  ");
                    } else {
                        current.push('\\');
                    }
                }
                lines.push(std::mem::take(&mut current));
                first = false;
                width = 0;
            }
        }
    }

    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

fn tokenize(text: &str, style: HardBreakStyle) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut tokens: Vec<Token> = Vec::new();
    let mut word = String::new();
    let mut spaces_run = 0usize;
    let mut index = 0usize;

    macro_rules! flush_word {
        () => {
            if !word.is_empty() {
                tokens.push(Token::Word(std::mem::take(&mut word)));
            }
        };
    }

    while index < bytes.len() {
        let ch = text[index..].chars().next().expect("char boundary");
        match ch {
            ' ' | '\t' => {
                if !word.is_empty() {
                    flush_word!();
                }
                // Only literal spaces before a newline form a hard break.
                if ch == ' ' {
                    spaces_run += 1;
                } else {
                    spaces_run = 0;
                }
                index += ch.len_utf8();
            }
            '\n' => {
                let hard = spaces_run >= 2;
                flush_word!();
                if hard && !tokens.is_empty() {
                    let write_spaces = match style {
                        HardBreakStyle::Spaces => true,
                        HardBreakStyle::Backslash => false,
                        HardBreakStyle::Preserve => true,
                    };
                    tokens.push(Token::HardBreak(write_spaces));
                }
                spaces_run = 0;
                index += 1;
                index = skip_line_indent(bytes, index);
            }
            '\\' => {
                spaces_run = 0;
                match bytes.get(index + 1) {
                    Some(b'\n') => {
                        flush_word!();
                        if !tokens.is_empty() {
                            let write_spaces = matches!(style, HardBreakStyle::Spaces);
                            tokens.push(Token::HardBreak(write_spaces));
                        }
                        spaces_run = 0;
                        index += 2;
                        index = skip_line_indent(bytes, index);
                    }
                    Some(_) => {
                        let next =
                            text[index + 1..].chars().next().expect("char boundary");
                        word.push('\\');
                        word.push(next);
                        index += 1 + next.len_utf8();
                    }
                    None => {
                        word.push('\\');
                        index += 1;
                    }
                }
            }
            '`' => {
                spaces_run = 0;
                let run = count_run(bytes, index, b'`');
                if let Some(end) = find_backtick_close(bytes, index + run, run) {
                    let inner = &text[index..end];
                    for part in inner.split('\n') {
                        word.push_str(part);
                        word.push(' ');
                    }
                    word.pop();
                    index = end;
                } else {
                    for _ in 0..run {
                        word.push('`');
                    }
                    index += run;
                }
            }
            '<' => {
                spaces_run = 0;
                if let Some(end) = find_protected_angle(text, index) {
                    word.push_str(&text[index..end]);
                    index = end;
                } else {
                    word.push('<');
                    index += 1;
                }
            }
            '&' => {
                spaces_run = 0;
                if let Some(end) = find_entity(text, index) {
                    word.push_str(&text[index..end]);
                    index = end;
                } else {
                    word.push('&');
                    index += 1;
                }
            }
            ']' => {
                spaces_run = 0;
                if bytes.get(index + 1) == Some(&b'(') {
                    if let Some(end) = find_link_destination(text, index + 1) {
                        word.push_str(&text[index..end]);
                        index = end;
                    } else {
                        word.push_str(&text[index..index + 2]);
                        index += 2;
                    }
                } else if bytes.get(index + 1) == Some(&b'[') {
                    if let Some(end) = find_reference_label(text, index + 1) {
                        word.push_str(&text[index..end]);
                        index = end;
                    } else {
                        word.push_str(&text[index..index + 2]);
                        index += 2;
                    }
                } else {
                    word.push(']');
                    index += 1;
                }
            }
            '[' => {
                spaces_run = 0;
                if bytes.get(index + 1) == Some(&b'^') {
                    if let Some(end) = find_reference_label(text, index) {
                        word.push_str(&text[index..end]);
                        index = end;
                    } else {
                        word.push('[');
                        index += 1;
                    }
                } else {
                    word.push('[');
                    index += 1;
                }
            }
            _ => {
                spaces_run = 0;
                word.push(ch);
                index += ch.len_utf8();
            }
        }
    }
    flush_word!();
    tokens
}

/// Whether starting a line with `word` could change block parsing.
///
/// These words would be interpreted as blockquote markers, ATX or setext
/// headings, list markers, thematic breaks or code fences if they began a
/// line inside a paragraph.
fn line_start_sensitive(word: &str) -> bool {
    if word.starts_with('>') || word.starts_with('<') {
        // A line beginning with `>` or raw HTML could start a new block.
        return true;
    }
    if word.len() <= 6 && word.chars().all(|ch| ch == '#') {
        return true;
    }
    if matches!(word, "-" | "+" | "*") {
        return true;
    }
    if word.len() >= 3
        && (word.chars().all(|ch| ch == '-')
            || word.chars().all(|ch| ch == '=')
            || word.chars().all(|ch| ch == '_')
            || word.chars().all(|ch| ch == '*'))
    {
        return true;
    }
    if word.starts_with("```") || word.starts_with("~~~") {
        return true;
    }
    // Ordered list markers such as `1.` or `12)`.
    let marker = word
        .strip_suffix('.')
        .or_else(|| word.strip_suffix(')'))
        .unwrap_or("");
    !marker.is_empty()
        && marker.len() <= 9
        && marker.chars().all(|ch| ch.is_ascii_digit())
}

fn skip_line_indent(bytes: &[u8], mut index: usize) -> usize {
    while matches!(bytes.get(index), Some(b' ' | b'\t')) {
        index += 1;
    }
    index
}

fn count_run(bytes: &[u8], start: usize, byte: u8) -> usize {
    let mut index = start;
    while bytes.get(index) == Some(&byte) {
        index += 1;
    }
    index - start
}

/// Finds the closing backtick run of exactly `run` backticks.
fn find_backtick_close(bytes: &[u8], from: usize, run: usize) -> Option<usize> {
    let mut index = from;
    while index < bytes.len() {
        if bytes[index] != b'`' {
            index += 1;
            continue;
        }
        let length = count_run(bytes, index, b'`');
        if length == run {
            return Some(index + length);
        }
        index += length;
    }
    None
}

/// Protects HTML tags, comments and autolinks ending on the same line.
fn find_protected_angle(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start + 1..];
    let newline = rest.find('\n').unwrap_or(rest.len());
    let window = &rest[..newline];
    let end = window.find('>')?;
    let content = &window[..end];
    if content.is_empty() || content.len() > 500 {
        return None;
    }
    let looks_like_markup = content.starts_with('/')
        || content.starts_with('!')
        || content.starts_with('?')
        || content
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        || content.contains("://")
        || content.contains('@');
    if looks_like_markup {
        Some(start + 1 + end + 1)
    } else {
        None
    }
}

fn find_entity(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start + 1..];
    let end = rest.find(';')?;
    if end == 0 || end > 32 {
        return None;
    }
    let name = &rest[..end];
    let valid = name.strip_prefix('#').map_or_else(
        || name.chars().all(|ch| ch.is_ascii_alphanumeric()),
        |digits| {
            let digits = digits.strip_prefix(['x', 'X']).unwrap_or(digits);
            !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_hexdigit())
        },
    );
    if valid {
        Some(start + 1 + end + 1)
    } else {
        None
    }
}

/// Parses `(destination "title")` starting at the opening parenthesis.
fn find_link_destination(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = open + 1;
    index = skip_spaces_and_newlines(bytes, index);
    // Destination
    match bytes.get(index)? {
        b')' => return Some(index + 1),
        b'<' => {
            let close = text[index..].find('>')?;
            if text[index..index + close].contains('\n') {
                return None;
            }
            index += close + 1;
        }
        _ => {
            let mut depth = 0usize;
            while index < bytes.len() {
                match bytes[index] {
                    b'\\' => index += 2,
                    b'(' => {
                        depth += 1;
                        index += 1;
                    }
                    b')' if depth == 0 => break,
                    b')' => {
                        depth -= 1;
                        index += 1;
                    }
                    b' ' | b'\t' | b'\n' => break,
                    _ => index += 1,
                }
            }
        }
    }
    index = skip_spaces_and_newlines(bytes, index);
    // Optional title
    match bytes.get(index) {
        Some(b')') => return Some(index + 1),
        Some(b'"') | Some(b'\'') => {
            let quote = bytes[index];
            index += 1;
            while index < bytes.len() {
                match bytes[index] {
                    b'\\' => index += 2,
                    byte if byte == quote => {
                        index += 1;
                        break;
                    }
                    b'\n' => return None,
                    _ => index += 1,
                }
            }
        }
        Some(b'(') => {
            index += 1;
            while index < bytes.len() {
                match bytes[index] {
                    b'\\' => index += 2,
                    b')' => {
                        index += 1;
                        break;
                    }
                    b'\n' => return None,
                    _ => index += 1,
                }
            }
        }
        _ => return None,
    }
    index = skip_spaces_and_newlines(bytes, index);
    if bytes.get(index) == Some(&b')') {
        Some(index + 1)
    } else {
        None
    }
}

/// Parses `[label]` starting at the opening bracket.
fn find_reference_label(text: &str, open: usize) -> Option<usize> {
    let rest = &text[open + 1..];
    let end = rest.find(']')?;
    let label = &rest[..end];
    if label.contains(['[', ']', '\n']) {
        return None;
    }
    Some(open + 1 + end + 1)
}

fn skip_spaces_and_newlines(bytes: &[u8], mut index: usize) -> usize {
    while matches!(bytes.get(index), Some(b' ' | b'\t' | b'\n')) {
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str, width: usize) -> Vec<String> {
        wrap(text, width, width, HardBreakStyle::Spaces)
    }

    #[test]
    fn wraps_at_word_boundaries() {
        assert_eq!(lines("hello world again", 11), vec!["hello world", "again"]);
    }

    #[test]
    fn never_breaks_cjk_without_spaces() {
        assert_eq!(lines("你好世界", 4), vec!["你好世界"]);
    }

    #[test]
    fn protects_code_spans() {
        assert_eq!(lines("a `b c` d", 5), vec!["a", "`b c`", "d"]);
    }

    #[test]
    fn protects_link_destinations() {
        let text = "[text](https://example.com/some/path)";
        assert_eq!(lines(text, 10), vec![text.to_string()]);
    }

    #[test]
    fn protects_reference_labels() {
        let text = "[a b][some label] c";
        assert_eq!(lines(text, 18), vec!["[a b][some label]", "c"]);
    }

    #[test]
    fn never_starts_a_line_with_a_block_marker() {
        assert_eq!(lines("a - b", 2), vec!["a -", "b"]);
        assert_eq!(lines("a 1. b", 2), vec!["a 1.", "b"]);
        assert_eq!(lines("a > b", 2), vec!["a >", "b"]);
        for line in lines("one two - three # four > five", 6) {
            let first = line.split_whitespace().next().unwrap_or("");
            assert!(
                !line_start_sensitive(first),
                "line starts with block marker: {line:?}"
            );
        }
    }

    #[test]
    fn never_starts_a_line_with_html() {
        assert_eq!(
            lines("x <!-- hidden --> y", 8),
            vec!["x <!-- hidden -->", "y"]
        );
    }

    #[test]
    fn soft_breaks_do_not_become_hard_breaks() {
        assert_eq!(
            lines("quote with lazy continuation\nthis line is lazy", 80),
            vec!["quote with lazy continuation this line is lazy"]
        );
    }

    #[test]
    fn hard_break_spaces() {
        assert_eq!(lines("a  \nb", 80), vec!["a  ", "b"]);
    }

    #[test]
    fn hard_break_backslash() {
        let result = wrap("a\\\nb", 80, 80, HardBreakStyle::Backslash);
        assert_eq!(result, vec!["a\\", "b"]);
    }

    #[test]
    fn preserves_inline_markup_around_breaks() {
        assert_eq!(lines("**bold text**", 8), vec!["**bold", "text**"]);
    }

    #[test]
    fn keeps_entities() {
        assert_eq!(lines("a &nbsp; b", 80), vec!["a &nbsp; b"]);
    }
}
