//! The formatting pipeline.
//!
//! Formatting runs in independent phases, re-parsing between them so each
//! phase sees offsets that match the current text:
//!
//! 1. whitespace: trailing whitespace, hard breaks, blank line management;
//! 2. structure: headings, list markers, code fences, tables, thematic
//!    breaks;
//! 3. reflow: wrap paragraphs to the configured width;
//! 4. delimiters: normalise emphasis and strong markers;
//! 5. finalisation: line endings and the trailing newline.
//!
//! When verification is enabled the result is rendered to HTML and compared
//! with the original; if they differ, formatting fails instead of returning
//! a document with changed semantics.

use std::ops::Range;

use comrak::nodes::{ListType, NodeValue};
use mdk_core::ast::{node_lines, node_span};
use mdk_core::text::{display_width, is_blank, trim_end};
use mdk_core::{Dialect, LineKind, Position, Source, classify, with_parsed_options};

use crate::options::{
    BulletStyle, DelimiterStyle, FenceStyle, FormatOptions, HeadingStyle,
    LineEndingStyle, OrderedNumbering,
};
use crate::prefix::{block_body_lines, block_prefix};
use crate::reflow;
use crate::tables;
use crate::verify;

/// The result of formatting a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatResult {
    /// The formatted document.
    pub output: String,
    /// Whether the output differs from the input.
    pub changed: bool,
}

/// Errors returned by the formatter.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    /// Formatting would have changed the rendered document, so the original
    /// text was kept. This indicates a formatter bug or an unsupported
    /// construct; `detail` describes the first HTML difference.
    #[error(
        "formatting would change the document's meaning; the file was left unchanged\n{detail}"
    )]
    SemanticChange {
        /// Description of the first rendered HTML difference.
        detail: String,
    },
}

/// Formats `text`, returning the rewritten document.
///
/// # Errors
///
/// Returns [`FormatError::SemanticChange`] when verification is enabled and
/// the formatted document does not render to the same HTML as the input.
pub fn format_text(
    text: &str,
    dialect: Dialect,
    options: &FormatOptions,
) -> Result<FormatResult, FormatError> {
    let mut current = text.to_string();
    if !current.is_empty() {
        current = phase_whitespace(&current, dialect, options);
        current = phase_structure(&current, dialect, options);
        if options.reflow {
            current = phase_reflow(&current, dialect, options);
        }
        current = phase_delimiters(&current, dialect, options);
        current = finalize(text, &current, options);
    }
    if options.verify {
        verify::verify_semantics(text, &current, dialect)
            .map_err(|detail| FormatError::SemanticChange { detail })?;
    }
    Ok(FormatResult {
        changed: current != text,
        output: current,
    })
}

#[derive(Debug, Clone, Default)]
struct LineEdit {
    delete: bool,
    replacement: Option<String>,
    insert_before: Vec<String>,
    insert_after: Vec<String>,
}

fn apply_line_edits(lines: &[String], edits: &[LineEdit]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        let edit = &edits[index];
        out.extend(edit.insert_before.iter().cloned());
        if !edit.delete {
            match &edit.replacement {
                Some(replacement) => out.push(replacement.clone()),
                None => out.push(line.clone()),
            }
        }
        out.extend(edit.insert_after.iter().cloned());
    }
    out
}

fn to_lines(text: &str) -> Vec<String> {
    Source::new(text.to_string())
        .lines()
        .map(ToString::to_string)
        .collect()
}

fn from_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        let mut text = lines.join("\n");
        text.push('\n');
        text
    }
}

fn is_loose_blank(line: &str) -> bool {
    let mut rest = line;
    loop {
        rest = rest.trim_start_matches([' ', '\t']);
        if let Some(stripped) = rest.strip_prefix('>') {
            rest = stripped;
        } else {
            break;
        }
    }
    is_blank(rest)
}

