//! Code block rules: MD031, MD040, MD046 and MD048.

use comrak::nodes::{NodeCodeBlock, NodeValue};
use mdk_core::{Diagnostic, Fix, Severity};

use crate::context::LintContext;
use crate::rules::{Rule, RuleMeta};

use super::{
    blank_lines_after, blank_lines_are_safe, blank_lines_before,
    fix_insert_blank_after, fix_insert_blank_before, node_last_line,
};

pub(crate) static MD031: RuleMeta = RuleMeta {
    id: "MD031",
    name: "blanks-around-fences",
    description: "Fenced code blocks should be surrounded by blank lines",
    default_severity: Severity::Error,
    fixable: true,
};

pub(crate) struct Md031;

impl Rule for Md031 {
    fn meta(&self) -> &'static RuleMeta {
        &MD031
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let list_items = ctx.options(&MD031).bool("list_items", true);

        for node in ctx.root.descendants() {
            let NodeValue::CodeBlock(code) = &node.data().value else {
                continue;
            };
            if !code.fenced {
                continue;
            }
            if !list_items && node.ancestors().any(ancestor_is_list) {
                continue;
            }
            let first = node.data().sourcepos.start.line.saturating_sub(1);
            let last = node_last_line(node);
            let span = mdk_core::ast::node_span(node);

            if node.previous_sibling().is_some() && blank_lines_before(ctx, first) == 0
            {
                let mut diagnostic = ctx.diagnostic(
                    &MD031,
                    span,
                    "fenced code block should be preceded by a blank line",
                );
                if blank_lines_are_safe(node) {
                    diagnostic.fix = Some(fix_insert_blank_before(ctx.source, first));
                }
                diagnostics.push(diagnostic);
            }
            if node.next_sibling().is_some() && blank_lines_after(ctx, last) == 0 {
                let mut diagnostic = ctx.diagnostic(
                    &MD031,
                    span,
                    "fenced code block should be followed by a blank line",
                );
                if blank_lines_are_safe(node) {
                    diagnostic.fix = Some(fix_insert_blank_after(ctx.source, last));
                }
                diagnostics.push(diagnostic);
            }
        }
    }
}

fn ancestor_is_list(node: &comrak::nodes::AstNode<'_>) -> bool {
    matches!(node.data().value, NodeValue::List(_))
}

pub(crate) static MD040: RuleMeta = RuleMeta {
    id: "MD040",
    name: "fenced-code-language",
    description: "Fenced code blocks should have a language specified",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md040;

impl Rule for Md040 {
    fn meta(&self) -> &'static RuleMeta {
        &MD040
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let allowed = ctx.options(&MD040).strings("allowed_languages");
        for node in ctx.root.descendants() {
            let NodeValue::CodeBlock(code) = &node.data().value else {
                continue;
            };
            if !code.fenced {
                continue;
            }
            let language = code
                .info
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if language.is_empty() {
                diagnostics.push(ctx.diagnostic(
                    &MD040,
                    mdk_core::ast::node_span(node),
                    "fenced code block has no language",
                ));
                continue;
            }
            if !allowed.is_empty()
                && !allowed
                    .iter()
                    .any(|item| item.eq_ignore_ascii_case(&language))
            {
                diagnostics.push(ctx.diagnostic(
                    &MD040,
                    mdk_core::ast::node_span(node),
                    format!("language `{language}` is not in allowed_languages"),
                ));
            }
        }
    }
}

pub(crate) static MD046: RuleMeta = RuleMeta {
    id: "MD046",
    name: "code-block-style",
    description: "Code block style",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md046;

impl Rule for Md046 {
    fn meta(&self) -> &'static RuleMeta {
        &MD046
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let style = ctx.options(&MD046).string("style", "fenced");
        let mut baseline: Option<bool> = None;
        for node in ctx.root.descendants() {
            let NodeValue::CodeBlock(code) = &node.data().value else {
                continue;
            };
            let actual = code.fenced;
            let expected = match style.as_str() {
                "indented" => Some(false),
                "fenced" => Some(true),
                _ => Some(*baseline.get_or_insert(actual)),
            };
            if let Some(expected) = expected
                && expected != actual
            {
                diagnostics.push(ctx.diagnostic(
                    &MD046,
                    mdk_core::ast::node_span(node),
                    format!(
                        "expected {} code block",
                        if expected { "fenced" } else { "indented" }
                    ),
                ));
            }
        }
    }
}

pub(crate) static MD048: RuleMeta = RuleMeta {
    id: "MD048",
    name: "code-fence-style",
    description: "Code fence style",
    default_severity: Severity::Warning,
    fixable: true,
};

pub(crate) struct Md048;

impl Rule for Md048 {
    fn meta(&self) -> &'static RuleMeta {
        &MD048
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let style = ctx.options(&MD048).string("style", "consistent");
        let mut baseline: Option<u8> = None;

        for node in ctx.root.descendants() {
            let NodeValue::CodeBlock(code) = &node.data().value else {
                continue;
            };
            if !code.fenced {
                continue;
            }
            let actual = code.fence_char;
            let expected = match style.as_str() {
                "backtick" => Some(b'`'),
                "tilde" => Some(b'~'),
                _ => Some(*baseline.get_or_insert(actual)),
            };
            let Some(expected) = expected else { continue };
            if expected == actual {
                continue;
            }
            let mut diagnostic = ctx.diagnostic(
                &MD048,
                mdk_core::ast::node_span(node),
                format!(
                    "expected `{}` code fence, found `{}`",
                    expected as char, actual as char
                ),
            );
            if let Some(fixes) = fence_fixes(ctx, node, code, actual, expected) {
                diagnostic.fix = Some(fixes);
            }
            diagnostics.push(diagnostic);
        }
    }
}

/// Builds a fix replacing the opening and (if present) closing fence runs.
fn fence_fixes(
    ctx: &LintContext<'_, '_>,
    node: &comrak::nodes::AstNode<'_>,
    code: &NodeCodeBlock,
    from: u8,
    to: u8,
) -> Option<Fix> {
    let first = node.data().sourcepos.start.line.saturating_sub(1);
    let last = node_last_line(node);
    if first != last {
        // Replace only the opening fence when the closing fence shares the
        // same range problem; a single combined fix keeps ranges disjoint.
        let opening_range = ctx.line_range(first);
        let opening =
            replace_fence_run(ctx.source.slice(opening_range.clone()), from, to);
        let mut replacement = opening?;
        if code.closed {
            let closing_range = ctx.line_range(last);
            let closing = ctx.source.slice(closing_range.clone());
            let fixed_closing = replace_fence_run(closing, from, to)?;
            replacement.push('\n');
            replacement.push_str(&fixed_closing);
            return Some(Fix::new(
                opening_range.start..closing_range.end,
                replacement,
            ));
        }
        return Some(Fix::new(opening_range, replacement));
    }
    let range = ctx.line_range(first);
    Some(Fix::new(
        range.clone(),
        replace_fence_run(ctx.source.slice(range), from, to)?,
    ))
}

fn replace_fence_run(line: &str, from: u8, to: u8) -> Option<String> {
    let from = from as char;
    let to_char = to as char;
    let start = line.find(from)?;
    let run = line[start..].chars().take_while(|ch| *ch == from).count();
    let mut out = String::with_capacity(line.len());
    out.push_str(&line[..start]);
    out.extend(std::iter::repeat_n(to_char, run));
    out.push_str(&line[start + run..]);
    Some(out)
}
