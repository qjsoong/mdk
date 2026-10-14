//! The lint engine.

use mdk_core::{Diagnostic, Dialect, Source};

use crate::config::LintConfig;
use crate::context::LintContext;
use crate::rules::RULES;

/// Lints a Markdown document and returns sorted diagnostics.
///
/// All enabled rules run exactly once over the parsed document. Diagnostics
/// are ordered by position, then by rule identifier.
///
/// # Examples
///
/// ```rust
/// use mdk_core::Dialect;
/// use mdk_lint::{LintConfig, lint_text};
///
/// let diagnostics = lint_text("#  spaced\n", Dialect::Commonmark, &LintConfig::default());
/// assert_eq!(diagnostics.len(), 1);
/// assert_eq!(diagnostics[0].rule, "MD019");
/// ```
#[must_use]
pub fn lint_text(text: &str, dialect: Dialect, config: &LintConfig) -> Vec<Diagnostic> {
    let source = Source::new(text.to_string());
    lint_source(&source, dialect, config)
}

/// Lints an already-prepared [`Source`].
#[must_use]
pub fn lint_source(
    source: &Source,
    dialect: Dialect,
    config: &LintConfig,
) -> Vec<Diagnostic> {
    let options = dialect.comrak_options();
    mdk_core::with_parsed_options(source, &options, |root| {
        let ctx = LintContext::new(source, root, dialect, config);
        let mut diagnostics = Vec::new();
        for rule in RULES {
            if config.is_enabled(rule.meta()) {
                rule.check(&ctx, &mut diagnostics);
            }
        }
        diagnostics.sort_by(|a, b| {
            a.span
                .start
                .cmp(&b.span.start)
                .then_with(|| a.rule.cmp(b.rule))
                .then_with(|| a.span.end.cmp(&b.span.end))
        });
        diagnostics.dedup();
        diagnostics
    })
}
