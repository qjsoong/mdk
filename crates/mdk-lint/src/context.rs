//! Per-document lint context and shared facts.

use std::collections::BTreeSet;
use std::ops::Range;

use comrak::Anchorizer;
use comrak::nodes::NodeValue;
use mdk_core::ast::{collect_text, node_span};
use mdk_core::text::is_blank;
use mdk_core::{Diagnostic, Dialect, Position, Source, Span};

use crate::config::{LintConfig, RuleOptions};
use crate::rules::RuleMeta;

/// How a physical line is classified for rule decisions.
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

/// A heading found in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingInfo {
    /// Heading level, 1 through 6.
    pub level: u8,
    /// Whether the heading uses the setext style.
    pub setext: bool,
    /// The heading's textual content.
    pub text: String,
    /// The heading's source location (both lines for setext headings).
    pub span: Span,
    /// 0-based line index where the heading starts.
    pub line: usize,
    /// The GitHub-style anchor slug for the heading.
    pub slug: String,
}

/// A link or image reference definition (`[label]: destination`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceDefinition {
    /// The label as written.
    pub label: String,
    /// The label normalized for matching (trimmed, collapsed, case-folded).
    pub normalized: String,
    /// Location of the label, including the brackets.
    pub span: Span,
    /// 0-based line index of the definition.
    pub line: usize,
}

/// A full or collapsed reference usage (`[text][label]` or `[label][]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceUsage {
    /// The raw label as written.
    pub label: String,
    /// The label normalized for matching.
    pub normalized: String,
    /// Location of the label text (without brackets).
    pub span: Span,
}

/// A footnote reference or definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootnoteInfo {
    /// The footnote name without the `^` prefix.
    pub name: String,
    /// Source location of the reference or definition.
    pub span: Span,
}

/// Facts computed once per document and shared by all rules.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// All headings, in document order.
    pub headings: Vec<HeadingInfo>,
    /// All link and image reference definitions.
    pub definitions: Vec<ReferenceDefinition>,
    /// All full and collapsed reference usages found in text nodes.
    pub usages: Vec<ReferenceUsage>,
    /// Reference usages found by scanning source lines; this includes
    /// references that resolved to links, which never appear as text.
    pub line_usages: Vec<ReferenceUsage>,
    /// All footnote definitions.
    pub footnote_definitions: Vec<FootnoteInfo>,
    /// All footnote references.
    pub footnote_references: Vec<FootnoteInfo>,
    /// All heading anchor slugs, including uniqueness suffixes.
    pub anchors: BTreeSet<String>,
}

impl Facts {
    /// The set of normalized defined reference labels.
    #[must_use]
    pub fn defined_labels(&self) -> BTreeSet<&str> {
        self.definitions
            .iter()
            .map(|definition| definition.normalized.as_str())
            .collect()
    }
}

/// Everything a rule needs to inspect one document.
pub struct LintContext<'a, 's> {
    /// The source document.
    pub source: &'s Source,
    /// The parsed AST root.
    pub root: comrak::Node<'a>,
    /// The dialect the document was parsed with.
    pub dialect: Dialect,
    /// Effective lint configuration.
    pub config: &'s LintConfig,
    line_kinds: Vec<LineKind>,
    facts: Facts,
}

impl<'a, 's> LintContext<'a, 's> {
    /// Builds a context, computing shared facts.
    #[must_use]
    pub fn new(
        source: &'s Source,
        root: comrak::Node<'a>,
        dialect: Dialect,
        config: &'s LintConfig,
    ) -> Self {
        let line_kinds = compute_line_kinds(source, root);
        let mut facts = Facts::default();
        compute_headings(root, &mut facts);
        compute_definitions(source, &line_kinds, &mut facts);
        compute_usages(source, root, &mut facts);
        compute_line_usages(source, &line_kinds, &mut facts);
        compute_footnotes(root, &mut facts);
        Self {
            source,
            root,
            dialect,
            config,
            line_kinds,
            facts,
        }
    }

    /// Shared document facts.
    #[must_use]
    pub const fn facts(&self) -> &Facts {
        &self.facts
    }

    /// The classification of a 0-based line.
    #[must_use]
    pub fn line_kind(&self, line: usize) -> LineKind {
        self.line_kinds
            .get(line)
            .copied()
            .unwrap_or(LineKind::Normal)
    }