/// A blank line that may be removed: not part of a verbatim block.
fn trim_line(line: &str, kind: LineKind, options: &FormatOptions) -> String {
    if kind.is_verbatim() {
        return line.to_string();
    }
    if !options.trim_trailing_whitespace {
        return line.to_string();
    }
    let trimmed = trim_end(line);
    let trailing = line.len() - trimmed.len();
    if kind == LineKind::Normal && trailing >= 2 && !trimmed.is_empty() {
        return match options.hard_break_style {
            crate::options::HardBreakStyle::Spaces => format!("{trimmed}  "),
            crate::options::HardBreakStyle::Backslash => format!("{trimmed}\\"),
            crate::options::HardBreakStyle::Preserve => {
                format!("{trimmed}{}", " ".repeat(trailing))
            }
        };
    }
    trimmed.to_string()
}

// ---------------------------------------------------------------------------
// Phase 1: whitespace and blank lines
// ---------------------------------------------------------------------------

fn phase_whitespace(text: &str, dialect: Dialect, options: &FormatOptions) -> String {
    let source = Source::new(text.to_string());
    let lines = to_lines(text);
    if lines.is_empty() {
        return String::new();
    }

    let (kinds, insertions) = {
        let comrak_options = dialect.comrak_options();
        with_parsed_options(&source, &comrak_options, |root| {
            let kinds = classify(&source, root);
            let insertions = if options.blank_lines_around_blocks {
                blank_insertions(&source, root)
            } else {
                Vec::new()
            };
            (kinds, insertions)
        })
    };

    let mut before: Vec<Vec<String>> = vec![Vec::new(); lines.len()];
    for line in insertions {
        if line < lines.len() {
            before[line].push(String::new());
        }
    }
    if options.blank_line_after_front_matter
        && let Some(range) = source.front_matter_range()
    {
        let last_front_matter_line = source.line_index_of(range.end.saturating_sub(1));
        let next = last_front_matter_line + 1;
        if next < lines.len() && !is_blank(&lines[next]) {
            before[next].push(String::new());
        }
    }

    let maximum = options.max_blank_lines.max(1);
    let mut collected: Vec<(String, bool)> = Vec::with_capacity(lines.len());
    let mut blank_run = 0usize;
    for (index, line) in lines.iter().enumerate() {
        for _ in &before[index] {
            collected.push((String::new(), true));
        }
        let kind = kinds[index];
        let processed = trim_line(line, kind, options);
        let removable_blank =
            kind == LineKind::Blank || (processed.is_empty() && !kind.is_verbatim());
        if removable_blank {
            blank_run += 1;
            if blank_run <= maximum {
                collected.push((processed, true));
            }
        } else {
            blank_run = 0;
            collected.push((processed, false));
        }
    }

    let mut start = 0;
    while start < collected.len() && collected[start].1 && collected[start].0.is_empty()
    {
        start += 1;
    }
    let mut end = collected.len();
    while end > start && collected[end - 1].1 && collected[end - 1].0.is_empty() {
        end -= 1;
    }
    let result: Vec<String> = collected[start..end]
        .iter()
        .map(|(text, _)| text.clone())
        .collect();
    from_lines(&result)
}

/// Line indices where a blank line should be inserted before the line.
fn blank_insertions(source: &Source, root: comrak::Node<'_>) -> Vec<usize> {
    let mut insertions = Vec::new();
    for parent in root.descendants() {
        let allowed = match &parent.data().value {
            NodeValue::Document | NodeValue::BlockQuote | NodeValue::FootnoteDefinition(_) => true,
            NodeValue::Item(_) | NodeValue::TaskItem(_) => parent.parent().is_some_and(|list| {
                matches!(&list.data().value, NodeValue::List(list) if !list.tight)
            }),
            _ => false,
        };
        if !allowed {
            continue;
        }
        let mut previous: Option<comrak::Node<'_>> = None;
        for child in parent.children() {
            if let Some(previous) = previous {
                let previous_end = node_lines(previous).end;
                let next_start = child.data().sourcepos.start.line.saturating_sub(1);
                if next_start > previous_end
                    && !(previous_end..next_start)
                        .any(|line| is_loose_blank(source.line(line)))
                {
                    insertions.push(next_start);
                }
            }
            previous = Some(child);
        }
    }
    insertions
}

