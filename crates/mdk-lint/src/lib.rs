#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! Markdown linting engine and rules for the `mdk` toolkit.
//!
//! The engine parses a document with [`mdk_core`] (CommonMark or GFM),
//! computes a small set of shared facts about it, then runs every enabled
//! [`Rule`]. Rules emit [`Diagnostic`]s with precise spans and, where safe,
//! autofixes that [`apply_fixes`] can apply in a stable loop.
//!
//! ```rust
//! use mdk_core::Dialect;
//! use mdk_lint::{lint_text, LintConfig};
//!
//! let diagnostics = lint_text("# Heading.\n", Dialect::Commonmark, &LintConfig::default());
//! assert_eq!(diagnostics.len(), 1);
//! assert_eq!(diagnostics[0].rule, "MD026");
//! ```

mod config;
mod context;
mod engine;
mod fix;
mod rules;

#[cfg(test)]
mod testutil;

pub use config::{LintConfig, RuleOptions};
pub use context::{
    Facts, FootnoteInfo, HeadingInfo, LintContext, ReferenceDefinition, ReferenceUsage,
};
pub use engine::lint_text;
pub use fix::{FixOutcome, apply_fixes};
pub use mdk_core::{Diagnostic, Fix, LineKind, Severity, Source, Span};
pub use rules::{RULES, Rule, RuleMeta, rule_by_id, rule_by_name};
