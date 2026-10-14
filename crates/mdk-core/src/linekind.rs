//! Classification of physical lines for rules and formatting decisions.

use comrak::nodes::NodeValue;

use crate::Source;
use crate::ast::node_lines;
use crate::text::is_blank;

/// How a physical line of a document is classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// A line inside a YAML or TOML front matter block.
    FrontMatter,
    /// A line inside an indented or fenced code block.
    Code,
    /// A line inside an HTML block.
    Html,
    /// A line belonging to a GFM table.
    Table,
    /// A blank line outside code and HTML blocks.
    Blank,
    /// Any other line.
    Normal,
}

impl LineKind {
    /// Whether the line is verbatim content that formatters should not touch.
    #[must_use]
    pub const fn is_verbatim(self) -> bool {
        matches!(self, Self::FrontMatter | Self::Code | Self::Html)
    }
}

/// Classifies every line of `source` using the parsed AST.
///
/// Lines start out as [`LineKind::Normal`] or [`LineKind::Blank`]; block
/// nodes then mark their lines as code, HTML, table or front matter. Front
/// matter takes precedence over every other classification.
#[must_use]
pub fn classify(source: &Source, root: comrak::Node<'_>) -> Vec<LineKind> {
    let mut kinds = vec![LineKind::Normal; source.line_count()];
    for (index, line) in source.lines().enumerate() {
        if is_blank(line) {
            kinds[index] = LineKind::Blank;
        }
    }
    for node in root.descendants() {
        let kind = match node.data().value {
            NodeValue::CodeBlock(_) => LineKind::Code,
            NodeValue::HtmlBlock(_) => LineKind::Html,
            NodeValue::Table(_) => LineKind::Table,
            _ => continue,
        };
        for line in node_lines(node) {
            if let Some(slot) = kinds.get_mut(line) {
                *slot = kind;
            }
        }
    }
    if let Some(range) = source.front_matter_range() {
        for (index, _) in source.lines().enumerate() {
            if source.line_range(index).start < range.end {
                kinds[index] = LineKind::FrontMatter;
            }
        }
    }
    kinds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dialect;

    #[test]
    fn classifies_blocks() {
        let source = Source::new(
            "---\ntitle: x\n---\n\n# h\n\n```rust\nfn main() {}\n```\n\n| a |\n| - |\n| 1 |\n"
                .to_string(),
        );
        let options = Dialect::Gfm.comrak_options();
        crate::with_parsed_options(&source, &options, |root| {
            let kinds = classify(&source, root);
            assert_eq!(kinds[0], LineKind::FrontMatter);
            assert_eq!(kinds[3], LineKind::Blank);
            assert_eq!(kinds[6], LineKind::Code);
            assert_eq!(kinds[7], LineKind::Code);
            assert_eq!(kinds[10], LineKind::Table);
        });
    }
}