    /// The number of classified lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_kinds.len()
    }

    /// Whether a 0-based line is inside code, HTML or front matter.
    #[must_use]
    pub fn is_verbatim_line(&self, line: usize) -> bool {
        matches!(
            self.line_kind(line),
            LineKind::Code | LineKind::Html | LineKind::FrontMatter
        )
    }

    /// Typed options for a rule.
    #[must_use]
    pub fn options(&self, meta: &RuleMeta) -> RuleOptions<'_> {
        self.config.options_for(meta)
    }

    /// Creates a diagnostic with the configured severity for `meta`.
    #[must_use]
    pub fn diagnostic(
        &self,
        meta: &RuleMeta,
        span: Span,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic::new(meta.id, self.config.severity_for(meta), message, span)
    }

    /// Creates a diagnostic covering a byte range of the source.
    #[must_use]
    pub fn range_diagnostic(
        &self,
        meta: &RuleMeta,
        range: Range<usize>,
        message: impl Into<String>,
    ) -> Diagnostic {
        self.diagnostic(meta, self.source.span_of(range), message)
    }

    /// A span covering the whole 0-based line.
    #[must_use]
    pub fn line_span(&self, line: usize) -> Span {
        let range = self.source.line_range(line);
        self.source.span_of(range)
    }

    /// The byte range of a 0-based line.
    #[must_use]
    pub fn line_range(&self, line: usize) -> Range<usize> {
        self.source.line_range(line)
    }

    /// A point position at the start of a 0-based line.
    #[must_use]
    pub fn line_start(&self, line: usize) -> Position {
        Position::new(line + 1, 1)
    }
}

fn compute_line_kinds(source: &Source, root: comrak::Node<'_>) -> Vec<LineKind> {
    let mut kinds = vec![LineKind::Normal; source.line_count()];
    for (index, line) in source.lines().enumerate() {
        if is_blank(line) {
            kinds[index] = LineKind::Blank;
        }
    }
    if let Some(range) = source.front_matter_range() {
        for (index, line) in source.lines().enumerate() {
            if source.line_range(index).start < range.end
                || !line.is_empty() && index == 0
            {
                kinds[index] = LineKind::FrontMatter;
            }
        }
    }
    for node in root.descendants() {
        let kind = match node.data().value {
            NodeValue::CodeBlock(_) => LineKind::Code,
            NodeValue::HtmlBlock(_) => LineKind::Html,
            NodeValue::Table(_) => LineKind::Table,
            _ => continue,
        };
        for line in mdk_core::ast::node_lines(node) {
            if let Some(slot) = kinds.get_mut(line) {
                *slot = kind;
            }
        }
    }
    kinds
}

fn compute_headings(root: comrak::Node<'_>, facts: &mut Facts) {
    let mut anchorizer = Anchorizer::new();
    for node in root.descendants() {
        let NodeValue::Heading(heading) = &node.data().value else {
            continue;
        };
        let text = collect_text(node);
        let slug = anchorizer.anchorize(&text);
        facts.anchors.insert(slug.clone());
        facts.headings.push(HeadingInfo {
            level: heading.level,
            setext: heading.setext,
            text,
            span: node_span(node),
            line: node.data().sourcepos.start.line.saturating_sub(1),
            slug,
        });
    }
}

fn compute_definitions(source: &Source, kinds: &[LineKind], facts: &mut Facts) {
    for (index, line) in source.lines().enumerate() {
        if !matches!(kinds.get(index), Some(LineKind::Normal | LineKind::Blank)) {
            continue;
        }
        let Some((label, label_range)) = parse_reference_definition(line) else {
            continue;
        };
        let line_start = source.line_range(index).start;
        facts.definitions.push(ReferenceDefinition {
            normalized: normalize_label(label),
            label: label.to_string(),
            span: source
                .span_of(line_start + label_range.start..line_start + label_range.end),
            line: index,
        });
    }
}

fn compute_usages(source: &Source, root: comrak::Node<'_>, facts: &mut Facts) {
    for node in root.descendants() {
        let NodeValue::Text(text) = &node.data().value else {
            continue;
        };
        let span = node_span(node);
        let range = source.span_range(span);
        let exact = source.slice(range.clone()) == text.as_ref();
        for (label, label_range) in scan_reference_usages(text) {
            let usage_span = if exact {
                source.span_of(
                    range.start + label_range.start..range.start + label_range.end,
                )
            } else {
                span
            };
            facts.usages.push(ReferenceUsage {
                normalized: normalize_label(&label),
                label,
                span: usage_span,
            });
        }
    }
}

fn compute_line_usages(source: &Source, kinds: &[LineKind], facts: &mut Facts) {
    for (index, kind) in kinds.iter().enumerate() {
        if !matches!(kind, LineKind::Normal) {
            continue;
        }
        let line = source.line(index);
        let line_start = source.line_range(index).start;
        for (label, range) in scan_reference_usages(line) {
            facts.line_usages.push(ReferenceUsage {
                normalized: normalize_label(&label),
                label,
                span: source.span_of(line_start + range.start..line_start + range.end),
            });
        }
    }
}

fn compute_footnotes(root: comrak::Node<'_>, facts: &mut Facts) {
    for node in root.descendants() {
        match &node.data().value {
            NodeValue::FootnoteDefinition(definition) => {
                facts.footnote_definitions.push(FootnoteInfo {
                    name: definition.name.clone(),
                    span: node_span(node),
                });
            }
            NodeValue::FootnoteReference(reference) => {
                facts.footnote_references.push(FootnoteInfo {
                    name: reference.name.clone(),
                    span: node_span(node),
                });
            }
            _ => {}
        }
    }
}

