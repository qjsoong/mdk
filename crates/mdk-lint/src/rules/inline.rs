//! Inline rules: MD033, MD034, MD036, MD038, MD039, MD042, MD044, MD045,
//! MD049 and MD050.

use comrak::nodes::{NodeCode, NodeValue};
use mdk_core::{Diagnostic, Fix, Severity};

use crate::context::LintContext;
use crate::rules::{Rule, RuleMeta};

pub(crate) static MD033: RuleMeta = RuleMeta {
    id: "MD033",
    name: "no-inline-html",
    description: "Inline HTML",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md033;

impl Rule for Md033 {
    fn meta(&self) -> &'static RuleMeta {
        &MD033
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let allowed: Vec<String> = ctx
            .options(&MD033)
            .strings("allowed_elements")
            .into_iter()
            .map(|item| item.to_ascii_lowercase())
            .collect();
        for node in ctx.root.descendants() {
            let literal = match &node.data().value {
                NodeValue::HtmlInline(html) => html.clone(),
                NodeValue::HtmlBlock(block) => block.literal.clone(),
                _ => continue,
            };
            let mut reported = false;
            for tag in html_tags(&literal) {
                if allowed.iter().any(|item| item == &tag) {
                    continue;
                }
                if !reported {
                    diagnostics.push(ctx.diagnostic(
                        &MD033,
                        mdk_core::ast::node_span(node),
                        format!("inline HTML element `<{tag}>`"),
                    ));
                    reported = true;
                }
            }
        }
    }
}

/// Extracts lowercase tag names from a raw HTML fragment.
fn html_tags(html: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let bytes = html.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        if html[index..].starts_with("<!--")
            || html[index..].starts_with("<!")
            || html[index..].starts_with("<?")
        {
            index += 2;
            continue;
        }
        index += 1;
        if bytes.get(index) == Some(&b'/') {
            index += 1;
        }
        let start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'-')
        {
            index += 1;
        }
        if index > start {
            tags.push(html[start..index].to_ascii_lowercase());
        }
    }
    tags
}

pub(crate) static MD034: RuleMeta = RuleMeta {
    id: "MD034",
    name: "no-bare-urls",
    description: "Bare URL used",
    default_severity: Severity::Warning,
    fixable: true,
};

pub(crate) struct Md034;

impl Rule for Md034 {
    fn meta(&self) -> &'static RuleMeta {
        &MD034
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            let NodeValue::Text(text) = &node.data().value else {
                continue;
            };
            if node
                .parent()
                .is_some_and(|parent| matches!(parent.data().value, NodeValue::Link(_)))
            {
                continue;
            }
            let span = mdk_core::ast::node_span(node);
            let range = ctx.source.span_range(span);
            let exact = ctx.source.slice(range.clone()) == text.as_ref();
            for (url, url_range) in bare_urls(text) {
                let diagnostic_range = if exact {
                    range.start + url_range.start..range.start + url_range.end
                } else {
                    range.clone()
                };
                let mut diagnostic = ctx.range_diagnostic(
                    &MD034,
                    diagnostic_range.clone(),
                    format!(
                        "bare URL `{url}` should be wrapped in angle brackets or linked"
                    ),
                );
                diagnostic.fix = Some(Fix::new(diagnostic_range, format!("<{url}>")));
                diagnostics.push(diagnostic);
            }
        }
    }
}

/// Finds bare URL candidates in plain text.
fn bare_urls(text: &str) -> Vec<(String, std::ops::Range<usize>)> {
    let mut urls = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        let found = ["http://", "https://", "ftp://", "www."]
            .iter()
            .filter_map(|scheme| rest.find(scheme).map(|offset| (offset, *scheme)))
            .min_by_key(|(offset, _)| *offset);
        let Some((offset, scheme)) = found else {
            break;
        };
        let start = index + offset;
        let preceding_ok = start == 0
            || !text[..start]
                .chars()
                .last()
                .is_some_and(|ch| ch.is_alphanumeric() || ch == '/' || ch == '@');
        let mut end = start + scheme.len();
        for (byte, ch) in text[end..].char_indices() {
            if ch.is_whitespace() || matches!(ch, '<' | '>' | '"' | '\'') {
                break;
            }
            end = start + scheme.len() + byte + ch.len_utf8();
        }
        // Do not include common sentence punctuation.
        let trimmed_end = trim_url_end(&text[start..end]);
        let end = start + trimmed_end;
        if preceding_ok && end > start + scheme.len() {
            urls.push((text[start..end].to_string(), start..end));
        }
        index = end.max(start + 1);
    }
    urls
}

