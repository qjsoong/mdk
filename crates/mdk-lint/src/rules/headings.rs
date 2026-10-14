//! Heading rules: MD001, MD003, MD018, MD019, MD022, MD023, MD024, MD025,
//! MD026 and MD041.

use comrak::nodes::NodeValue;
use mdk_core::{Diagnostic, Severity, Span};

use crate::context::LintContext;
use crate::rules::{Rule, RuleMeta};
use mdk_core::LineKind;

use super::{
    blank_lines_after, blank_lines_are_safe, blank_lines_before, first_content_span,
    fix_insert_blank_after, fix_insert_blank_before,
};

pub(crate) static MD001: RuleMeta = RuleMeta {
    id: "MD001",
    name: "heading-increment",
    description: "Heading levels should only increment by one level at a time",
    default_severity: Severity::Error,
    fixable: false,
};

pub(crate) struct Md001;

impl Rule for Md001 {
    fn meta(&self) -> &'static RuleMeta {
        &MD001
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let mut previous: Option<u8> = None;
        for heading in &ctx.facts().headings {
            if let Some(previous) = previous
                && heading.level > previous + 1
            {
                diagnostics.push(ctx.diagnostic(
                    &MD001,
                    heading.span,
                    format!(
                        "heading level {} follows level {previous}; increment by one level at a time",
                        heading.level
                    ),
                ));
            }
            previous = Some(heading.level);
        }
    }
}

pub(crate) static MD003: RuleMeta = RuleMeta {
    id: "MD003",
    name: "heading-style",
    description: "Headings should use a consistent style",
    default_severity: Severity::Warning,
    fixable: false,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeadingStyle {
    Atx,
    AtxClosed,
    Setext,
}

impl HeadingStyle {
    fn name(self) -> &'static str {
        match self {
            Self::Atx => "atx",
            Self::AtxClosed => "atx_closed",
            Self::Setext => "setext",
        }
    }
}

pub(crate) struct Md003;

impl Rule for Md003 {
    fn meta(&self) -> &'static RuleMeta {
        &MD003
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let style = ctx.options(&MD003).string("style", "consistent");
        let mut baseline: Option<HeadingStyle> = None;

        for node in ctx.root.descendants() {
            let NodeValue::Heading(heading) = &node.data().value else {
                continue;
            };
            let actual = if heading.setext {
                HeadingStyle::Setext
            } else if heading.closed {
                HeadingStyle::AtxClosed
            } else {
                HeadingStyle::Atx
            };

            let expected = match style.as_str() {
                "atx" => Some(HeadingStyle::Atx),
                "atx_closed" => Some(HeadingStyle::AtxClosed),
                "setext" => Some(HeadingStyle::Setext),
                _ => {
                    let baseline = *baseline.get_or_insert(actual);
                    Some(baseline)
                }
            };

            if let Some(expected) = expected
                && actual != expected
            {
                diagnostics.push(ctx.diagnostic(
                    &MD003,
                    mdk_core::ast::node_span(node),
                    format!(
                        "expected {} heading style, found {}",
                        expected.name(),
                        actual.name()
                    ),
                ));
            }
        }
    }
}

/// Parsed ATX prefix information for a source line.
struct AtxPrefix {
    indent: usize,
    hashes: usize,
    spaces_after: usize,
}

fn parse_atx_prefix(line: &str) -> Option<AtxPrefix> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let hashes = rest.bytes().take_while(|byte| *byte == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let after = &rest[hashes..];
    if after.is_empty() {
        return Some(AtxPrefix {
            indent,
            hashes,
            spaces_after: 0,
        });
    }
    if !after.starts_with([' ', '\t']) {
        return Some(AtxPrefix {
            indent,
            hashes,
            spaces_after: 0,
        });
    }
    let spaces_after = after.len() - after.trim_start_matches([' ', '\t']).len();
    Some(AtxPrefix {
        indent,
        hashes,
        spaces_after,
    })
}

pub(crate) static MD018: RuleMeta = RuleMeta {
    id: "MD018",
    name: "no-missing-space-atx",
    description: "No space after hash on atx style heading",
    default_severity: Severity::Error,
    fixable: true,
};

pub(crate) struct Md018;