// ---------------------------------------------------------------------------
// Phase 2: block structure
// ---------------------------------------------------------------------------

fn phase_structure(text: &str, dialect: Dialect, options: &FormatOptions) -> String {
    let source = Source::new(text.to_string());
    let lines = to_lines(text);
    let mut edits = vec![LineEdit::default(); lines.len()];
    let comrak_options = dialect.comrak_options();
    with_parsed_options(&source, &comrak_options, |root| {
        collect_heading_edits(&source, root, options, &mut edits);
        collect_list_edits(&source, root, options, &mut edits);
        collect_fence_edits(&source, root, options, &mut edits);
        collect_table_edits(&source, root, options, &mut edits);
        collect_thematic_break_edits(&source, root, options, &mut edits);
    });
    from_lines(&apply_line_edits(&lines, &edits))
}

fn split_closing_hashes(body: &str) -> (&str, bool) {
    let trimmed = body.trim_end();
    let hashes = trimmed.len() - trimmed.trim_end_matches('#').len();
    if hashes == 0 {
        return (trimmed, false);
    }
    let before = &trimmed[..trimmed.len() - hashes];
    if before.is_empty() || before.ends_with([' ', '\t']) {
        (before.trim_end(), true)
    } else {
        (trimmed, false)
    }
}

fn collect_heading_edits(
    source: &Source,
    root: comrak::Node<'_>,
    options: &FormatOptions,
    edits: &mut [LineEdit],
) {
    for node in root.descendants() {
        let NodeValue::Heading(heading) = &node.data().value else {
            continue;
        };
        let lines = node_lines(node);
        let first = lines.start;
        let last = lines.end.saturating_sub(1);
        let Some(prefix) = block_prefix(source, node) else {
            continue;
        };
        let level = heading.level as usize;

        if heading.setext {
            let body = block_body_lines(source, &prefix);
            let text_lines = body
                .iter()
                .take(body.len().saturating_sub(1))
                .map(|line| line.trim())
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let text = text_lines.trim();
            match options.heading_style {
                HeadingStyle::Atx | HeadingStyle::AtxClosed => {
                    let hashes = "#".repeat(level.max(1));
                    let replacement = if text.is_empty() {
                        format!("{}{hashes}", prefix.first)
                    } else if options.heading_style == HeadingStyle::AtxClosed {
                        format!("{}{hashes} {text} {hashes}", prefix.first)
                    } else {
                        format!("{}{hashes} {text}", prefix.first)
                    };
                    edits[first].replacement = Some(replacement);
                    for edit in &mut edits[first + 1..=last] {
                        edit.delete = true;
                    }
                }
                HeadingStyle::Setext => {
                    let underline = if level == 1 { "=" } else { "-" };
                    let width = display_width(text).max(3);
                    edits[last].replacement = Some(format!(
                        "{}{}",
                        prefix.continuation,
                        underline.repeat(width)
                    ));
                }
                HeadingStyle::Preserve => {}
            }
            continue;
        }

        let line_start = source.line_range(first).start;
        let rest = &source.slice(prefix.body_start..source.line_range(first).end);
        let hashes = rest.bytes().take_while(|byte| *byte == b'#').count();
        if hashes == 0 {
            continue;
        }
        let after_hashes = rest[hashes..].trim_start_matches([' ', '\t']);
        let (content, had_closing) = split_closing_hashes(after_hashes);
        let marker = "#".repeat(hashes.max(level));

        let replacement = match options.heading_style {
            HeadingStyle::Preserve => {
                if content.is_empty() {
                    Some(format!("{}{marker}", prefix.first))
                } else if had_closing {
                    Some(format!("{}{marker} {content} {marker}", prefix.first))
                } else {
                    Some(format!("{}{marker} {content}", prefix.first))
                }
            }
            HeadingStyle::Atx => Some(if content.is_empty() {
                format!("{}{marker}", prefix.first)
            } else {
                format!("{}{marker} {content}", prefix.first)
            }),
            HeadingStyle::AtxClosed => Some(if content.is_empty() {
                format!("{}{marker}", prefix.first)
            } else {
                format!("{}{marker} {content} {marker}", prefix.first)
            }),
            HeadingStyle::Setext => {
                if level <= 2 && !content.is_empty() {
                    let underline = if level == 1 { "=" } else { "-" };
                    let width = display_width(content).max(3);
                    edits[first].replacement =
                        Some(format!("{}{content}", prefix.first));
                    edits[first].insert_after.push(format!(
                        "{}{}",
                        prefix.continuation,
                        underline.repeat(width)
                    ));
                    None
                } else {
                    Some(format!("{}{marker} {content}", prefix.first))
                }
            }
        };
        let _ = line_start;
        if let Some(replacement) = replacement
            && source.line(first) != replacement
        {
            edits[first].replacement = Some(replacement);
        }
    }
}

