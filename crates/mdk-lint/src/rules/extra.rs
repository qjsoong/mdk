//! Rules that go beyond the markdownlint rule set.

use comrak::nodes::NodeValue;
use mdk_core::{Diagnostic, Severity};

use crate::context::{LintContext, normalize_label};
use crate::rules::{Rule, RuleMeta};

pub(crate) static MDK001: RuleMeta = RuleMeta {
    id: "MDK001",
    name: "footnote-reference-defined",
    description: "Footnote references should have a matching definition",
    default_severity: Severity::Error,
    fixable: false,
};

pub(crate) struct Mdk001;

impl Rule for Mdk001 {
    fn meta(&self) -> &'static RuleMeta {
        &MDK001
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        if ctx.dialect != mdk_core::Dialect::Gfm {
            return;
        }
        let defined: Vec<String> = ctx
            .facts()
            .footnote_definitions
            .iter()
            .map(|definition| normalize_label(&definition.name))
            .collect();

        for node in ctx.root.descendants() {
            let NodeValue::Text(text) = &node.data().value else {
                continue;
            };
            let span = mdk_core::ast::node_span(node);
            let range = ctx.source.span_range(span);
            let exact = ctx.source.slice(range.clone()) == text.as_ref();
            for (name, name_range) in footnote_references(text) {
                if defined.iter().any(|item| item == &normalize_label(&name)) {
                    continue;
                }
                let diagnostic_span = if exact {
                    ctx.source.span_of(
                        range.start + name_range.start..range.start + name_range.end,
                    )
                } else {
                    span
                };
                diagnostics.push(ctx.diagnostic(
                    &MDK001,
                    diagnostic_span,
                    format!("footnote reference `[^{name}]` has no definition"),
                ));
            }
        }
    }
}

/// Finds `[^name]` references in plain text, ignoring inline footnote
/// definitions (`^[...]`).
fn footnote_references(text: &str) -> Vec<(String, std::ops::Range<usize>)> {
    let mut references = Vec::new();
    let mut index = 0;
    while let Some(found) = text[index..].find("[^") {
        let start = index + found;
        let Some(close) = text[start + 2..].find(']') else {
            break;
        };
        let close = start + 2 + close;
        let name = &text[start + 2..close];
        if !name.is_empty() && !name.contains('[') && !name.contains('\n') {
            references.push((name.to_string(), start + 2..close));
        }
        index = close + 1;
    }
    references
}

#[cfg(test)]
mod tests {
    use crate::config::LintConfig;
    use crate::engine::lint_text;
    use mdk_core::Dialect;

    #[test]
    fn mdk001_undefined_footnote() {
        let diagnostics = lint_text("note[^x]\n", Dialect::Gfm, &LintConfig::default());
        assert!(diagnostics.iter().any(|item| item.rule == "MDK001"));
    }

    #[test]
    fn mdk001_defined_footnote_is_fine() {
        let diagnostics = lint_text(
            "note[^x]\n\n[^x]: body\n",
            Dialect::Gfm,
            &LintConfig::default(),
        );
        assert!(!diagnostics.iter().any(|item| item.rule == "MDK001"));
    }

    #[test]
    fn mdk001_ignores_commonmark() {
        let diagnostics =
            lint_text("note[^x]\n", Dialect::Commonmark, &LintConfig::default());
        assert!(!diagnostics.iter().any(|item| item.rule == "MDK001"));
    }
}