impl Rule for Md018 {
    fn meta(&self) -> &'static RuleMeta {
        &MD018
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for line in 0..ctx.line_count() {
            if ctx.line_kind(line) != LineKind::Normal {
                continue;
            }
            let text = ctx.source.line(line);
            let Some(prefix) = parse_atx_prefix(text) else {
                continue;
            };
            if prefix.spaces_after != 0 {
                continue;
            }
            let after = &text[prefix.indent + prefix.hashes..];
            if after.is_empty() {
                continue;
            }
            if after.starts_with('#') {
                continue;
            }
            let offset = ctx.line_range(line).start + prefix.indent + prefix.hashes;
            let mut diagnostic =
                ctx.range_diagnostic(&MD018, offset..offset, "missing space after `#`");
            diagnostic.fix = Some(mdk_core::Fix::new(offset..offset, " "));
            diagnostics.push(diagnostic);
        }
    }
}

pub(crate) static MD019: RuleMeta = RuleMeta {
    id: "MD019",
    name: "no-multiple-space-atx",
    description: "Multiple spaces after hash on atx style heading",
    default_severity: Severity::Error,
    fixable: true,
};

pub(crate) struct Md019;

impl Rule for Md019 {
    fn meta(&self) -> &'static RuleMeta {
        &MD019
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            let NodeValue::Heading(heading) = &node.data().value else {
                continue;
            };
            if heading.setext {
                continue;
            }
            let line = node.data().sourcepos.start.line.saturating_sub(1);
            if ctx.line_kind(line) != LineKind::Normal {
                continue;
            }
            let text = ctx.source.line(line);
            let Some(prefix) = parse_atx_prefix(text) else {
                continue;
            };
            if prefix.spaces_after <= 1 {
                continue;
            }
            let start = ctx.line_range(line).start + prefix.indent + prefix.hashes;
            let end = start + prefix.spaces_after;
            let mut diagnostic = ctx.range_diagnostic(
                &MD019,
                start..end,
                format!(
                    "expected one space after `#`, found {}",
                    prefix.spaces_after
                ),
            );
            diagnostic.fix = Some(mdk_core::Fix::new(start..end, " "));
            diagnostics.push(diagnostic);
        }
    }
}

pub(crate) static MD022: RuleMeta = RuleMeta {
    id: "MD022",
    name: "blanks-around-headings",
    description: "Headings should be surrounded by blank lines",
    default_severity: Severity::Error,
    fixable: true,
};

pub(crate) struct Md022;

impl Rule for Md022 {
    fn meta(&self) -> &'static RuleMeta {
        &MD022
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let options = ctx.options(&MD022);
        let above = options.usize("lines_above", 1);
        let below = options.usize("lines_below", 1);
        let safe_to_fix =
            |node: &comrak::nodes::AstNode<'_>| blank_lines_are_safe(node);

        for node in ctx.root.descendants() {
            if !matches!(node.data().value, NodeValue::Heading(_)) {
                continue;
            }
            let first = node.data().sourcepos.start.line.saturating_sub(1);
            let last = super::node_last_line(node);
            let span = mdk_core::ast::node_span(node);

            if node.previous_sibling().is_some() {
                let found = blank_lines_before(ctx, first);
                if found < above {
                    let mut diagnostic = ctx.diagnostic(
                        &MD022,
                        span,
                        format!("heading should be preceded by {above} blank line(s)"),
                    );
                    if safe_to_fix(node) {
                        diagnostic.fix =
                            Some(fix_insert_blank_before(ctx.source, first));
                    }
                    diagnostics.push(diagnostic);
                }
            }

            if node.next_sibling().is_some() {
                let found = blank_lines_after(ctx, last);
                if found < below {
                    let mut diagnostic = ctx.diagnostic(
                        &MD022,
                        span,
                        format!("heading should be followed by {below} blank line(s)"),
                    );
                    if safe_to_fix(node) {
                        diagnostic.fix = Some(fix_insert_blank_after(ctx.source, last));
                    }
                    diagnostics.push(diagnostic);
                }
            }
        }
    }
}

pub(crate) static MD023: RuleMeta = RuleMeta {
    id: "MD023",
    name: "heading-start-left",
    description: "Headings must start at the beginning of the line",
    default_severity: Severity::Error,
    fixable: true,
};

pub(crate) struct Md023;