struct MarkerInfo {
    number: Option<usize>,
    delimiter: Option<char>,
    marker_len: usize,
    spaces: usize,
}

fn parse_marker(text: &str) -> Option<MarkerInfo> {
    let bytes = text.as_bytes();
    let first = *bytes.first()?;
    match first {
        b'-' | b'+' | b'*' => Some(MarkerInfo {
            number: None,
            delimiter: None,
            marker_len: 1,
            spaces: count_spaces(&text[1..]),
        }),
        b'0'..=b'9' => {
            let digits = bytes
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            let delimiter = *bytes.get(digits)?;
            if delimiter != b'.' && delimiter != b')' {
                return None;
            }
            let number = text[..digits].parse().ok()?;
            Some(MarkerInfo {
                number: Some(number),
                delimiter: Some(delimiter as char),
                marker_len: digits + 1,
                spaces: count_spaces(&text[digits + 1..]),
            })
        }
        _ => None,
    }
}

fn count_spaces(text: &str) -> usize {
    text.len() - text.trim_start_matches([' ', '\t']).len()
}

struct MarkerEdit {
    /// Container prefix of the marker line, for example `> ` or `  `.
    container: String,
    /// Absolute byte offset where the item content begins.
    content_offset: usize,
    marker: String,
    spaces: usize,
}

