//! Rule registry.

use mdk_core::{Diagnostic, Severity};

use crate::context::LintContext;

use super::code;
use super::extra;
use super::headings;
use super::inline;
use super::links;
use super::lists;
use super::tables;
use super::whitespace;

/// Static metadata describing a lint rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleMeta {
    /// Canonical identifier such as `MD013`.
    pub id: &'static str,
    /// Kebab-case name such as `line-length`.
    pub name: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// Severity used when the configuration does not override it.
    pub default_severity: Severity,
    /// Whether the rule can emit autofixes.
    pub fixable: bool,
}

/// A lint rule.
pub trait Rule: Send + Sync {
    /// Static metadata for this rule.
    fn meta(&self) -> &'static RuleMeta;

    /// Checks one document, pushing any diagnostics into `diagnostics`.
    fn check(&self, ctx: &LintContext<'_, '_>, diagnostics: &mut Vec<Diagnostic>);
}

/// Every rule known to the engine, in identifier order.
pub static RULES: &[&dyn Rule] = &[
    &headings::Md001,
    &headings::Md003,
    &lists::Md004,
    &lists::Md005,
    &lists::Md007,
    &whitespace::Md009,
    &whitespace::Md010,
    &whitespace::Md012,
    &whitespace::Md013,
    &headings::Md018,
    &headings::Md019,
    &headings::Md022,
    &headings::Md023,
    &headings::Md024,
    &headings::Md025,
    &headings::Md026,
    &whitespace::Md027,
    &whitespace::Md028,
    &lists::Md029,
    &lists::Md030,
    &code::Md031,
    &lists::Md032,
    &inline::Md033,
    &inline::Md034,
    &tables::Md035,
    &inline::Md036,
    &inline::Md038,
    &inline::Md039,
    &code::Md040,
    &headings::Md041,
    &inline::Md042,
    &inline::Md044,
    &inline::Md045,
    &code::Md046,
    &whitespace::Md047,
    &code::Md048,
    &inline::Md049,
    &inline::Md050,
    &links::Md051,
    &links::Md052,
    &links::Md053,
    &tables::Md056,
    &tables::Md058,
    &extra::Mdk001,
];

/// Looks up a rule by canonical identifier.
#[must_use]
pub fn rule_by_id(id: &str) -> Option<&'static dyn Rule> {
    RULES
        .iter()
        .copied()
        .find(|rule| rule.meta().id.eq_ignore_ascii_case(id))
}

/// Looks up a rule by kebab-case name.
#[must_use]
pub fn rule_by_name(name: &str) -> Option<&'static dyn Rule> {
    RULES
        .iter()
        .copied()
        .find(|rule| rule.meta().name.eq_ignore_ascii_case(name))
}