fn trim_url_end(url: &str) -> usize {
    let mut end = url.len();
    while let Some(ch) = url[..end].chars().last() {
        if matches!(ch, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}') {
            end -= ch.len_utf8();
            // Keep balanced parentheses, e.g. Wikipedia URLs.
            if ch == ')'
                && url[..end].matches('(').count() > url[..end].matches(')').count()
            {
                end += ch.len_utf8();
                break;
            }
        } else {
            break;
        }
    }
    end
}

pub(crate) static MD036: RuleMeta = RuleMeta {
    id: "MD036",
    name: "no-emphasis-as-heading",
    description: "Emphasis used instead of a heading",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md036;

impl Rule for Md036 {
    fn meta(&self) -> &'static RuleMeta {
        &MD036
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            if !matches!(node.data().value, NodeValue::Paragraph) {
                continue;
            }
            let mut significant = Vec::new();
            for child in node.children() {
                let data = child.data();
                match &data.value {
                    NodeValue::Text(text) if text.trim().is_empty() => {}
                    value => significant.push(value.clone()),
                }
            }
            if significant.len() == 1
                && matches!(significant[0], NodeValue::Emph | NodeValue::Strong)
            {
                diagnostics.push(ctx.diagnostic(
                    &MD036,
                    mdk_core::ast::node_span(node),
                    "paragraph is entirely emphasized; use a heading instead",
                ));
            }
        }
    }
}

pub(crate) static MD038: RuleMeta = RuleMeta {
    id: "MD038",
    name: "no-space-in-code",
    description: "Spaces inside code span elements",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md038;

impl Rule for Md038 {
    fn meta(&self) -> &'static RuleMeta {
        &MD038
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            let NodeValue::Code(code) = &node.data().value else {
                continue;
            };
            if code.literal.trim().is_empty() {
                continue;
            }
            let span = mdk_core::ast::node_span(node);
            let range = ctx.source.span_range(span);
            let raw = ctx.source.slice(range);
            if code_span_has_spaces(raw, code) {
                diagnostics.push(ctx.diagnostic(
                    &MD038,
                    span,
                    "spaces inside code span should be removed",
                ));
            }
        }
    }
}

fn code_span_has_spaces(raw: &str, code: &NodeCode) -> bool {
    let ticks = "`".repeat(code.num_backticks);
    let Some(inner) = raw
        .strip_prefix(&ticks)
        .and_then(|rest| rest.strip_suffix(&ticks))
    else {
        return false;
    };
    if inner.trim().is_empty() {
        return false;
    }
    inner.starts_with(' ') || inner.ends_with(' ')
}

pub(crate) static MD039: RuleMeta = RuleMeta {
    id: "MD039",
    name: "no-space-in-links",
    description: "Spaces inside link text",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md039;

impl Rule for Md039 {
    fn meta(&self) -> &'static RuleMeta {
        &MD039
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            if !matches!(node.data().value, NodeValue::Link(_) | NodeValue::Image(_)) {
                continue;
            }
            let (Some(first), Some(last)) = (node.first_child(), node.last_child())
            else {
                continue;
            };
            let start = ctx.source.span_range(mdk_core::ast::node_span(first)).start;
            let end = ctx.source.span_range(mdk_core::ast::node_span(last)).end;
            let text = ctx.source.slice(start..end);
            if text.starts_with(' ') || text.ends_with(' ') {
                let span = mdk_core::ast::node_span(node);
                let range = ctx.source.span_range(span);
                let raw = ctx.source.slice(range);
                if raw.starts_with('[') || raw.starts_with("![") {
                    diagnostics.push(ctx.diagnostic(
                        &MD039,
                        span,
                        "spaces inside link text should be removed",
                    ));
                }
            }
        }
    }
}

pub(crate) static MD042: RuleMeta = RuleMeta {
    id: "MD042",
    name: "no-empty-links",
    description: "Links must have a destination or an alternative",
    default_severity: Severity::Error,
    fixable: false,
};

pub(crate) struct Md042;

impl Rule for Md042 {
    fn meta(&self) -> &'static RuleMeta {
        &MD042
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            let NodeValue::Link(link) = &node.data().value else {
                continue;
            };
            if link.url.is_empty() {
                diagnostics.push(ctx.diagnostic(
                    &MD042,
                    mdk_core::ast::node_span(node),
                    "link has an empty destination",
                ));
            }
        }
    }
}

pub(crate) static MD044: RuleMeta = RuleMeta {
    id: "MD044",
    name: "proper-names",
    description: "Proper names should have the correct capitalization",
    default_severity: Severity::Error,
    fixable: true,
};

pub(crate) struct Md044;

impl Rule for Md044 {
    fn meta(&self) -> &'static RuleMeta {
        &MD044
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let options = ctx.options(&MD044);
        let names = options.strings("names");
        if names.is_empty() {
            return;
        }
        let code_blocks = options.bool("code_blocks", true);