fn collect_list_edits(
    source: &Source,
    root: comrak::Node<'_>,
    options: &FormatOptions,
    edits: &mut [LineEdit],
) {
    let line_count = source.line_count();
    let lines: Vec<String> = source.lines().map(ToString::to_string).collect();
    let mut indent_delta = vec![0isize; line_count];
    let mut marker_edits: Vec<Option<MarkerEdit>> =
        (0..line_count).map(|_| None).collect();

    for node in root.descendants() {
        if !matches!(
            node.data().value,
            NodeValue::Item(_) | NodeValue::TaskItem(_)
        ) {
            continue;
        }
        let Some(list) = node.parent() else { continue };
        let NodeValue::List(list_meta) = &list.data().value else {
            continue;
        };
        let lines_range = node_lines(node);
        let first = lines_range.start;
        let last = lines_range.end.saturating_sub(1);
        let sourcepos = node.data().sourcepos;
        let Some(marker_offset) = source
            .offset_of(Position::new(sourcepos.start.line, sourcepos.start.column))
        else {
            continue;
        };
        if !source.text().is_char_boundary(marker_offset) {
            continue;
        }
        let line_end = source.line_range(first).end;
        let Some(marker) = parse_marker(source.slice(marker_offset..line_end)) else {
            continue;
        };
        let Some(prefix) = block_prefix(source, node) else {
            continue;
        };
        // The container prefix is everything before the marker; the padding
        // region between marker and content may be shorter than the spaces
        // written after the marker when the rest is an indented code block.
        let used_spaces = prefix
            .body_start
            .saturating_sub(marker_offset + marker.marker_len);
        if used_spaces != marker.spaces
            || prefix.first.len() < marker.marker_len + used_spaces
        {
            continue;
        }
        let container = prefix.first
            [..prefix.first.len() - marker.marker_len - used_spaces]
            .to_string();

        let mut item_index = None;
        for (index, child) in list
            .children()
            .filter(|child| {
                matches!(
                    child.data().value,
                    NodeValue::Item(_) | NodeValue::TaskItem(_)
                )
            })
            .enumerate()
        {
            if child.same_node(node) {
                item_index = Some(index);
                break;
            }
        }
        let item_index = item_index.unwrap_or(0);

        let marker_text = match list_meta.list_type {
            ListType::Bullet => source.text()[marker_offset..]
                .chars()
                .next()
                .unwrap_or('-')
                .to_string(),
            ListType::Ordered => {
                let number = match options.ordered_list_numbering {
                    OrderedNumbering::Preserve => marker.number.unwrap_or(1),
                    OrderedNumbering::Sequential => list_meta.start + item_index,
                    OrderedNumbering::One => 1,
                };
                let delimiter = marker.delimiter.unwrap_or('.');
                format!("{number}{delimiter}")
            }
        };
        let new_marker = if list_meta.list_type == ListType::Bullet {
            // Changing the bullet character of a list that sits next to
            // another bullet list would merge the two lists, changing tight
            // and loose rendering, so such lists keep their markers.
            let risky = has_adjacent_bullet_list(list);
            match options.list_bullet {
                BulletStyle::Preserve => marker_text,
                _ if risky => marker_text,
                BulletStyle::Dash => "-".to_string(),
                BulletStyle::Asterisk => "*".to_string(),
                BulletStyle::Plus => "+".to_string(),
            }
        } else {
            marker_text
        };
        let spaces = options.list_marker_space;
        let delta = (new_marker.len() + spaces) as isize
            - (marker.marker_len + marker.spaces) as isize;

        if delta != 0 && !shift_guard(&lines, first, last, delta, &container) {
            continue;
        }

        marker_edits[first] = Some(MarkerEdit {
            container,
            content_offset: prefix.body_start,
            marker: new_marker,
            spaces,
        });
        for line in first + 1..=last {
            if let Some(slot) = indent_delta.get_mut(line) {
                *slot += delta;
            }
        }
    }

    for line in 0..line_count {
        let delta = indent_delta[line];
        let marker = marker_edits[line].take();
        if delta == 0 && marker.is_none() {
            continue;
        }
        let text = &lines[line];
        let line_start = source.line_range(line).start;
        let replacement = if let Some(marker) = marker {
            let container = adjust_container(&marker.container, delta);
            let mut value = container;
            value.push_str(&marker.marker);
            value.push_str(&" ".repeat(marker.spaces));
            let content = marker.content_offset.saturating_sub(line_start);
            if content <= text.len() {
                value.push_str(&text[content..]);
            }
            value
        } else {
            let original_indent = text.len() - text.trim_start_matches(' ').len();
            let indent = (original_indent as isize + delta).max(0) as usize;
            format!("{}{}", " ".repeat(indent), &text[original_indent..])
        };
        if &replacement != text {
            edits[line].replacement = Some(replacement);
        }
    }
}

/// Whether `list` has an adjacent bullet-list sibling.
fn has_adjacent_bullet_list(list: comrak::Node<'_>) -> bool {
    let is_bullet = |node: &comrak::nodes::AstNode<'_>| matches!(&node.data().value, NodeValue::List(list) if list.list_type == ListType::Bullet);
    list.previous_sibling().is_some_and(is_bullet)
        || list.next_sibling().is_some_and(is_bullet)
}

