//! Lint configuration and per-rule options.

use std::collections::BTreeMap;

use mdk_core::Severity;

use crate::rules::RuleMeta;

/// Configuration for the linting stage.
///
/// Every field is optional; the defaults enable all rules with each rule's
/// built-in severity. Rules are selected by canonical identifier (`MD013`) or
/// kebab-case name (`line-length`).
///
/// ```toml
/// [lint]
/// disable = ["line-length"]
/// severity = { MD013 = "warning" }
///
/// [lint.rules.MD013]
/// line_length = 100
/// code_blocks = false
/// ```
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LintConfig {
    /// When non-empty, only these rules run.
    pub enable: Vec<String>,
    /// Rules that never run, even if listed in `enable`.
    pub disable: Vec<String>,
    /// Per-rule severity overrides.
    pub severity: BTreeMap<String, Severity>,
    /// Per-rule option tables.
    pub rules: BTreeMap<String, toml::Value>,
}

impl LintConfig {
    /// Whether the rule is enabled by this configuration.
    #[must_use]
    pub fn is_enabled(&self, meta: &RuleMeta) -> bool {
        if self.disable.iter().any(|item| item_matches(item, meta)) {
            return false;
        }
        self.enable.is_empty()
            || self.enable.iter().any(|item| item_matches(item, meta))
    }

    /// The effective severity for a rule.
    #[must_use]
    pub fn severity_for(&self, meta: &RuleMeta) -> Severity {
        self.severity
            .iter()
            .find(|(key, _)| item_matches(key, meta))
            .map_or(meta.default_severity, |(_, severity)| *severity)
    }

    /// The options table for a rule, if configured.
    #[must_use]
    pub fn options_for(&self, meta: &RuleMeta) -> RuleOptions<'_> {
        self.rules
            .iter()
            .find(|(key, _)| item_matches(key, meta))
            .map_or(RuleOptions { value: None }, |(_, value)| RuleOptions {
                value: Some(value),
            })
    }

    /// Checks that every rule selector and rule table key names a known rule.
    ///
    /// Unknown selectors are otherwise silently ignored, which makes a typo
    /// in `enable`, `disable`, `severity` or a `[lint.rules.*]` table look
    /// like a configuration that has no effect.
    ///
    /// # Errors
    ///
    /// Returns a message listing every unknown selector.
    pub fn validate(&self) -> Result<(), String> {
        let known = |key: &str| {
            crate::rules::RULES
                .iter()
                .any(|rule| item_matches(key, rule.meta()))
        };
        let mut unknown = Vec::new();
        for key in self.enable.iter().chain(self.disable.iter()) {
            if !known(key) {
                unknown.push(format!("`{key}` (enable/disable)"));
            }
        }
        for key in self.severity.keys() {
            if !known(key) {
                unknown.push(format!("`{key}` (severity)"));
            }
        }
        for key in self.rules.keys() {
            if !known(key) {
                unknown.push(format!("`{key}` (rules)"));
            }
        }
        if unknown.is_empty() {
            Ok(())
        } else {
            Err(format!("unknown rule(s): {}", unknown.join(", ")))
        }
    }
}

fn item_matches(item: &str, meta: &RuleMeta) -> bool {
    item.eq_ignore_ascii_case(meta.id) || item.eq_ignore_ascii_case(meta.name)
}

/// Typed access to a rule's TOML options.
///
/// Missing keys fall back to the rule's documented defaults, so a rule never
/// fails to run because of unrelated configuration.
#[derive(Debug, Clone, Copy)]
pub struct RuleOptions<'a> {
    value: Option<&'a toml::Value>,
}

impl<'a> RuleOptions<'a> {
    /// Creates an options view over a raw TOML value.
    #[must_use]
    pub const fn new(value: Option<&'a toml::Value>) -> Self {
        Self { value }
    }

    fn get(&self, key: &str) -> Option<&'a toml::Value> {
        self.value?.get(key)
    }

    /// A boolean option.
    #[must_use]
    pub fn bool(&self, key: &str, default: bool) -> bool {
        self.get(key)
            .and_then(toml::Value::as_bool)
            .unwrap_or(default)
    }

    /// An unsigned integer option.
    #[must_use]
    pub fn usize(&self, key: &str, default: usize) -> usize {
        self.get(key)
            .and_then(toml::Value::as_integer)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(default)
    }

    /// A string option.
    #[must_use]
    pub fn string(&self, key: &str, default: &str) -> String {
        self.get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or(default)
            .to_string()
    }

    /// A list-of-strings option.
    #[must_use]
    pub fn strings(&self, key: &str) -> Vec<String> {
        self.get(key)
            .and_then(toml::Value::as_array)
            .map(|array| {
                array
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A single-character string option.
    #[must_use]
    pub fn char(&self, key: &str, default: char) -> char {
        self.get(key)
            .and_then(toml::Value::as_str)
            .and_then(|value| value.chars().next())
            .unwrap_or(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RuleMeta;

    static META: RuleMeta = RuleMeta {
        id: "MD013",
        name: "line-length",
        description: "test",
        default_severity: Severity::Warning,
        fixable: false,
    };

    #[test]
    fn parsing_accepts_rule_options() {
        let config: LintConfig = toml::from_str(
            r#"
            disable = ["MD013"]
            severity = { MD013 = "error" }

            [rules.MD013]
            line_length = 100
            "#,
        )
        .unwrap();
        assert!(!config.is_enabled(&META));
        assert_eq!(config.severity_for(&META), Severity::Error);
        let options = config.options_for(&META);
        assert_eq!(options.usize("line_length", 80), 100);
        assert!(options.bool("code_blocks", true));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(toml::from_str::<LintConfig>("nope = 1").is_err());
    }

    #[test]
    fn validate_rejects_unknown_rule_names() {
        assert!(LintConfig::default().validate().is_ok());

        let config: LintConfig = LintConfig {
            disable: vec!["MD0OOPS".to_string()],
            ..LintConfig::default()
        };
        assert!(config.validate().is_err());

        let config: LintConfig = LintConfig {
            enable: vec!["line-length".to_string()],
            severity: std::collections::BTreeMap::from([(
                "MD013".to_string(),
                mdk_core::Severity::Info,
            )]),
            rules: std::collections::BTreeMap::new(),
            ..LintConfig::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn options_tables_match_ids_case_insensitively() {
        let config: LintConfig =
            toml::from_str("[rules.md013]\nline_length = 100\n").unwrap();
        assert_eq!(config.options_for(&META).usize("line_length", 80), 100);
    }
}
