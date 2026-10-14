//! Link and reference rules: MD051, MD052 and MD053.

use std::collections::{BTreeMap, BTreeSet};

use comrak::nodes::NodeValue;
use mdk_core::{Diagnostic, Severity};

use crate::context::LintContext;
use crate::rules::{Rule, RuleMeta};

pub(crate) static MD051: RuleMeta = RuleMeta {
    id: "MD051",
    name: "link-fragments",
    description: "Link fragments should be valid",
    default_severity: Severity::Warning,
    fixable: false,
};

pub(crate) struct Md051;

impl Rule for Md051 {
    fn meta(&self) -> &'static RuleMeta {
        &MD051
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let anchors = collect_anchors(ctx);
        for node in ctx.root.descendants() {
            let NodeValue::Link(link) = &node.data().value else {
                continue;
            };
            let Some(fragment) = link.url.strip_prefix('#') else {
                continue;
            };
            if fragment.is_empty() {
                continue;
            }
            let fragment = fragment.to_lowercase();
            if anchors.contains(&fragment) {
                continue;
            }
            diagnostics.push(ctx.diagnostic(
                &MD051,
                mdk_core::ast::node_span(node),
                format!(
                    "link fragment `#{fragment}` does not match any heading or anchor"
                ),
            ));
        }
    }
}

fn collect_anchors(ctx: &LintContext<'_, '_>) -> BTreeSet<String> {
    let mut anchors: BTreeSet<String> = ctx
        .facts()
        .anchors
        .iter()
        .map(|slug| slug.to_lowercase())
        .collect();
    // Collect explicit `id` attributes from raw HTML.
    for node in ctx.root.descendants() {
        let data = node.data();
        let html = match &data.value {
            NodeValue::HtmlInline(html) => Some(html.as_str()),
            NodeValue::HtmlBlock(block) => Some(block.literal.as_str()),
            _ => None,
        };
        let Some(html) = html else { continue };
        for id in html_identifiers(html) {
            anchors.insert(id.to_lowercase());
        }
    }
    anchors
}

fn html_identifiers(html: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let bytes = html.as_bytes();
    let mut index = 0;
    while index + 3 < bytes.len() {
        if &bytes[index..index + 3] != b"id=" {
            index += 1;
            continue;
        }
        index += 3;
        let Some(quote) = bytes.get(index).copied() else {
            break;
        };
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        index += 1;
        let start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index < bytes.len() {
            ids.push(html[start..index].to_string());
        }
        index += 1;
    }
    ids
}

pub(crate) static MD052: RuleMeta = RuleMeta {
    id: "MD052",
    name: "reference-links-images",
    description: "Link and image reference definitions should be defined",
    default_severity: Severity::Error,
    fixable: false,
};

pub(crate) struct Md052;

impl Rule for Md052 {
    fn meta(&self) -> &'static RuleMeta {
        &MD052
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let ignore: Vec<String> = ctx
            .options(&MD052)
            .strings("ignore")
            .into_iter()
            .map(|item| crate::context::normalize_label(&item))
            .collect();
        let defined = ctx.facts().defined_labels();
        for usage in &ctx.facts().usages {
            if defined.contains(usage.normalized.as_str())
                || ignore.iter().any(|label| label == &usage.normalized)
            {
                continue;
            }
            diagnostics.push(ctx.diagnostic(
                &MD052,
                usage.span,
                format!("reference `{}` is not defined", usage.label),
            ));
        }
    }
}

pub(crate) static MD053: RuleMeta = RuleMeta {
    id: "MD053",
    name: "link-image-reference-definitions",
    description: "Link and image reference definitions should be needed",
    default_severity: Severity::Error,
    fixable: false,
};

pub(crate) struct Md053;