impl Rule for Md023 {
    fn meta(&self) -> &'static RuleMeta {
        &MD023
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            let NodeValue::Heading(heading) = &node.data().value else {
                continue;
            };
            let sourcepos = node.data().sourcepos;
            if sourcepos.start.column <= 1 {
                continue;
            }
            let line = sourcepos.start.line.saturating_sub(1);
            if ctx.line_kind(line) != LineKind::Normal {
                continue;
            }
            let indent_end = ctx.line_range(line).start + sourcepos.start.column - 1;
            let mut diagnostic = ctx.diagnostic(
                &MD023,
                mdk_core::ast::node_span(node),
                "heading should start at the beginning of the line",
            );
            if !heading.setext {
                let start = ctx.line_range(line).start;
                diagnostic.fix = Some(mdk_core::Fix::new(start..indent_end, ""));
            }
            diagnostics.push(diagnostic);
        }
    }
}

pub(crate) static MD024: RuleMeta = RuleMeta {
    id: "MD024",
    name: "no-duplicate-heading",
    description: "Headings should be unique",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md024;

impl Rule for Md024 {
    fn meta(&self) -> &'static RuleMeta {
        &MD024
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let siblings_only = ctx.options(&MD024).bool("siblings_only", false);
        // (parent heading index, normalized text, span)
        let mut seen: Vec<(Option<usize>, String, Span)> = Vec::new();
        let mut stack: Vec<(u8, usize)> = Vec::new();

        for node in ctx.root.descendants() {
            let NodeValue::Heading(heading) = &node.data().value else {
                continue;
            };
            while stack
                .last()
                .is_some_and(|(level, _)| *level >= heading.level)
            {
                stack.pop();
            }
            let parent = stack.last().map(|(_, index)| *index);
            let text = normalize_heading_text(&mdk_core::ast::collect_text(node));
            let span = mdk_core::ast::node_span(node);

            let duplicate = seen.iter().find(|(seen_parent, seen_text, _)| {
                seen_text == &text && (!siblings_only || *seen_parent == parent)
            });
            if let Some((_, _, _)) = duplicate {
                diagnostics.push(ctx.diagnostic(
                    &MD024,
                    span,
                    format!("duplicate heading `{}`", text.trim()),
                ));
            } else {
                seen.push((parent, text, span));
            }
            stack.push((heading.level, seen.len() - 1));
        }
    }
}

fn normalize_heading_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(crate) static MD025: RuleMeta = RuleMeta {
    id: "MD025",
    name: "single-title",
    description: "Multiple top-level headings in the same document",
    default_severity: Severity::Error,
    fixable: false,
};

pub(crate) struct Md025;

impl Rule for Md025 {
    fn meta(&self) -> &'static RuleMeta {
        &MD025
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let options = ctx.options(&MD025);
        let level = options.usize("level", 1).clamp(1, 6) as u8;
        let front_matter_title = options.bool("front_matter_title", true);

        let mut found: Option<Span> = None;
        if front_matter_title && front_matter_has_title(ctx) {
            // The front matter title counts as the document's title.
            for heading in ctx
                .facts()
                .headings
                .iter()
                .filter(|item| item.level == level)
            {
                diagnostics.push(ctx.diagnostic(
                    &MD025,
                    heading.span,
                    "document already has a title in front matter",
                ));
            }
            return;
        }

        for heading in ctx
            .facts()
            .headings
            .iter()
            .filter(|item| item.level == level)
        {
            if found.is_some() {
                diagnostics.push(ctx.diagnostic(
                    &MD025,
                    heading.span,
                    format!("multiple level-{level} headings in the same document"),
                ));
            } else {
                found = Some(heading.span);
            }
        }
    }
}

fn front_matter_has_title(ctx: &LintContext<'_, '_>) -> bool {
    ctx.source.front_matter_value("title").is_some()
}

pub(crate) static MD026: RuleMeta = RuleMeta {
    id: "MD026",
    name: "no-trailing-punctuation",
    description: "Trailing punctuation in heading",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md026;

impl Rule for Md026 {
    fn meta(&self) -> &'static RuleMeta {
        &MD026
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let punctuation = ctx.options(&MD026).string("punctuation", ".,;:!?");
        for heading in &ctx.facts().headings {
            let trimmed = heading.text.trim_end();
            let Some(last) = trimmed.chars().last() else {
                continue;
            };
            if punctuation.contains(last) {
                diagnostics.push(ctx.diagnostic(
                    &MD026,
                    heading.span,
                    format!("heading ends with punctuation `{last}`"),
                ));
            }
        }
    }
}

