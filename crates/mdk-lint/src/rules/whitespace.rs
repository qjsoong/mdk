//! Whitespace rules: MD009, MD010, MD012, MD013, MD027, MD028 and MD047.

use comrak::nodes::NodeValue;
use mdk_core::text::{display_width, is_blank, trim_end};
use mdk_core::{Diagnostic, Fix, Severity};

use crate::context::LintContext;
use crate::rules::{Rule, RuleMeta};
use mdk_core::LineKind;

use super::blank_lines_before;

pub(crate) static MD009: RuleMeta = RuleMeta {
    id: "MD009",
    name: "no-trailing-spaces",
    description: "Trailing spaces",
    default_severity: Severity::Warning,
    fixable: true,
};

pub(crate) struct Md009;

impl Rule for Md009 {
    fn meta(&self) -> &'static RuleMeta {
        &MD009
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let br_spaces = ctx.options(&MD009).usize("br_spaces", 2);
        for line in 0..ctx.line_count() {
            if matches!(
                ctx.line_kind(line),
                LineKind::Code | LineKind::Html | LineKind::FrontMatter
            ) {
                continue;
            }
            let text = ctx.source.line(line);
            let content = trim_end(text);
            let trailing = text.len() - content.len();
            if trailing == 0 {
                continue;
            }
            let is_break =
                !content.is_empty() && trailing == br_spaces && br_spaces > 0;
            if is_break {
                continue;
            }
            let range = ctx.line_range(line);
            let start = range.start + content.len();
            let end = range.start + text.len();
            let mut diagnostic = ctx.range_diagnostic(
                &MD009,
                start..end,
                format!("trailing space(s): {trailing}"),
            );
            let replacement =
                if br_spaces > 0 && trailing > br_spaces && !content.is_empty() {
                    " ".repeat(br_spaces)
                } else {
                    String::new()
                };
            diagnostic.fix = Some(Fix::new(start..end, replacement));
            diagnostics.push(diagnostic);
        }
    }
}

pub(crate) static MD010: RuleMeta = RuleMeta {
    id: "MD010",
    name: "no-hard-tabs",
    description: "Hard tabs",
    default_severity: Severity::Warning,
    fixable: true,
};

pub(crate) struct Md010;

impl Rule for Md010 {
    fn meta(&self) -> &'static RuleMeta {
        &MD010
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let options = ctx.options(&MD010);
        let code_blocks = options.bool("code_blocks", true);
        let spaces_per_tab = options.usize("spaces_per_tab", 4);
        for line in 0..ctx.line_count() {
            let kind = ctx.line_kind(line);
            if kind == LineKind::FrontMatter
                || (kind == LineKind::Code && !code_blocks)
                || (kind == LineKind::Html && !code_blocks)
            {
                continue;
            }
            let text = ctx.source.line(line);
            if !text.contains('\t') {
                continue;
            }
            let range = ctx.line_range(line);
            let tabs = text.matches('\t').count();
            let mut diagnostic = ctx.range_diagnostic(
                &MD010,
                range.clone(),
                format!("hard tab(s): {tabs}"),
            );
            let fixed = text.replace('\t', &" ".repeat(spaces_per_tab));
            diagnostic.fix = Some(Fix::new(range.clone(), fixed));
            diagnostics.push(diagnostic);
        }
    }
}

pub(crate) static MD012: RuleMeta = RuleMeta {
    id: "MD012",
    name: "no-multiple-blanks",
    description: "Multiple consecutive blank lines",
    default_severity: Severity::Warning,
    fixable: true,
};

pub(crate) struct Md012;

impl Rule for Md012 {
    fn meta(&self) -> &'static RuleMeta {
        &MD012
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let maximum = ctx.options(&MD012).usize("maximum", 1).max(1);
        let mut consecutive = 0usize;

        for line in 0..ctx.line_count() {
            let kind = ctx.line_kind(line);
            if kind == LineKind::FrontMatter {
                continue;
            }
            let blank = matches!(kind, LineKind::Blank)
                || (kind == LineKind::Normal && is_blank(ctx.source.line(line)))
                || (kind == LineKind::Table && is_blank(ctx.source.line(line)));
            if blank && !matches!(kind, LineKind::Code | LineKind::Html) {
                consecutive += 1;
                if consecutive > maximum {
                    let info = ctx.source.line_info(line);
                    let (start, end) = match info {
                        Some(info) if info.newline_len() > 0 => {
                            (info.start(), info.full_end())
                        }
                        Some(info) => (info.start(), info.end()),
                        None => continue,
                    };
                    let mut diagnostic = ctx.range_diagnostic(
                        &MD012,
                        start..end,
                        format!("expected at most {maximum} consecutive blank line(s)"),
                    );
                    diagnostic.fix = Some(Fix::new(start..end, ""));
                    diagnostics.push(diagnostic);
                }
            } else {
                consecutive = 0;
            }
        }
    }
}