        for node in ctx.root.descendants() {
            // Check prose text nodes with exact source mapping and autofix.
            if let NodeValue::Text(text) = &node.data().value {
                check_proper_names(ctx, node, text, &names, diagnostics);
                continue;
            }
            // Optionally check code spans and code blocks, where the literal
            // content is not modeled as text nodes. These yield no autofix
            // because their content is verbatim.
            if !code_blocks {
                continue;
            }
            let data = node.data();
            let literal = match &data.value {
                NodeValue::Code(code) => code.literal.as_str(),
                NodeValue::CodeBlock(code) => code.literal.as_str(),
                _ => continue,
            };
            for name in &names {
                if !find_case_insensitive(literal, name).is_empty() {
                    diagnostics.push(ctx.diagnostic(
                        &MD044,
                        mdk_core::ast::node_span(node),
                        format!(
                            "proper name `{name}` has incorrect capitalization in code"
                        ),
                    ));
                }
            }
        }
    }
}

fn check_proper_names(
    ctx: &LintContext<'_, '_>,
    node: &comrak::nodes::AstNode<'_>,
    text: &str,
    names: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let span = mdk_core::ast::node_span(node);
    let range = ctx.source.span_range(span);
    let exact = ctx.source.slice(range.clone()) == text;
    for name in names {
        for found in find_case_insensitive(text, name) {
            if &text[found.clone()] == name.as_str() {
                continue;
            }
            let diagnostic_range = if exact {
                range.start + found.start..range.start + found.end
            } else {
                range.clone()
            };
            let mut diagnostic = ctx.range_diagnostic(
                &MD044,
                diagnostic_range.clone(),
                format!(
                    "proper name `{name}` should be capitalized as `{name}`, found `{}`",
                    &text[found.clone()]
                ),
            );
            diagnostic.fix = Some(Fix::new(diagnostic_range, name.clone()));
            diagnostics.push(diagnostic);
        }
    }
}

/// Finds case-insensitive, character-wise occurrences of `needle` in
/// `haystack`, returning byte ranges.
fn find_case_insensitive(haystack: &str, needle: &str) -> Vec<std::ops::Range<usize>> {
    if needle.is_empty() {
        return Vec::new();
    }
    let needle_chars: Vec<char> = needle.chars().collect();
    let mut found = Vec::new();
    let mut index = 0;
    while index <= haystack.len() {
        let mut cursor = index;
        let mut matched = true;
        for expected in &needle_chars {
            let Some(ch) = haystack[cursor..].chars().next() else {
                matched = false;
                break;
            };
            if !chars_eq_ignore_case(ch, *expected) {
                matched = false;
                break;
            }
            cursor += ch.len_utf8();
        }
        if matched {
            // Reject matches embedded in longer words.
            let before = haystack[..index].chars().last();
            let after = haystack[cursor..].chars().next();
            let boundary_before = before.is_none_or(|ch| !ch.is_alphanumeric());
            let boundary_after = after.is_none_or(|ch| !ch.is_alphanumeric());
            if boundary_before && boundary_after {
                found.push(index..cursor);
            }
            index = cursor.max(index + 1);
        } else {
            let Some(ch) = haystack[index..].chars().next() else {
                break;
            };
            index += ch.len_utf8();
        }
    }
    found
}

fn chars_eq_ignore_case(a: char, b: char) -> bool {
    a == b
        || a.to_lowercase().eq(b.to_lowercase())
        || a.to_uppercase().eq(b.to_uppercase())
}

pub(crate) static MD045: RuleMeta = RuleMeta {
    id: "MD045",
    name: "no-alt-text",
    description: "Images should have alternate text (alt text)",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md045;

impl Rule for Md045 {
    fn meta(&self) -> &'static RuleMeta {
        &MD045
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        for node in ctx.root.descendants() {
            if !matches!(node.data().value, NodeValue::Image(_)) {
                continue;
            }
            if mdk_core::ast::collect_text(node).trim().is_empty() {
                diagnostics.push(ctx.diagnostic(
                    &MD045,
                    mdk_core::ast::node_span(node),
                    "image is missing alt text",
                ));
            }
        }
    }
}

pub(crate) static MD049: RuleMeta = RuleMeta {
    id: "MD049",
    name: "emphasis-style",
    description: "Emphasis style",
    default_severity: Severity::Warning,
    fixable: true,
};

pub(crate) struct Md049;

impl Rule for Md049 {
    fn meta(&self) -> &'static RuleMeta {
        &MD049
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        delimiter_rule(ctx, &MD049, NodeValue::Emph, 1, diagnostics);
    }
}

pub(crate) static MD050: RuleMeta = RuleMeta {
    id: "MD050",
    name: "strong-style",
    description: "Strong style",
    default_severity: Severity::Warning,
    fixable: true,
};

pub(crate) struct Md050;

impl Rule for Md050 {
    fn meta(&self) -> &'static RuleMeta {
        &MD050
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        delimiter_rule(ctx, &MD050, NodeValue::Strong, 2, diagnostics);
    }
}

