//! Container prefix computation for blocks inside blockquotes and lists.

use comrak::nodes::NodeValue;
use mdk_core::{Position, Source};

/// The container prefix in front of a leaf block.
///
/// `first` is the literal text before the block's content on its first line
/// (for example `> - `), `continuation` is the same prefix with list and
/// footnote markers replaced by spaces (for example `>   `), and
/// `body_start` is the byte offset where the content begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Prefix {
    pub first: String,
    pub continuation: String,
    pub body_start: usize,
    pub first_line: usize,
    pub last_line: usize,
}

impl Prefix {
    /// The display width of the continuation prefix.
    pub fn continuation_width(&self) -> usize {
        mdk_core::text::display_width(&self.continuation)
    }
}

/// Computes the container prefix for `node`.
///
/// Returns `None` for containers the formatter does not understand
/// (multiline blockquotes, directives, description lists, alerts), for
/// prefixes containing tabs, or when positions cannot be mapped.
pub(crate) fn block_prefix(source: &Source, node: comrak::Node<'_>) -> Option<Prefix> {
    let sourcepos = node.data().sourcepos;
    let start_line = sourcepos.start.line.checked_sub(1)?;
    let line_info = source.line_info(start_line)?;
    let line_start = line_info.start();
    let mut cursor = line_start;

    let ancestors: Vec<comrak::Node<'_>> = node.ancestors().collect();
    for ancestor in ancestors.iter().rev() {
        let container = ancestor.data();
        match &container.value {
            NodeValue::Document | NodeValue::List(_) => {}
            NodeValue::BlockQuote => {
                let start = container.sourcepos.start;
                if start.line == start_line + 1 {
                    let offset =
                        source.offset_of(Position::new(start.line, start.column))?;
                    cursor = offset.max(cursor) + 1;
                    if source.text().as_bytes().get(cursor) == Some(&b' ') {
                        cursor += 1;
                    }
                } else {
                    cursor = skip_blockquote_marker(source, cursor);
                }
            }
            NodeValue::Item(item) => {
                cursor = item_content_offset(
                    source,
                    ancestor,
                    item.padding,
                    cursor,
                    start_line,
                )?;
            }
            NodeValue::TaskItem(_) => {
                let padding =
                    match ancestor.parent().map(|parent| parent.data().value.clone()) {
                        Some(NodeValue::List(list)) => list.padding,
                        _ => return None,
                    };
                cursor =
                    item_content_offset(source, ancestor, padding, cursor, start_line)?;
            }
            NodeValue::FootnoteDefinition(definition) => {
                let start = container.sourcepos.start;
                if start.line == start_line + 1 {
                    let offset =
                        source.offset_of(Position::new(start.line, start.column))?;
                    cursor = offset.max(cursor) + definition.name.chars().count() + 4;
                    if source.text().as_bytes().get(cursor) == Some(&b' ') {
                        cursor += 1;
                    }
                } else {
                    let target = start.column + definition.name.chars().count() + 5;
                    cursor = advance_to_column(source, cursor, start_line, target);
                }
            }
            NodeValue::MultilineBlockQuote(_)
            | NodeValue::BlockDirective(_)
            | NodeValue::DescriptionList
            | NodeValue::DescriptionItem(_)
            | NodeValue::DescriptionDetails
            | NodeValue::Alert(_)
            | NodeValue::Subtext => return None,
            _ => {}
        }
    }

    if cursor < line_start
        || cursor > line_info.end()
        || !source.text().is_char_boundary(cursor)
    {
        return None;
    }
    let first = source.slice(line_start..cursor).to_string();
    if first.contains('\t') {
        return None;
    }
    let continuation: String = first
        .chars()
        .map(|ch| if ch == '>' || ch == ' ' { ch } else { ' ' })
        .collect();
    let last_line = mdk_core::ast::node_lines(node).end.saturating_sub(1);
    Some(Prefix {
        first,
        continuation,
        body_start: cursor,
        first_line: start_line,
        last_line,
    })
}

fn item_content_offset(
    source: &Source,
    item: &comrak::nodes::AstNode<'_>,
    padding: usize,
    cursor: usize,
    start_line: usize,
) -> Option<usize> {
    let sourcepos = item.data().sourcepos;
    if sourcepos.start.line == start_line + 1 {
        let offset = source
            .offset_of(Position::new(sourcepos.start.line, sourcepos.start.column))?;
        Some(advance_chars(source.text(), offset.max(cursor), padding))
    } else {
        // The item marker is on an earlier line. Advance over at most the
        // spaces required to reach the item's content column, so that a
        // fence's own extra indentation stays part of the block.
        let target = sourcepos.start.column + padding;
        Some(advance_to_column(source, cursor, start_line, target))
    }
}

/// Advances `cursor` over spaces until it reaches `target` (1-based column),
/// or the first non-space character.
fn advance_to_column(
    source: &Source,
    cursor: usize,
    line: usize,
    target: usize,
) -> usize {
    let line_start = source.line_range(line).start;
    let text = source.text();
    let mut offset = cursor.max(line_start);
    let mut column = text[line_start..offset].chars().count() + 1;
    while column < target {
        match text[offset..].chars().next() {
            Some(' ') => {
                offset += 1;
                column += 1;
            }
            _ => break,
        }
    }
    offset
}

fn advance_chars(text: &str, mut offset: usize, count: usize) -> usize {
    for _ in 0..count {
        match text[offset..].chars().next() {
            Some(ch) => offset += ch.len_utf8(),
            None => break,
        }
    }
    offset
}