pub(crate) static MD041: RuleMeta = RuleMeta {
    id: "MD041",
    name: "first-line-heading",
    description: "First line in a file should be a top-level heading",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md041;

impl Rule for Md041 {
    fn meta(&self) -> &'static RuleMeta {
        &MD041
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let options = ctx.options(&MD041);
        let level = options.usize("level", 1).clamp(1, 6) as u8;
        if options.bool("front_matter_title", true) && front_matter_has_title(ctx) {
            return;
        }
        let first = ctx
            .root
            .children()
            .find(|node| !matches!(node.data().value, NodeValue::FrontMatter(_)));
        let Some(first) = first else {
            return;
        };
        match &first.data().value {
            NodeValue::Heading(heading) if heading.level == level => {}
            _ => {
                let line = first.data().sourcepos.start.line.saturating_sub(1);
                diagnostics.push(ctx.diagnostic(
                    &MD041,
                    first_content_span(ctx, line),
                    format!("first line should be a level-{level} heading"),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::LintConfig;
    use crate::engine::lint_text;
    use crate::testutil::ids;
    use mdk_core::Dialect;

    #[test]
    fn md001_reports_skipped_levels() {
        assert_eq!(
            ids(&["MD001"], "# a\n\n### b\n", Dialect::Commonmark),
            vec!["MD001"]
        );
        assert!(
            ids(&["MD001"], "# a\n\n## b\n\n### c\n", Dialect::Commonmark).is_empty()
        );
    }

    #[test]
    fn md003_consistent_style() {
        assert_eq!(
            ids(&["MD003"], "# a\n\nb\n===\n", Dialect::Commonmark),
            vec!["MD003"]
        );
        assert!(ids(&["MD003"], "# a\n\n## b\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md018_and_md019_detect_spacing() {
        assert_eq!(ids(&["MD018"], "#a\n", Dialect::Commonmark), vec!["MD018"]);
        assert_eq!(
            ids(&["MD019"], "#  spaced\n", Dialect::Commonmark),
            vec!["MD019"]
        );
        assert!(ids(&["MD018", "MD019"], "# fine\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md022_requires_blanks() {
        assert_eq!(
            ids(&["MD022"], "text\n# heading\ntext\n", Dialect::Commonmark),
            vec!["MD022", "MD022"]
        );
    }

    #[test]
    fn md023_indented_heading() {
        assert_eq!(
            ids(&["MD023"], "  # heading\n", Dialect::Commonmark),
            vec!["MD023"]
        );
    }

    #[test]
    fn md024_duplicates() {
        assert_eq!(
            ids(&["MD024"], "# A\n\n## A\n", Dialect::Commonmark),
            vec!["MD024"]
        );
        let config: LintConfig =
            toml::from_str("[rules.MD024]\nsiblings_only = true\n").unwrap();
        let diagnostics = lint_text("# A\n\n## A\n", Dialect::Commonmark, &config);
        assert!(diagnostics.iter().all(|item| item.rule != "MD024"));
    }

    #[test]
    fn md025_single_h1() {
        assert_eq!(
            ids(&["MD025"], "# a\n\n# b\n", Dialect::Commonmark),
            vec!["MD025"]
        );
    }

    #[test]
    fn md026_punctuation() {
        assert_eq!(
            ids(&["MD026"], "# heading.\n", Dialect::Commonmark),
            vec!["MD026"]
        );
        assert!(ids(&["MD026"], "# heading\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md041_first_line() {
        assert_eq!(
            ids(&["MD041"], "text\n\n# later\n", Dialect::Commonmark),
            vec!["MD041"]
        );
        assert!(ids(&["MD041"], "# title\n", Dialect::Commonmark).is_empty());
        let config = LintConfig {
            enable: vec!["MD041".to_string()],
            ..LintConfig::default()
        };
        let diagnostics =
            lint_text("---\ntitle: x\n---\n\ntext\n", Dialect::Gfm, &config);
        assert!(diagnostics.is_empty());

        // TOML front matter, with and without spaces around the separator.
        let diagnostics =
            lint_text("+++\ntitle=\"x\"\n+++\n\ntext\n", Dialect::Gfm, &config);
        assert!(diagnostics.is_empty());

        // An indented key is not a document title.
        let diagnostics = lint_text(
            "---\nauthor:\n  title: nested\n---\n\ntext\n",
            Dialect::Gfm,
            &config,
        );
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].rule, "MD041");
    }
}
