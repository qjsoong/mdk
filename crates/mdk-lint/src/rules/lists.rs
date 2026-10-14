//! List rules: MD004, MD005, MD007, MD029, MD030 and MD032.

use comrak::nodes::NodeValue;
use mdk_core::{Diagnostic, Severity, Span};

use crate::context::LintContext;
use crate::rules::{Rule, RuleMeta};

use super::{
    blank_lines_after, blank_lines_are_safe, blank_lines_before,
    fix_insert_blank_after, fix_insert_blank_before,
};

/// Parsed list marker information.
struct MarkerInfo {
    number: Option<usize>,
    marker_len: usize,
    spaces: usize,
}

fn parse_marker(text: &str) -> Option<MarkerInfo> {
    let bytes = text.as_bytes();
    let first = *bytes.first()?;
    let (number, marker_len) = match first {
        b'-' | b'+' | b'*' => (None, 1),
        b'0'..=b'9' => {
            let digits = bytes
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            if digits > 9 {
                return None;
            }
            let delimiter = *bytes.get(digits)?;
            if delimiter != b'.' && delimiter != b')' {
                return None;
            }
            let number = text[..digits].parse().ok()?;
            (Some(number), digits + 1)
        }
        _ => return None,
    };
    let rest = &text[marker_len..];
    let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    Some(MarkerInfo {
        number,
        marker_len,
        spaces,
    })
}

fn item_marker(
    ctx: &LintContext<'_, '_>,
    node: &comrak::nodes::AstNode<'_>,
) -> Option<MarkerInfo> {
    let sourcepos = node.data().sourcepos;
    let offset = ctx.source.offset_of(mdk_core::Position::new(
        sourcepos.start.line,
        sourcepos.start.column,
    ))?;
    let rest = &ctx.source.text()[offset..];
    let line_end = rest.find('\n').unwrap_or(rest.len());
    parse_marker(&rest[..line_end])
}

fn list_items<'a>(
    list: &'a comrak::nodes::AstNode<'a>,
) -> impl Iterator<Item = &'a comrak::nodes::AstNode<'a>> {
    list.children().filter(|child| {
        matches!(
            child.data().value,
            NodeValue::Item(_) | NodeValue::TaskItem(_)
        )
    })
}

fn element_span(node: &comrak::nodes::AstNode<'_>) -> Span {
    mdk_core::ast::node_span(node)
}

pub(crate) static MD004: RuleMeta = RuleMeta {
    id: "MD004",
    name: "ul-style",
    description: "Unordered list style",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md004;

const BULLET_CYCLE: [char; 3] = ['-', '*', '+'];

impl Rule for Md004 {
    fn meta(&self) -> &'static RuleMeta {
        &MD004
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let style = ctx.options(&MD004).string("style", "consistent");
        let mut baseline: Option<char> = None;

        for node in ctx.root.descendants() {
            let NodeValue::List(list) = &node.data().value else {
                continue;
            };
            if list.list_type != comrak::nodes::ListType::Bullet {
                continue;
            }
            let depth = node
                .ancestors()
                .filter(|ancestor| matches!(ancestor.data().value, NodeValue::List(_)))
                .count();
            let actual = char::from(list.bullet_char);
            let expected = match style.as_str() {
                "dash" => Some('-'),
                "asterisk" => Some('*'),
                "plus" => Some('+'),
                "sublist" => {
                    let baseline = *baseline.get_or_insert(actual);
                    let index = BULLET_CYCLE
                        .iter()
                        .position(|item| *item == baseline)
                        .unwrap_or(0);
                    Some(BULLET_CYCLE[(index + depth) % BULLET_CYCLE.len()])
                }
                _ => {
                    let baseline = *baseline.get_or_insert(actual);
                    Some(baseline)
                }
            };
            if let Some(expected) = expected
                && actual != expected
            {
                diagnostics.push(ctx.diagnostic(
                    &MD004,
                    element_span(node),
                    format!("expected `{expected}` bullet, found `{actual}`"),
                ));
            }
        }
    }
}

pub(crate) static MD005: RuleMeta = RuleMeta {
    id: "MD005",
    name: "list-indent",
    description: "Inconsistent indentation for list items at the same level",
    default_severity: Severity::Error,
    fixable: false,
};

pub(crate) struct Md005;