fn skip_spaces(source: &Source, mut cursor: usize) -> usize {
    let bytes = source.text().as_bytes();
    while bytes.get(cursor) == Some(&b' ') {
        cursor += 1;
    }
    cursor
}

/// Skips a single blockquote marker (`>` with optional following space).
///
/// Called for containers that started on an earlier line, where each ancestor
/// contributes exactly one marker level to the current line.
fn skip_blockquote_marker(source: &Source, cursor: usize) -> usize {
    let bytes = source.text().as_bytes();
    let mut current = skip_spaces(source, cursor);
    if bytes.get(current) == Some(&b'>') {
        current += 1;
        if bytes.get(current) == Some(&b' ') {
            current += 1;
        }
        current
    } else {
        cursor
    }
}

/// Extracts a block's content lines, stripping the container prefix.
///
/// The first line starts at [`Prefix::body_start`]; continuation lines are
/// stripped of the continuation prefix when present. Lazy continuation lines
/// (which omit the prefix) are returned unchanged.
pub(crate) fn block_body_lines(source: &Source, prefix: &Prefix) -> Vec<String> {
    let mut lines = Vec::with_capacity(prefix.last_line + 1 - prefix.first_line);
    for line in prefix.first_line..=prefix.last_line {
        let content = if line == prefix.first_line {
            let end = source.line_range(line).end;
            source.slice(prefix.body_start..end)
        } else {
            let text = source.line(line);
            strip_container_prefix(text, &prefix.continuation)
        };
        lines.push(content.to_string());
    }
    lines
}

/// Strips as much of `pattern` from `line` as matches character by
/// character.
///
/// Unlike [`str::strip_prefix`] this tolerates lazy continuation lines that
/// omit the item indentation but keep the blockquote markers: `> item`
/// followed by `> lazy` strips only `> ` instead of failing entirely.
fn strip_container_prefix<'a>(line: &'a str, pattern: &str) -> &'a str {
    let mut position = 0usize;
    for expected in pattern.chars() {
        match line[position..].chars().next() {
            Some(ch) if ch == expected => position += ch.len_utf8(),
            _ => break,
        }
    }
    &line[position..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use comrak::nodes::NodeValue as V;
    use mdk_core::{Dialect, with_parsed_options};

    fn prefix_of(markdown: &str, wanted_line: usize) -> Option<Prefix> {
        let source = Source::new(markdown.to_string());
        let options = Dialect::Gfm.comrak_options();
        with_parsed_options(&source, &options, |root| {
            root.descendants()
                .filter(|node| matches!(node.data().value, V::Paragraph))
                .find(|node| node.data().sourcepos.start.line == wanted_line)
                .and_then(|node| block_prefix(&source, node))
        })
    }

    #[test]
    fn plain_paragraph_has_no_prefix() {
        let prefix = prefix_of("hello\n", 1).expect("prefix");
        assert_eq!(prefix.first, "");
        assert_eq!(prefix.continuation, "");
        assert_eq!(prefix.body_start, 0);
        assert_eq!(prefix.last_line, 0);
    }

    #[test]
    fn blockquote_prefix() {
        let prefix = prefix_of("> quoted\n> continued\n", 1).expect("prefix");
        assert_eq!(prefix.first, "> ");
        assert_eq!(prefix.continuation, "> ");
    }

    #[test]
    fn list_prefix_becomes_spaces() {
        let prefix = prefix_of("- item\n  continued\n", 1).expect("prefix");
        assert_eq!(prefix.first, "- ");
        assert_eq!(prefix.continuation, "  ");
    }

    #[test]
    fn nested_prefix() {
        let prefix = prefix_of("> - item\n>   continued\n", 1).expect("prefix");
        assert_eq!(prefix.first, "> - ");
        assert_eq!(prefix.continuation, ">   ");
    }

    #[test]
    fn task_item_keeps_checkbox_in_body() {
        let prefix = prefix_of("- [x] done\n", 1).expect("prefix");
        assert_eq!(prefix.first, "- ");
        assert_eq!(prefix.continuation, "  ");
        assert_eq!(prefix.body_start, 2);
    }

    #[test]
    fn footnote_prefix() {
        // A footnote definition is only parsed as such when referenced.
        let prefix = prefix_of("note[^a]\n\n[^a]: body\n", 3).expect("prefix");
        assert_eq!(prefix.first, "[^a]: ");
        assert_eq!(prefix.continuation, "      ");
    }

    #[test]
    fn lazy_continuation_keeps_only_actual_prefix() {
        let markdown = "> - item\n> lazy continuation\n";
        let source = Source::new(markdown.to_string());
        let options = Dialect::Gfm.comrak_options();
        with_parsed_options(&source, &options, |root| {
            let node = root
                .descendants()
                .find(|node| matches!(node.data().value, V::Paragraph))
                .expect("paragraph");
            let prefix = block_prefix(&source, node).expect("prefix");
            let body = block_body_lines(&source, &prefix);
            assert_eq!(
                body,
                vec!["item".to_string(), "lazy continuation".to_string()]
            );
        });
    }

    #[test]
    fn body_lines_are_stripped() {
        let markdown = "- item one\n  continued\n";
        let source = Source::new(markdown.to_string());
        let options = Dialect::Gfm.comrak_options();
        with_parsed_options(&source, &options, |root| {
            let node = root
                .descendants()
                .find(|node| matches!(node.data().value, V::Paragraph))
                .expect("paragraph");
            let prefix = block_prefix(&source, node).expect("prefix");
            let body = block_body_lines(&source, &prefix);
            assert_eq!(body, vec!["item one".to_string(), "continued".to_string()]);
        });
    }
}
