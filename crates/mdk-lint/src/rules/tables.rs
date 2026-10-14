//! Table and thematic-break rules: MD035, MD056 and MD058.

use comrak::nodes::{NodeTable, NodeValue};
use mdk_core::{Diagnostic, Fix, Severity};

use crate::context::LintContext;
use crate::rules::{Rule, RuleMeta};

use super::{
    blank_lines_after, blank_lines_are_safe, blank_lines_before,
    fix_insert_blank_after, fix_insert_blank_before, node_last_line,
};

pub(crate) static MD035: RuleMeta = RuleMeta {
    id: "MD035",
    name: "hr-style",
    description: "Horizontal rule style",
    default_severity: Severity::Warning,
    fixable: true,
};

pub(crate) struct Md035;

impl Rule for Md035 {
    fn meta(&self) -> &'static RuleMeta {
        &MD035
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let style = ctx.options(&MD035).string("style", "consistent");
        let mut baseline: Option<String> = None;

        for node in ctx.root.descendants() {
            if !matches!(node.data().value, NodeValue::ThematicBreak) {
                continue;
            }
            let line = node.data().sourcepos.start.line.saturating_sub(1);
            let text = ctx.source.line(line).trim_end();
            let actual: String =
                text.chars().filter(|ch| !ch.is_whitespace()).collect();
            if actual.is_empty() {
                continue;
            }
            let expected = match style.as_str() {
                "---" => "---".to_string(),
                "***" => "***".to_string(),
                "___" => "___".to_string(),
                _ => baseline.clone().unwrap_or_else(|| actual.clone()),
            };
            if baseline.is_none() {
                baseline = Some(actual.clone());
            }
            if actual == expected {
                continue;
            }
            let mut diagnostic = ctx.diagnostic(
                &MD035,
                mdk_core::ast::node_span(node),
                format!("expected horizontal rule `{expected}`, found `{actual}`"),
            );
            // Changing `***` to `---` can turn a preceding paragraph into a
            // setext heading, so only fix when no paragraph precedes.
            let safe = node.previous_sibling().is_none_or(|previous| {
                !matches!(previous.data().value, NodeValue::Paragraph)
            });
            if safe {
                let range = ctx.line_range(line);
                let content = ctx.source.slice(range.clone());
                let replacement = content.replace(&actual, &expected);
                diagnostic.fix = Some(Fix::new(range, replacement));
            }
            diagnostics.push(diagnostic);
        }
    }
}

pub(crate) static MD056: RuleMeta = RuleMeta {
    id: "MD056",
    name: "table-column-count",
    description: "Table column count",
    default_severity: Severity::Error,
    fixable: false,
};

pub(crate) struct Md056;

impl Rule for Md056 {
    fn meta(&self) -> &'static RuleMeta {
        &MD056
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            let NodeValue::Table(table) = &node.data().value else {
                continue;
            };
            check_table_columns(ctx, node, table, diagnostics);
        }
    }
}

fn check_table_columns(
    ctx: &LintContext<'_, '_>,
    node: comrak::Node<'_>,
    table: &NodeTable,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let lines = mdk_core::ast::node_lines(node);
    let mut expected: Option<usize> = None;
    for line in lines {
        let Some(cells) = count_table_cells(ctx.source.line(line)) else {
            continue;
        };
        match expected {
            None => expected = Some(cells),
            Some(expected) if cells != expected => {
                diagnostics.push(ctx.diagnostic(
                    &MD056,
                    ctx.line_span(line),
                    format!(
                        "table row has {cells} cells, expected {expected} (table has {} columns)",
                        table.num_columns
                    ),
                ));
            }
            _ => {}
        }
    }
}

/// Counts GFM table cells on a single line, ignoring escaped pipes and pipes
/// inside code spans.
fn count_table_cells(line: &str) -> Option<usize> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let leading = trimmed.starts_with('|');
    let trailing = trimmed.ends_with('|');
    let mut pipes = 0usize;
    let mut backticks = 0usize;
    let mut escaped = false;
    for ch in trimmed.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '`' => backticks += 1,
            '|' if backticks.is_multiple_of(2) => pipes += 1,
            _ => {}
        }
    }
    Some((pipes + 1).saturating_sub(usize::from(leading) + usize::from(trailing)))
}

pub(crate) static MD058: RuleMeta = RuleMeta {
    id: "MD058",
    name: "blanks-around-tables",
    description: "Tables should be surrounded by blank lines",
    default_severity: Severity::Error,
    fixable: true,
};

pub(crate) struct Md058;

impl Rule for Md058 {
    fn meta(&self) -> &'static RuleMeta {
        &MD058
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            if !matches!(node.data().value, NodeValue::Table(_)) {
                continue;
            }
            let first = node.data().sourcepos.start.line.saturating_sub(1);
            let last = node_last_line(node);
            let span = mdk_core::ast::node_span(node);

            if node.previous_sibling().is_some() && blank_lines_before(ctx, first) == 0
            {
                let mut diagnostic = ctx.diagnostic(
                    &MD058,
                    span,
                    "table should be preceded by a blank line",
                );
                if blank_lines_are_safe(node) {
                    diagnostic.fix = Some(fix_insert_blank_before(ctx.source, first));
                }
                diagnostics.push(diagnostic);
            }
            if node.next_sibling().is_some() && blank_lines_after(ctx, last) == 0 {
                let mut diagnostic = ctx.diagnostic(
                    &MD058,
                    span,
                    "table should be followed by a blank line",
                );
                if blank_lines_are_safe(node) {
                    diagnostic.fix = Some(fix_insert_blank_after(ctx.source, last));
                }
                diagnostics.push(diagnostic);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::ids;
    use mdk_core::Dialect;

    #[test]
    fn md035_consistent_hr() {
        assert_eq!(
            ids(&["MD035"], "---\n\n***\n", Dialect::Commonmark),
            vec!["MD035"]
        );
    }

    #[test]
    fn md056_column_count() {
        let markdown = "| a | b |\n|---|---|\n| 1 |\n";
        let diagnostics =
            crate::testutil::lint_with(&["MD056"], markdown, Dialect::Gfm);
        assert!(
            !diagnostics.is_empty(),
            "expected MD056 to fire for {markdown:?}"
        );
    }

    #[test]
    fn md058_blanks_around_tables() {
        let markdown = "# H\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        let diagnostics =
            crate::testutil::lint_with(&["MD058"], markdown, Dialect::Gfm);
        assert!(
            !diagnostics.is_empty(),
            "expected MD058 to fire for {markdown:?}"
        );
    }
}