pub(crate) static MD013: RuleMeta = RuleMeta {
    id: "MD013",
    name: "line-length",
    description: "Line length",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md013;

impl Rule for Md013 {
    fn meta(&self) -> &'static RuleMeta {
        &MD013
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let options = ctx.options(&MD013);
        let line_length = options.usize("line_length", 80);
        let heading_line_length = options.usize("heading_line_length", line_length);
        let code_blocks = options.bool("code_blocks", true);
        let tables = options.bool("tables", true);
        let headings = options.bool("headings", true);
        let strict = options.bool("strict", false);

        let heading_lines: Vec<usize> = ctx
            .facts()
            .headings
            .iter()
            .flat_map(|heading| {
                let start = heading.span.start.line.saturating_sub(1);
                let end = heading.span.end.line.saturating_sub(1);
                start..=end
            })
            .collect();

        for line in 0..ctx.line_count() {
            let kind = ctx.line_kind(line);
            if matches!(kind, LineKind::FrontMatter) {
                continue;
            }
            if !code_blocks && kind == LineKind::Code {
                continue;
            }
            let is_heading = heading_lines.contains(&line);
            if !headings && is_heading {
                continue;
            }
            if !tables && kind == LineKind::Table {
                continue;
            }
            let text = ctx.source.line(line);
            let width = display_width(text);
            let limit = if is_heading {
                heading_line_length
            } else {
                line_length
            };
            if limit == 0 || width <= limit {
                continue;
            }
            if !strict && !text.contains([' ', '\t']) {
                continue;
            }
            let range = ctx.line_range(line);
            let overflow = text
                .char_indices()
                .nth(limit)
                .map_or(range.start + text.len(), |(index, _)| range.start + index);
            diagnostics.push(ctx.diagnostic(
                &MD013,
                mdk_core::Span::point(ctx.source.position_of(overflow)),
                format!("line length {width}, exceeds limit of {limit}"),
            ));
        }
    }
}

pub(crate) static MD027: RuleMeta = RuleMeta {
    id: "MD027",
    name: "no-multiple-space-blockquote",
    description: "Multiple spaces after blockquote symbol",
    default_severity: Severity::Error,
    fixable: true,
};

pub(crate) struct Md027;

impl Rule for Md027 {
    fn meta(&self) -> &'static RuleMeta {
        &MD027
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for line in 0..ctx.line_count() {
            if !matches!(ctx.line_kind(line), LineKind::Normal | LineKind::Blank) {
                continue;
            }
            let text = ctx.source.line(line);
            let prefix_len = blockquote_prefix_len(text);
            if prefix_len == 0 {
                continue;
            }
            let prefix = &text[..prefix_len];
            let fixed = collapse_blockquote_spaces(prefix);
            if fixed == prefix {
                continue;
            }
            let range = ctx.line_range(line);
            let mut diagnostic = ctx.range_diagnostic(
                &MD027,
                range.start..range.start + prefix_len,
                "multiple spaces after blockquote symbol",
            );
            diagnostic.fix =
                Some(Fix::new(range.start..range.start + prefix_len, fixed));
            diagnostics.push(diagnostic);
        }
    }
}

/// Returns the byte length of the leading container prefix made of up to
/// three spaces and any number of `>` markers with optional following spaces.
fn blockquote_prefix_len(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut leading_spaces = 0;
    while leading_spaces < 3 && bytes.get(index) == Some(&b' ') {
        index += 1;
        leading_spaces += 1;
    }
    let mut saw_marker = false;
    loop {
        if bytes.get(index) != Some(&b'>') {
            break;
        }
        saw_marker = true;
        index += 1;
        while bytes.get(index) == Some(&b' ') {
            index += 1;
        }
    }
    if saw_marker { index } else { 0 }
}