impl Rule for Md053 {
    fn meta(&self) -> &'static RuleMeta {
        &MD053
    }

    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>) {
        let ignored: BTreeSet<String> = ctx
            .options(&MD053)
            .strings("ignored_definitions")
            .into_iter()
            .map(|item| crate::context::normalize_label(&item))
            .collect();

        let mut used: BTreeSet<String> = ctx
            .facts()
            .line_usages
            .iter()
            .chain(ctx.facts().usages.iter())
            .map(|usage| usage.normalized.clone())
            .collect();
        // Shortcuts that resolve to a link have their brackets split away
        // from the label, so inspect the raw source of link nodes as well.
        for node in ctx.root.descendants() {
            let data = node.data();
            if !matches!(&data.value, NodeValue::Link(_) | NodeValue::Image(_)) {
                continue;
            }
            let span = mdk_core::ast::node_span(node);
            let raw = ctx.source.slice(ctx.source.span_range(span));
            if !is_shortcut_reference(raw) {
                continue;
            }
            let normalized =
                crate::context::normalize_label(&mdk_core::ast::collect_text(node));
            if ctx
                .facts()
                .definitions
                .iter()
                .any(|definition| definition.normalized == normalized)
            {
                used.insert(normalized);
            }
        }
        // Unresolved shortcut text stays a Text node; scan those too.
        for node in ctx.root.descendants() {
            if let NodeValue::Text(text) = &node.data().value {
                for definition in &ctx.facts().definitions {
                    if !used.contains(&definition.normalized)
                        && contains_shortcut(text, &definition.label)
                    {
                        used.insert(definition.normalized.clone());
                    }
                }
            }
        }

        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for definition in &ctx.facts().definitions {
            let count = seen.entry(definition.normalized.clone()).or_insert(0);
            *count += 1;
            if *count > 1 {
                diagnostics.push(ctx.diagnostic(
                    &MD053,
                    definition.span,
                    format!("duplicate reference definition `{}`", definition.label),
                ));
                continue;
            }
            if used.contains(&definition.normalized)
                || ignored.contains(&definition.normalized)
            {
                continue;
            }
            diagnostics.push(ctx.diagnostic(
                &MD053,
                definition.span,
                format!("reference definition `{}` is unused", definition.label),
            ));
        }
    }
}

/// Whether `raw` is a shortcut reference (`[label]` or `![label]`) rather
/// than an inline, full or collapsed link or image.
fn is_shortcut_reference(raw: &str) -> bool {
    let raw = raw.strip_prefix('!').unwrap_or(raw);
    let Some(inner) = raw
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        return false;
    };
    !inner.is_empty() && !inner.contains("][") && !inner.contains("](")
}

fn contains_shortcut(text: &str, label: &str) -> bool {
    let lowered = text.to_lowercase();
    let needle = format!("[{}]", label.to_lowercase());
    lowered.contains(&needle)
}

#[cfg(test)]
mod tests {
    use crate::testutil::ids;
    use mdk_core::Dialect;

    #[test]
    fn md051_valid_and_invalid_fragments() {
        assert!(ids(&["MD051"], "# Title\n\n[go](#title)\n", Dialect::Gfm).is_empty());
        assert_eq!(
            ids(&["MD051"], "# Title\n\n[go](#missing)\n", Dialect::Gfm),
            vec!["MD051"]
        );
    }

    #[test]
    fn md051_accepts_html_ids() {
        assert!(
            ids(
                &["MD051"],
                "<a id=\"custom\"></a>\n\n[go](#custom)\n",
                Dialect::Gfm
            )
            .is_empty()
        );
    }

    #[test]
    fn md052_undefined_references() {
        assert_eq!(
            ids(&["MD052"], "[text][nope]\n", Dialect::Gfm),
            vec!["MD052"]
        );
        assert!(ids(&["MD052"], "[text][ref]\n\n[ref]: /x\n", Dialect::Gfm).is_empty());
    }

    #[test]
    fn md053_unused_and_duplicate_definitions() {
        assert_eq!(ids(&["MD053"], "[ref]: /x\n", Dialect::Gfm), vec!["MD053"]);
        let duplicates = ids(
            &["MD053"],
            "[ref]: /x\n[ref]: /y\n\n[text][ref]\n",
            Dialect::Gfm,
        );
        assert_eq!(duplicates, vec!["MD053"]);
    }

    #[test]
    fn md053_shortcut_reference_marks_a_definition_used() {
        assert!(ids(&["MD053"], "[ref]: /x\n\nSee [ref].\n", Dialect::Gfm).is_empty());
        assert!(ids(&["MD053"], "[img]: /x\n\n![img]\n", Dialect::Gfm).is_empty());
    }
}
