//! Stable application of autofixes.

use mdk_core::{Dialect, Fix};

use crate::config::LintConfig;
use crate::engine::lint_text;

/// The maximum number of fix/relint passes before giving up.
pub const MAX_PASSES: usize = 10;

/// The result of applying autofixes.
#[derive(Debug, Clone)]
pub struct FixOutcome {
    /// The rewritten document.
    pub text: String,
    /// The number of individual fixes applied across all passes.
    pub applied: usize,
    /// Diagnostics that remain after fixing.
    pub diagnostics: Vec<mdk_core::Diagnostic>,
}

impl FixOutcome {
    /// Whether the document was modified.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.applied > 0
    }

    /// Remaining diagnostics that still carry a fix.
    #[must_use]
    pub fn remaining_fixable(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_fixable())
            .count()
    }
}

/// Applies all safe autofixes repeatedly until the document stops changing.
///
/// Fixes conflict when their byte ranges overlap; the first one in document
/// order wins and the rest are retried on the next pass.
#[must_use]
pub fn apply_fixes(text: &str, dialect: Dialect, config: &LintConfig) -> FixOutcome {
    let mut current = text.to_string();
    let mut applied = 0;
    let mut last = lint_text(&current, dialect, config);

    for _pass in 0..MAX_PASSES {
        let diagnostics = lint_text(&current, dialect, config);
        let mut fixes: Vec<Fix> = diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.fix.clone())
            .collect();
        fixes.sort_by(|a, b| {
            a.range
                .start
                .cmp(&b.range.start)
                .then_with(|| a.range.end.cmp(&b.range.end))
        });

        let mut output = String::with_capacity(current.len());
        let mut cursor = 0;
        let mut pass_applied = 0;
        for fix in fixes {
            if fix.range.start < cursor
                || fix.range.end > current.len()
                || fix.range.start > fix.range.end
            {
                continue;
            }
            output.push_str(&current[cursor..fix.range.start]);
            output.push_str(&fix.replacement);
            cursor = fix.range.end;
            pass_applied += 1;
        }
        output.push_str(&current[cursor..]);

        if pass_applied == 0 || output == current {
            last = diagnostics;
            break;
        }
        applied += pass_applied;
        current = output;
        last = lint_text(&current, dialect, config);
    }

    FixOutcome {
        text: current,
        applied,
        diagnostics: last,
    }
}
