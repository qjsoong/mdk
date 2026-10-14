//! Lint results and autofixes.

use std::fmt;
use std::ops::Range;

use crate::{Span, source::Position};

/// The severity of a diagnostic.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A problem that should fail a lint run.
    #[default]
    Error,
    /// A stylistic problem that does not fail a lint run by default.
    Warning,
    /// Informational output.
    Info,
}

impl Severity {
    /// The lowercase name used in configuration files and JSON output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single-source replacement that resolves a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Fix {
    /// Byte range in the original document to replace.
    pub range: Range<usize>,
    /// Replacement text.
    pub replacement: String,
}

impl Fix {
    /// Creates a fix replacing `range` with `replacement`.
    #[must_use]
    pub fn new(range: Range<usize>, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }
}

/// A problem found by a lint rule.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    /// Canonical rule identifier, for example `MD013`.
    pub rule: &'static str,
    /// Configured severity.
    pub severity: Severity,
    /// Human-readable description of the problem.
    pub message: String,
    /// Location of the problem in the source document.
    pub span: Span,
    /// An optional autofix. Byte offsets refer to the original document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

impl Diagnostic {
    /// Creates a diagnostic without a fix.
    #[must_use]
    pub fn new(
        rule: &'static str,
        severity: Severity,
        message: impl Into<String>,
        span: Span,
    ) -> Self {
        Self {
            rule,
            severity,
            message: message.into(),
            span,
            fix: None,
        }
    }

    /// Attaches a fix to this diagnostic.
    #[must_use]
    pub fn with_fix(
        mut self,
        range: Range<usize>,
        replacement: impl Into<String>,
    ) -> Self {
        self.fix = Some(Fix::new(range, replacement));
        self
    }

    /// Whether this diagnostic carries an autofix.
    #[must_use]
    pub const fn is_fixable(&self) -> bool {
        self.fix.is_some()
    }

    /// Convenience constructor for a diagnostic at a single point.
    #[must_use]
    pub fn point(
        rule: &'static str,
        severity: Severity,
        message: impl Into<String>,
        position: Position,
    ) -> Self {
        Self::new(rule, severity, message, Span::point(position))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_uses_lowercase_severity() {
        let diagnostic = Diagnostic::point(
            "MD041",
            Severity::Warning,
            "first line should be a top-level heading",
            Position::new(1, 1),
        );
        let json = serde_json::to_string(&diagnostic).unwrap();
        assert!(json.contains(r#""severity":"warning""#));
        assert!(!json.contains("fix"));
    }
}
