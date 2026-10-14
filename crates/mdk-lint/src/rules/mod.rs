//! Lint rules.

pub(crate) mod code;
pub(crate) mod extra;
pub(crate) mod headings;
pub(crate) mod inline;
pub(crate) mod links;
pub(crate) mod lists;
pub(crate) mod rule;
pub(crate) mod tables;
pub(crate) mod whitespace;

use comrak::nodes::NodeValue;
use mdk_core::text;
use mdk_core::{Fix, Source};

use crate::context::{LineKind, LintContext};

pub use rule::{RULES, Rule, RuleMeta, rule_by_id, rule_by_name};

/// Whether a line is blank once blockquote markers are stripped.
pub(crate) fn is_loose_blank(line: &str) -> bool {
    let mut rest = line;
    loop {
        rest = rest.trim_start_matches([' ', '\t']);
        if let Some(stripped) = rest.strip_prefix('>') {
            rest = stripped;
        } else {
            break;
        }
    }
    text::is_blank(rest)
}

/// Counts blank lines immediately before a 0-based line.
pub(crate) fn blank_lines_before(ctx: &LintContext<'_, '_>, line: usize) -> usize {
    let mut count = 0;
    let mut index = line;
    while index > 0 {
        index -= 1;
        if matches!(
            ctx.line_kind(index),
            LineKind::Code | LineKind::Html | LineKind::FrontMatter
        ) {
            break;
        }
        if is_loose_blank(ctx.source.line(index)) {
            count += 1;
        } else {
            break;
        }
    }
    count
}

/// Counts blank lines immediately after a 0-based line (exclusive).
pub(crate) fn blank_lines_after(ctx: &LintContext<'_, '_>, line: usize) -> usize {
    let mut count = 0;
    let mut index = line + 1;
    while index < ctx.line_count() {
        if matches!(
            ctx.line_kind(index),
            LineKind::Code | LineKind::Html | LineKind::FrontMatter
        ) {
            break;
        }
        if is_loose_blank(ctx.source.line(index)) {
            count += 1;
            index += 1;
        } else {
            break;
        }
    }
    count
}

/// The last line covered by a node.
pub(crate) fn node_last_line(node: &comrak::nodes::AstNode<'_>) -> usize {
    let source = mdk_core::ast::node_lines(node);
    source.end.saturating_sub(1)
}

/// A fix inserting a blank line before a 0-based line.
pub(crate) fn fix_insert_blank_before(source: &Source, line: usize) -> Fix {
    let offset = source.line_range(line).start;
    Fix::new(offset..offset, "\n")
}

/// A fix inserting a blank line after a 0-based line.
pub(crate) fn fix_insert_blank_after(source: &Source, line: usize) -> Fix {
    let info = source.line_info(line);
    match info {
        Some(info) if info.newline_len() > 0 => {
            Fix::new(info.full_end()..info.full_end(), "\n")
        }
        _ => {
            let end = source.text().len();
            Fix::new(end..end, "\n\n")
        }
    }
}

/// Whether inserting blank lines around `node` is safe: it is not inside a
/// tight list, where a blank line would change rendering.
pub(crate) fn blank_lines_are_safe(node: &comrak::nodes::AstNode<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if let NodeValue::List(list) = &parent.data().value
            && list.tight
        {
            return false;
        }
        current = parent.parent();
    }
    true
}

/// The byte offset of the first non-whitespace character of a line.
pub(crate) fn first_content_offset(ctx: &LintContext<'_, '_>, line: usize) -> usize {
    let range = ctx.line_range(line);
    let content = ctx.source.slice(range.clone());
    let offset = content
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map_or(content.len(), |(index, _)| index);
    range.start + offset
}

/// A point span at the first content character of a line.
pub(crate) fn first_content_span(
    ctx: &LintContext<'_, '_>,
    line: usize,
) -> mdk_core::Span {
    let offset = first_content_offset(ctx, line);
    mdk_core::Span::point(ctx.source.position_of(offset))
}