fn delimiter_rule(
    ctx: &LintContext<'_, '_>,
    meta: &'static RuleMeta,
    kind: NodeValue,
    count: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let style = ctx.options(meta).string("style", "consistent");
    let mut baseline: Option<char> = None;

    for node in ctx.root.descendants() {
        if !node_value_matches(&node.data().value, &kind) {
            continue;
        }
        let span = mdk_core::ast::node_span(node);
        let range = ctx.source.span_range(span);
        let raw = ctx.source.slice(range.clone());
        let actual = if raw.starts_with('*') {
            '*'
        } else if raw.starts_with('_') {
            '_'
        } else {
            continue;
        };
        let expected = match style.as_str() {
            "asterisk" => Some('*'),
            "underscore" => Some('_'),
            _ => Some(*baseline.get_or_insert(actual)),
        };
        let Some(expected) = expected else { continue };
        if expected == actual {
            continue;
        }
        let mut diagnostic = ctx.diagnostic(
            meta,
            span,
            format!("expected `{expected}` delimiters, found `{actual}`",),
        );
        if let Some(fix) = delimiter_fix(raw, count, actual, expected) {
            diagnostic.fix = Some(Fix::new(range, fix));
        }
        diagnostics.push(diagnostic);
    }
}

fn node_value_matches(value: &NodeValue, expected: &NodeValue) -> bool {
    std::mem::discriminant(value) == std::mem::discriminant(expected)
}

fn delimiter_fix(
    raw: &str,
    count: usize,
    actual: char,
    expected: char,
) -> Option<String> {
    let lead = actual.to_string().repeat(count);
    let tail = lead.clone();
    let inner = raw.strip_prefix(&lead)?.strip_suffix(&tail)?;
    Some(format!(
        "{}{inner}{}",
        expected.to_string().repeat(count),
        expected.to_string().repeat(count)
    ))
}

#[cfg(test)]
mod tests {
    use crate::config::LintConfig;
    use crate::engine::lint_text;
    use crate::testutil::ids;
    use mdk_core::Dialect;

    #[test]
    fn md033_inline_html() {
        assert_eq!(
            ids(&["MD033"], "text <br> more\n", Dialect::Commonmark),
            vec!["MD033"]
        );
    }

    #[test]
    fn md034_bare_urls() {
        assert_eq!(
            ids(
                &["MD034"],
                "see https://example.com now\n",
                Dialect::Commonmark
            ),
            vec!["MD034"]
        );
        assert!(
            ids(
                &["MD034"],
                "see <https://example.com> now\n",
                Dialect::Commonmark
            )
            .is_empty()
        );
    }

    #[test]
    fn md036_emphasis_paragraph() {
        assert_eq!(
            ids(&["MD036"], "**bold only**\n", Dialect::Commonmark),
            vec!["MD036"]
        );
        assert!(
            ids(&["MD036"], "some **bold** text\n", Dialect::Commonmark).is_empty()
        );
    }

    #[test]
    fn md038_spaces_in_code() {
        assert_eq!(
            ids(&["MD038"], "` spaced `\n", Dialect::Commonmark),
            vec!["MD038"]
        );
        assert!(ids(&["MD038"], "`fine`\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md039_spaces_in_links() {
        assert_eq!(
            ids(&["MD039"], "[ text](url)\n", Dialect::Commonmark),
            vec!["MD039"]
        );
        assert!(ids(&["MD039"], "[text](url)\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md042_empty_links() {
        assert_eq!(
            ids(&["MD042"], "[text]()\n", Dialect::Commonmark),
            vec!["MD042"]
        );
    }

    #[test]
    fn md044_proper_names() {
        let config: LintConfig =
            toml::from_str("[rules.MD044]\nnames = [\"JavaScript\"]\n").unwrap();
        let diagnostics =
            lint_text("use javascript please\n", Dialect::Commonmark, &config);
        let diagnostics: Vec<_> = diagnostics
            .iter()
            .filter(|item| item.rule == "MD044")
            .collect();
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn md045_alt_text() {
        assert_eq!(
            ids(&["MD045"], "![](image.png)\n", Dialect::Commonmark),
            vec!["MD045"]
        );
        assert!(ids(&["MD045"], "![alt](image.png)\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md049_emphasis_style() {
        assert_eq!(
            ids(&["MD049"], "*a* and _b_\n", Dialect::Commonmark),
            vec!["MD049"]
        );
        assert!(ids(&["MD049"], "*a* and *b*\n", Dialect::Commonmark).is_empty());
    }

    #[test]
    fn md050_strong_style() {
        assert_eq!(
            ids(&["MD050"], "**a** and __b__\n", Dialect::Commonmark),
            vec!["MD050"]
        );
    }
}