impl Rule for Md005 {
    fn meta(&self) -> &'static RuleMeta {
        &MD005
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            if !matches!(node.data().value, NodeValue::List(_)) {
                continue;
            }
            let mut expected: Option<usize> = None;
            for item in list_items(node) {
                let column = item.data().sourcepos.start.column;
                match expected {
                    None => expected = Some(column),
                    Some(expected) if column != expected => {
                        diagnostics.push(ctx.diagnostic(
                            &MD005,
                            element_span(item),
                            format!("expected list item at column {expected}, found column {column}"),
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
}

pub(crate) static MD007: RuleMeta = RuleMeta {
    id: "MD007",
    name: "ul-indent",
    description: "Unordered list indentation",
    default_severity: Severity::Error,
    fixable: false,
};

pub(crate) struct Md007;

impl Rule for Md007 {
    fn meta(&self) -> &'static RuleMeta {
        &MD007
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let options = ctx.options(&MD007);
        let indent = options.usize("indent", 2);
        let start_indented = options.bool("start_indented", false);

        for node in ctx.root.descendants() {
            let NodeValue::List(list) = &node.data().value else {
                continue;
            };
            if list.list_type != comrak::nodes::ListType::Bullet {
                continue;
            }
            let column = node.data().sourcepos.start.column;
            match node.parent() {
                Some(parent)
                    if matches!(
                        parent.data().value,
                        NodeValue::Item(_) | NodeValue::TaskItem(_)
                    ) =>
                {
                    let parent_column = parent.data().sourcepos.start.column;
                    let parent_padding = match &parent.data().value {
                        NodeValue::Item(item) => item.padding,
                        // Task items reuse the containing list's padding.
                        _ => {
                            match parent.parent().map(|list| list.data().value.clone())
                            {
                                Some(NodeValue::List(list)) => list.padding,
                                _ => continue,
                            }
                        }
                    };
                    let content_column = parent_column + parent_padding;
                    let expected =
                        content_column + indent.saturating_sub(parent_padding);
                    if column != expected {
                        diagnostics.push(ctx.diagnostic(
                            &MD007,
                            element_span(node),
                            format!("expected nested list at position {expected}, found {column}"),
                        ));
                    }
                }
                _ => {
                    if !start_indented && column != 1 {
                        diagnostics.push(ctx.diagnostic(
                            &MD007,
                            element_span(node),
                            format!("expected top-level list at column 1, found column {column}"),
                        ));
                    }
                }
            }
        }
    }
}

pub(crate) static MD029: RuleMeta = RuleMeta {
    id: "MD029",
    name: "ol-prefix",
    description: "Ordered list item prefix",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md029;

impl Rule for Md029 {
    fn meta(&self) -> &'static RuleMeta {
        &MD029
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let style = ctx.options(&MD029).string("style", "one_or_ordered");
        for node in ctx.root.descendants() {
            let NodeValue::List(list) = &node.data().value else {
                continue;
            };
            if list.list_type != comrak::nodes::ListType::Ordered {
                continue;
            }
            let numbers: Vec<Option<usize>> = list_items(node)
                .map(|item| item_marker(ctx, item).and_then(|marker| marker.number))
                .collect();
            let first = numbers.iter().flatten().next().copied().unwrap_or(1);
            let sequential = |index: usize, number: usize| number == first + index;
            let all_one = |number: usize| number == 1;

            for (index, number) in numbers.iter().enumerate() {
                let Some(number) = number else { continue };
                let ok = match style.as_str() {
                    "one" => all_one(*number),
                    "ordered" => sequential(index, *number),
                    "zero" => *number == 0,
                    _ => all_one(*number) || sequential(index, *number),
                };
                if !ok {
                    diagnostics.push(ctx.diagnostic(
                        &MD029,
                        element_span(node),
                        format!("expected ordered list prefix `{}`", first + index),
                    ));
                }
            }
        }
    }
}

pub(crate) static MD030: RuleMeta = RuleMeta {
    id: "MD030",
    name: "list-marker-space",
    description: "Spaces after list markers",
    default_severity: Severity::Warning,
    fixable: true,
};

pub(crate) struct Md030;

impl Rule for Md030 {
    fn meta(&self) -> &'static RuleMeta {
        &MD030
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let options = ctx.options(&MD030);
        for node in ctx.root.descendants() {
            let NodeValue::List(list) = &node.data().value else {
                continue;
            };
            for item in list_items(node) {
                let Some(marker) = item_marker(ctx, item) else {
                    continue;
                };
                let multi = is_multi_line(item);
                let expected = match (list.list_type, multi) {
                    (comrak::nodes::ListType::Bullet, false) => {
                        options.usize("ul_single", 1)
                    }
                    (comrak::nodes::ListType::Bullet, true) => {
                        options.usize("ul_multi", 1)
                    }
                    (comrak::nodes::ListType::Ordered, false) => {
                        options.usize("ol_single", 1)
                    }
                    (comrak::nodes::ListType::Ordered, true) => {
                        options.usize("ol_multi", 1)
                    }
                };
                if marker.spaces == expected {
                    continue;
                }
                let sourcepos = item.data().sourcepos;
                let start = ctx
                    .source
                    .offset_of(mdk_core::Position::new(
                        sourcepos.start.line,
                        sourcepos.start.column,
                    ))
                    .unwrap_or(0);
                let range = start + marker.marker_len
                    ..start + marker.marker_len + marker.spaces;
                let mut diagnostic = ctx.range_diagnostic(
                    &MD030,
                    range.clone(),
                    format!(
                        "expected {expected} space(s) after list marker, found {}",
                        marker.spaces
                    ),
                );
                diagnostic.fix = Some(mdk_core::Fix::new(range, " ".repeat(expected)));
                diagnostics.push(diagnostic);
            }
        }
    }
}

fn is_multi_line(item: comrak::Node<'_>) -> bool {
    let lines = mdk_core::ast::node_lines(item);
    if lines.len() > 1 {
        return true;
    }
    item.children().count() > 1
}

pub(crate) static MD032: RuleMeta = RuleMeta {
    id: "MD032",
    name: "blanks-around-lists",
    description: "Lists should be surrounded by blank lines",
    default_severity: Severity::Error,
    fixable: true,
};

pub(crate) struct Md032;

impl Rule for Md032 {
    fn meta(&self) -> &'static RuleMeta {
        &MD032
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            if !is_top_level_list(node) {
                continue;
            }
            let first = node.data().sourcepos.start.line.saturating_sub(1);
            let last = super::node_last_line(node);
            let span = element_span(node);

            if node.previous_sibling().is_some() && blank_lines_before(ctx, first) == 0
            {
                let mut diagnostic = ctx.diagnostic(
                    &MD032,
                    span,
                    "list should be preceded by a blank line",
                );
                if blank_lines_are_safe(node) {
                    diagnostic.fix = Some(fix_insert_blank_before(ctx.source, first));
                }
                diagnostics.push(diagnostic);
            }
            if node.next_sibling().is_some() && blank_lines_after(ctx, last) == 0 {
                let mut diagnostic = ctx.diagnostic(
                    &MD032,
                    span,
                    "list should be followed by a blank line",
                );
                if blank_lines_are_safe(node) {
                    diagnostic.fix = Some(fix_insert_blank_after(ctx.source, last));
                }
                diagnostics.push(diagnostic);
            }
        }
    }
}

fn is_top_level_list(node: &comrak::nodes::AstNode<'_>) -> bool {
    if !matches!(node.data().value, NodeValue::List(_)) {
        return false;
    }
    node.parent().is_none_or(|parent| {
        matches!(
            parent.data().value,
            NodeValue::Document
                | NodeValue::BlockQuote
                | NodeValue::FootnoteDefinition(_)
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::testutil::ids;
    use mdk_core::Dialect;

    #[test]
    fn md004_consistent() {
        assert_eq!(
            ids(&["MD004"], "- a\n\n* b\n", Dialect::Commonmark),
            vec!["MD004"]
        );
        assert!(ids(&["MD004"], "- a\n- b\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md005_inconsistent_indent() {
        let inconsistent = "- a\n  - b\n   - c\n";
        let diagnostics =
            crate::testutil::lint_with(&["MD005"], inconsistent, Dialect::Commonmark);
        assert!(
            !diagnostics.is_empty(),
            "expected MD005 to fire for {inconsistent:?}"
        );
    }

    #[test]
    fn md007_nested_indent() {
        assert!(ids(&["MD007"], "- a\n  - b\n", Dialect::Commonmark).is_empty());
        assert_eq!(
            ids(&["MD007"], "- a\n    - b\n", Dialect::Commonmark),
            vec!["MD007"]
        );
    }

    #[test]
    fn md029_ordered_prefix() {
        assert_eq!(
            ids(&["MD029"], "1. a\n3. b\n", Dialect::Commonmark),
            vec!["MD029"]
        );
        assert!(ids(&["MD029"], "1. a\n2. b\n", Dialect::Commonmark).is_empty());
        assert!(ids(&["MD029"], "1. a\n1. b\n", Dialect::Commonmark).is_empty());
        assert!(ids(&["MD029"], "2. a\n3. b\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md030_marker_space() {
        assert_eq!(
            ids(&["MD030"], "-   a\n", Dialect::Commonmark),
            vec!["MD030"]
        );
        assert!(ids(&["MD030"], "- a\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md032_blanks_around_lists() {
        assert!(ids(&["MD032"], "text\n\n- a\n", Dialect::Commonmark).is_empty());
        assert_eq!(
            ids(&["MD032"], "text\n- a\n", Dialect::Commonmark),
            vec!["MD032"]
        );
    }
}
