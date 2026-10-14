//! Test helpers shared by rule unit tests.

use mdk_core::{Diagnostic, Dialect};

use crate::{LintConfig, lint_text};

/// Lints `markdown` with only the named rules enabled.
pub fn lint_with(rules: &[&str], markdown: &str, dialect: Dialect) -> Vec<Diagnostic> {
    let config = LintConfig {
        enable: rules.iter().map(|rule| (*rule).to_string()).collect(),
        ..LintConfig::default()
    };
    lint_text(markdown, dialect, &config)
}

/// Lints `markdown` with only the named rules enabled and returns their ids.
pub fn ids(rules: &[&str], markdown: &str, dialect: Dialect) -> Vec<&'static str> {
    lint_with(rules, markdown, dialect)
        .iter()
        .map(|diagnostic| diagnostic.rule)
        .collect()
}