/// Parses a link reference definition line.
///
/// Returns the raw label and the byte range of the label text (without
/// brackets) within `line`.
pub(crate) fn parse_reference_definition(line: &str) -> Option<(&str, Range<usize>)> {
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let bytes = rest.as_bytes();
    if bytes.first() != Some(&b'[') {
        return None;
    }
    let mut index = 1;
    let close = loop {
        match bytes.get(index)? {
            b'\\' => index += 2,
            b'[' => return None,
            b']' => break index,
            _ => index += 1,
        }
    };
    if bytes.get(close + 1) != Some(&b':') {
        return None;
    }
    let label = &rest[1..close];
    if label.is_empty() || label.chars().count() > 999 {
        return None;
    }
    let after = rest[close + 2..].trim_start_matches([' ', '\t']);
    if after.is_empty() || after.starts_with('\n') {
        return None;
    }
    Some((label, indent + 1..indent + close))
}

/// Scans text for full (`[text][label]`) and collapsed (`[label][]`)
/// reference usages.
///
/// Shortcut references (`[label]`) are intentionally ignored: they are
/// indistinguishable from ordinary bracketed prose, so reporting them would
/// produce false positives. Returns the raw label and its byte range within
/// `text`.
pub(crate) fn scan_reference_usages(text: &str) -> Vec<(String, Range<usize>)> {
    let bytes = text.as_bytes();
    let mut usages = Vec::new();
    let mut cursor = 0;
    while let Some(found) = text[cursor..].find("][") {
        let boundary = cursor + found;
        let mut index = boundary + 2;
        let mut close = None;
        while index < bytes.len() {
            match bytes[index] {
                b'\\' => index += 2,
                b'[' => break,
                b']' => {
                    close = Some(index);
                    break;
                }
                _ => index += 1,
            }
        }
        let Some(close) = close else {
            cursor = boundary + 2;
            continue;
        };
        let label_range = boundary + 2..close;
        if label_range.is_empty() {
            if let Some(open) = find_open_bracket(text, boundary) {
                let collapsed = open + 1..boundary;
                if !collapsed.is_empty() {
                    usages.push((text[collapsed.clone()].to_string(), collapsed));
                }
            }
        } else {
            usages.push((text[label_range.clone()].to_string(), label_range));
        }
        cursor = close + 1;
        if cursor >= text.len() {
            break;
        }
    }
    usages
}

fn find_open_bracket(text: &str, before: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = before;
    let mut escaped = false;
    while index > 0 {
        index -= 1;
        match bytes[index] {
            b']' if !escaped => return None,
            b'\\' if !escaped => {
                escaped = true;
                continue;
            }
            b'[' if !escaped => return Some(index),
            _ => {}
        }
        escaped = false;
    }
    None
}

/// Normalizes a reference label per CommonMark: trim, collapse internal
/// whitespace and case-fold.
#[must_use]
pub fn normalize_label(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut pending_space = false;
    for ch in label.trim().chars() {
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;
        out.extend(ch.to_lowercase());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_labels() {
        assert_eq!(normalize_label("  Foo   Bar "), "foo bar");
        assert_eq!(normalize_label("CASE"), "case");
    }

    #[test]
    fn parses_reference_definitions() {
        let (label, range) =
            parse_reference_definition("[foo]: https://example.com").unwrap();
        assert_eq!(label, "foo");
        assert_eq!(&"[foo]: https://example.com"[range], "foo");
        assert!(parse_reference_definition("  [foo]: https://example.com").is_some());
        assert!(parse_reference_definition("    [foo]: https://example.com").is_none());
        assert!(parse_reference_definition("[foo] https://example.com").is_none());
        assert!(parse_reference_definition("[foo]:").is_none());
        assert!(parse_reference_definition("text [foo]: x").is_none());
    }

    #[test]
    fn scans_full_and_collapsed_usages() {
        let usages =
            scan_reference_usages("see [text][label] and [other][] plus [shortcut]");
        let labels: Vec<&str> =
            usages.iter().map(|(label, _)| label.as_str()).collect();
        assert_eq!(labels, vec!["label", "other"]);
    }

    #[test]
    fn ignores_unclosed_brackets() {
        assert!(scan_reference_usages("a][b and [c][d").is_empty());
    }

    #[test]
    fn computes_headings_and_facts() {
        let source =
            Source::new("# One\n\n## Two\n\n[ref]: /x\n\nSee [a][ref].\n".to_string());
        let config = LintConfig::default();
        let options = Dialect::Gfm.comrak_options();
        mdk_core::with_parsed_options(&source, &options, |root| {
            let context = LintContext::new(&source, root, Dialect::Gfm, &config);
            assert_eq!(context.facts().headings.len(), 2);
            assert_eq!(context.facts().definitions.len(), 1);
            assert!(context.facts().usages.is_empty());
            assert_eq!(context.facts().line_usages.len(), 1);
            assert!(context.facts().anchors.contains("one"));
            assert_eq!(context.line_kind(3), LineKind::Blank);
        });
    }
}
