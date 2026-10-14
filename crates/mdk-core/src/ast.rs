//! AST helpers shared by lint, format and render stages.

use std::ops::Range;

use comrak::Node;
use comrak::nodes::NodeValue;

use crate::{Position, Source, Span};

/// The 0-based, half-open line range covered by a node.
///
/// `comrak` sometimes reports an end position of `(line, 0)`, meaning the
/// node ends just before that line; this function compensates so callers can
/// iterate the lines a node really exists on.
#[must_use]
pub fn node_lines(node: &comrak::nodes::AstNode<'_>) -> Range<usize> {
    let sourcepos = node.data().sourcepos;
    let start = sourcepos.start.line.saturating_sub(1);
    let mut end = sourcepos.end.line.saturating_sub(1);
    if sourcepos.end.column == 0 && end > 0 {
        end -= 1;
    }
    if end < start {
        end = start;
    }
    start..end + 1
}

/// The 1-based source span of a node, with zero columns clamped to one.
#[must_use]
pub fn node_span(node: &comrak::nodes::AstNode<'_>) -> Span {
    let sourcepos = node.data().sourcepos;
    Span::new(
        Position::new(sourcepos.start.line, sourcepos.start.column.max(1)),
        Position::new(sourcepos.end.line, sourcepos.end.column.max(1)),
    )
}

/// Iterates over the descendants of `root` (excluding `root` itself).
pub fn descendants(root: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    root.descendants()
}

/// Concatenates the textual content of a node and its descendants.
///
/// Code spans contribute their literal content, line breaks contribute a
/// single space, and raw HTML is skipped. This mirrors the text used by
/// `comrak` when generating heading anchors.
#[must_use]
pub fn collect_text(node: Node<'_>) -> String {
    let mut out = String::new();
    collect_text_into(node, &mut out);
    out
}

fn collect_text_into(node: Node<'_>, out: &mut String) {
    for child in node.children() {
        match &child.data().value {
            NodeValue::Text(text) => out.push_str(text),
            NodeValue::Code(code) => out.push_str(&code.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            NodeValue::Math(math) => out.push_str(&math.literal),
            NodeValue::HtmlInline(_) | NodeValue::HtmlBlock(_) => {}
            NodeValue::FootnoteReference(_) => {}
            _ => collect_text_into(child, out),
        }
    }
}

/// Whether `node` is a block-level container that can hold other blocks.
#[must_use]
pub fn is_block_container(node: &comrak::nodes::AstNode<'_>) -> bool {
    matches!(
        node.data().value,
        NodeValue::Document
            | NodeValue::BlockQuote
            | NodeValue::List(_)
            | NodeValue::Item(_)
            | NodeValue::FootnoteDefinition(_)
            | NodeValue::DescriptionList
            | NodeValue::DescriptionItem(_)
            | NodeValue::DescriptionDetails
            | NodeValue::TaskItem(_)
            | NodeValue::MultilineBlockQuote(_)
            | NodeValue::Alert(_)
            | NodeValue::BlockDirective(_)
    )
}

/// The span of a byte range in `source`, treating `range` as half-open.
#[must_use]
pub fn source_span(source: &Source, range: Range<usize>) -> Span {
    source.span_of(range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Dialect, with_parsed_options};

    #[test]
    fn node_lines_handles_zero_end_column() {
        let source = Source::new("- item\n\nnext\n".to_string());
        let options = Dialect::Gfm.comrak_options();
        with_parsed_options(&source, &options, |root| {
            let item = root
                .descendants()
                .find(|node| matches!(node.data().value, NodeValue::Item(_)))
                .expect("item");
            assert_eq!(node_lines(item), 0..1);
        });
    }

    #[test]
    fn collect_text_walks_inlines() {
        let source = Source::new("# hello `code` *world*\n".to_string());
        let options = Dialect::Gfm.comrak_options();
        with_parsed_options(&source, &options, |root| {
            let heading = root
                .descendants()
                .find(|node| matches!(node.data().value, NodeValue::Heading(_)))
                .expect("heading");
            assert_eq!(collect_text(heading), "hello code world");
        });
    }
}