/// Adjusts the trailing spaces of a container prefix by `delta`.
fn adjust_container(container: &str, delta: isize) -> String {
    if delta == 0 {
        return container.to_string();
    }
    let spaces = container.len() - container.trim_end_matches(' ').len();
    let adjusted = (spaces as isize + delta).max(0) as usize;
    format!(
        "{}{}",
        &container[..container.len() - spaces],
        " ".repeat(adjusted)
    )
}

/// Whether continuation lines of an item can be shifted by `delta`.
fn shift_guard(
    lines: &[String],
    first: usize,
    last: usize,
    delta: isize,
    container: &str,
) -> bool {
    if delta == 0 {
        return true;
    }
    if delta < 0 {
        let spaces = container.len() - container.trim_end_matches(' ').len();
        if spaces < (-delta) as usize {
            return false;
        }
    }
    for line in first + 1..=last {
        let Some(text) = lines.get(line) else { break };
        if is_blank(text) {
            continue;
        }
        let whitespace =
            &text[..text.len() - text.trim_start_matches([' ', '\t']).len()];
        if whitespace.contains('\t') {
            return false;
        }
        if delta < 0 && whitespace.len() < (-delta) as usize {
            return false;
        }
    }
    true
}

fn collect_fence_edits(
    source: &Source,
    root: comrak::Node<'_>,
    options: &FormatOptions,
    edits: &mut [LineEdit],
) {
    for node in root.descendants() {
        let NodeValue::CodeBlock(code) = &node.data().value else {
            continue;
        };
        if !code.fenced {
            continue;
        }
        let lines = node_lines(node);
        let first = lines.start;
        let last = lines.end.saturating_sub(1);
        let opening = source.line(first);
        let Some(prefix) = block_prefix(source, node) else {
            continue;
        };

        let mut fence_char = code.fence_char as char;
        match options.code_fence_style {
            FenceStyle::Preserve => {}
            FenceStyle::Backtick => fence_char = '`',
            FenceStyle::Tilde => fence_char = '~',
        }
        let info = code.info.trim();
        if info.contains(fence_char) {
            fence_char = code.fence_char as char;
        }
        let run = longest_run(&code.literal, fence_char);
        let length = code.fence_length.max(3).max(run + 1);
        let fence: String = std::iter::repeat_n(fence_char, length).collect();
        let opening_replacement = if info.is_empty() {
            format!("{}{fence}", prefix.first)
        } else {
            format!("{}{fence}{info}", prefix.first)
        };
        if opening_replacement != opening {
            edits[first].replacement = Some(opening_replacement);
        }

        if code.closed && last > first {
            let closing = source.line(last);
            let closing_replacement = format!("{}{fence}", prefix.continuation);
            if closing_replacement != closing {
                edits[last].replacement = Some(closing_replacement);
            }
        } else if !code.closed && options.close_code_fences {
            edits[last]
                .insert_after
                .push(format!("{}{fence}", prefix.continuation));
        }
    }
}