fn collapse_blockquote_spaces(prefix: &str) -> String {
    let mut out = String::with_capacity(prefix.len());
    let mut chars = prefix.chars().peekable();
    while let Some(ch) = chars.next() {
        out.push(ch);
        if ch == '>' {
            let mut spaces = 0;
            while chars.peek() == Some(&' ') {
                chars.next();
                spaces += 1;
            }
            if spaces > 0 {
                out.push(' ');
            }
        }
    }
    out
}

pub(crate) static MD028: RuleMeta = RuleMeta {
    id: "MD028",
    name: "no-blanks-blockquote",
    description: "Blank line inside blockquote",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md028;

impl Rule for Md028 {
    fn meta(&self) -> &'static RuleMeta {
        &MD028
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            if !matches!(node.data().value, NodeValue::BlockQuote) {
                continue;
            }
            let Some(previous) = node.previous_sibling() else {
                continue;
            };
            if !matches!(previous.data().value, NodeValue::BlockQuote) {
                continue;
            }
            let first = node.data().sourcepos.start.line.saturating_sub(1);
            let blanks = blank_lines_before(ctx, first);
            if blanks == 0 {
                continue;
            }
            let blank_line = first - 1;
            diagnostics.push(ctx.diagnostic(
                &MD028,
                ctx.line_span(blank_line),
                "blank line inside blockquote",
            ));
        }
    }
}

pub(crate) static MD047: RuleMeta = RuleMeta {
    id: "MD047",
    name: "file-end-newline",
    description: "Files should end with a single newline character",
    default_severity: Severity::Error,
    fixable: true,
};

pub(crate) struct Md047;

impl Rule for Md047 {
    fn meta(&self) -> &'static RuleMeta {
        &MD047
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let text = ctx.source.text();
        if text.is_empty() || text.ends_with('\n') || text.ends_with('\r') {
            return;
        }
        let end = text.len();
        let mut diagnostic = ctx.diagnostic(
            &MD047,
            mdk_core::Span::point(ctx.source.position_of(end.saturating_sub(1))),
            "file should end with a newline",
        );
        diagnostic.fix = Some(Fix::new(end..end, "\n"));
        diagnostics.push(diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use crate::config::LintConfig;
    use crate::testutil::ids;
    use mdk_core::Dialect;

    #[test]
    fn md009_trailing_spaces() {
        assert_eq!(
            ids(&["MD009"], "text  \nmore \n", Dialect::Commonmark),
            vec!["MD009"]
        );
        // Two trailing spaces are a hard break.
        assert!(ids(&["MD009"], "text  \nmore\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md010_hard_tabs() {
        assert_eq!(
            ids(&["MD010"], "a\tb\n", Dialect::Commonmark),
            vec!["MD010"]
        );
    }

    #[test]
    fn md012_multiple_blanks() {
        assert_eq!(
            ids(&["MD012"], "a\n\n\nb\n", Dialect::Commonmark),
            vec!["MD012"]
        );
        assert!(ids(&["MD012"], "a\n\nb\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md013_line_length() {
        let long = format!("{}\n", "word ".repeat(30));
        assert_eq!(ids(&["MD013"], &long, Dialect::Commonmark), vec!["MD013"]);
        assert!(ids(&["MD013"], "short line\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md027_blockquote_spacing() {
        assert_eq!(
            ids(&["MD027"], ">  quoted\n", Dialect::Commonmark),
            vec!["MD027"]
        );
        assert!(ids(&["MD027"], "> quoted\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md047_final_newline() {
        assert_eq!(ids(&["MD047"], "text", Dialect::Commonmark), vec!["MD047"]);
        assert!(ids(&["MD047"], "text\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn fixes_are_applied() {
        use crate::fix::apply_fixes;
        let config = LintConfig {
            enable: vec![
                "MD009".to_string(),
                "MD012".to_string(),
                "MD047".to_string(),
            ],
            ..LintConfig::default()
        };
        let outcome = apply_fixes("a \n\n\nb", Dialect::Commonmark, &config);
        assert!(outcome.changed());
        assert_eq!(outcome.text, "a\n\nb\n");
        assert!(outcome.diagnostics.is_empty());
    }

    #[test]
    fn is_loose_blank_handles_markers() {
        assert!(crate::rules::is_loose_blank("> >"));
        assert!(crate::rules::is_loose_blank("  >"));
        assert!(!crate::rules::is_loose_blank("> x"));
    }
}