fn longest_run(text: &str, ch: char) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for candidate in text.chars() {
        if candidate == ch {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

fn collect_table_edits(
    source: &Source,
    root: comrak::Node<'_>,
    options: &FormatOptions,
    edits: &mut [LineEdit],
) {
    if !options.align_tables {
        return;
    }
    for node in root.descendants() {
        let NodeValue::Table(table) = &node.data().value else {
            continue;
        };
        let lines = node_lines(node);
        let text_lines: Vec<&str> = (lines.start..lines.end)
            .map(|line| source.line(line))
            .collect();
        let Some(mut parsed) = tables::parse(&text_lines, table) else {
            continue;
        };
        if let Some(prefix) = block_prefix(source, node) {
            parsed.indent = prefix.continuation;
        }
        for (offset, rendered) in tables::render(&parsed, options).iter().enumerate() {
            let line = lines.start + offset;
            if source.line(line) != rendered {
                edits[line].replacement = Some(rendered.clone());
            }
        }
    }
}

fn collect_thematic_break_edits(
    source: &Source,
    root: comrak::Node<'_>,
    options: &FormatOptions,
    edits: &mut [LineEdit],
) {
    let Some(marker) = options.thematic_break_marker() else {
        return;
    };
    for node in root.descendants() {
        if !matches!(node.data().value, NodeValue::ThematicBreak) {
            continue;
        }
        // `---` directly after a paragraph would become a setext heading.
        if node.previous_sibling().is_some_and(|previous| {
            matches!(previous.data().value, NodeValue::Paragraph)
        }) {
            continue;
        }
        let line = node.data().sourcepos.start.line.saturating_sub(1);
        let Some(prefix) = block_prefix(source, node) else {
            continue;
        };
        let replacement = format!("{}{marker}", prefix.continuation);
        if source.line(line) != replacement {
            edits[line].replacement = Some(replacement);
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 3: paragraph reflow
// ---------------------------------------------------------------------------

fn phase_reflow(text: &str, dialect: Dialect, options: &FormatOptions) -> String {
    let source = Source::new(text.to_string());
    let lines = to_lines(text);
    let mut edits = vec![LineEdit::default(); lines.len()];
    let comrak_options = dialect.comrak_options();
    with_parsed_options(&source, &comrak_options, |root| {
        let kinds = classify(&source, root);
        for node in root.descendants() {
            if !matches!(node.data().value, NodeValue::Paragraph) {
                continue;
            }
            let range = node_lines(node);
            if range
                .clone()
                .any(|line| kinds[line].is_verbatim() || kinds[line] == LineKind::Table)
            {
                continue;
            }
            let Some(prefix) = block_prefix(&source, node) else {
                continue;
            };
            let body = block_body_lines(&source, &prefix);
            if body.iter().all(|line| line.trim().is_empty()) {
                continue;
            }
            if body
                .iter()
                .any(|line| looks_like_reference_definition(line))
            {
                continue;
            }
            let joined = body.join("\n");
            let first_available = options
                .line_width
                .saturating_sub(display_width(&prefix.first));
            let continuation_available = options
                .line_width
                .saturating_sub(prefix.continuation_width());
            let wrapped = reflow::wrap(
                &joined,
                first_available,
                continuation_available,
                options.hard_break_style,
            );
            edits[range.start].replacement =
                Some(format!("{}{}", prefix.first, wrapped[0]));
            for edit in &mut edits[range.start + 1..range.end] {
                edit.delete = true;
            }
            let last = range.end.saturating_sub(1);
            for extra in &wrapped[1..] {
                edits[last]
                    .insert_after
                    .push(format!("{}{}", prefix.continuation, extra));
            }
        }
    });
    from_lines(&apply_line_edits(&lines, &edits))
}

fn looks_like_reference_definition(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix('[') else {
        return false;
    };
    let Some(close) = rest.find(']') else {
        return false;
    };
    rest[..close].chars().all(|ch| ch != '\n') && rest[close..].starts_with("]:")
}

// ---------------------------------------------------------------------------
// Phase 4: emphasis delimiters
// ---------------------------------------------------------------------------

fn phase_delimiters(text: &str, dialect: Dialect, options: &FormatOptions) -> String {
    if options.emphasis_style == DelimiterStyle::Preserve
        && options.strong_style == DelimiterStyle::Preserve
    {
        return text.to_string();
    }
    let source = Source::new(text.to_string());
    let mut replacements: Vec<(Range<usize>, String)> = Vec::new();
    let comrak_options = dialect.comrak_options();
    with_parsed_options(&source, &comrak_options, |root| {
        for node in root.descendants() {
            let (count, style) = match &node.data().value {
                NodeValue::Emph => (1usize, options.emphasis_style),
                NodeValue::Strong => (2usize, options.strong_style),
                _ => continue,
            };
            let desired = match style {
                DelimiterStyle::Preserve => continue,
                DelimiterStyle::Asterisk => '*',
                DelimiterStyle::Underscore => '_',
            };
            let range = source.span_range(node_span(node));
            let raw = source.slice(range.clone());
            let Some(actual) = raw.chars().next().filter(|ch| *ch == '*' || *ch == '_')
            else {
                continue;
            };
            if actual == desired {
                continue;
            }
            let lead = actual.to_string().repeat(count);
            let Some(inner) = raw
                .strip_prefix(&lead)
                .and_then(|rest| rest.strip_suffix(&lead))
            else {
                continue;
            };
            let delimiter = desired.to_string().repeat(count);
            replacements.push((range, format!("{delimiter}{inner}{delimiter}")));
        }
    });
    apply_text_edits(text, replacements)
}

fn apply_text_edits(text: &str, mut edits: Vec<(Range<usize>, String)>) -> String {
    edits.sort_by_key(|(range, _)| (range.start, range.end));
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for (range, replacement) in edits {
        if range.start < cursor || range.end > text.len() {
            continue;
        }
        out.push_str(&text[cursor..range.start]);
        out.push_str(&replacement);
        cursor = range.end;
    }
    out.push_str(&text[cursor..]);
    out
}

// ---------------------------------------------------------------------------
// Finalisation
// ---------------------------------------------------------------------------

fn finalize(original: &str, formatted: &str, options: &FormatOptions) -> String {
    if formatted.is_empty() {
        return String::new();
    }
    let ending = match options.line_ending {
        LineEndingStyle::Lf => "\n",
        LineEndingStyle::Crlf => "\r\n",
        LineEndingStyle::Preserve => {
            match Source::new(original.to_string()).line_ending() {
                mdk_core::LineEnding::Crlf => "\r\n",
                _ => "\n",
            }
        }
    };
    let body = formatted.trim_end_matches(['\n', '\r']);
    if options.final_newline {
        format!("{body}{ending}")
    } else {
        body.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(markdown: &str) -> String {
        format_with(markdown, Dialect::Commonmark)
    }

    fn format_with(markdown: &str, dialect: Dialect) -> String {
        format_text(markdown, dialect, &FormatOptions::default())
            .expect("formatting should succeed")
            .output
    }

    #[test]
    fn normalises_whitespace_and_final_newline() {
        assert_eq!(format("hello   \n\n\n\nworld"), "hello\n\nworld\n");
    }

    #[test]
    fn wraps_long_paragraphs() {
        let input = format!("{}\n", "word ".repeat(30));
        let output = format(&input);
        for line in output.lines() {
            let width = display_width(line);
            assert!(width <= 80, "line too wide ({width}): {line:?}");
        }
        assert!(output.ends_with('\n'));
    }

    #[test]
    fn keeps_front_matter_verbatim() {
        let output = format("---\ntitle:  spaced  \n---\n\nbody\n");
        assert!(output.starts_with("---\ntitle:  spaced  \n---\n"));
    }

    #[test]
    fn converts_setext_to_atx() {
        assert_eq!(format("Title\n=====\n"), "# Title\n");
    }

    #[test]
    fn reflows_list_items_with_indent() {
        let output =
            format("- item one that is quite long and will need to wrap around\n");
        for line in output.lines().skip(1) {
            assert!(line.starts_with("  "), "continuation line: {line:?}");
        }
    }

    #[test]
    fn preserves_hard_breaks() {
        assert_eq!(format("a  \nb\n"), "a  \nb\n");
    }

    #[test]
    fn aligns_tables() {
        let output = format_with("| a | b |\n|---|---|\n| 1 | 2 |\n", Dialect::Gfm);
        assert_eq!(output, "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n");
    }

    #[test]
    fn is_idempotent() {
        let input = "# Title\n\nSome   text that   needs reflowing because it is quite long indeed and keeps going.\n\n- a\n- b\n";
        let once = format(input);
        let twice = format(&once);
        assert_eq!(once, twice);
    }
}
