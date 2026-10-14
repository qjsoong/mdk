//! Configuration file loading and discovery.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mdk_core::Dialect;
use mdk_fmt::FormatOptions;
use mdk_lint::LintConfig;
use mdk_render::{HtmlOptions, TermOptions};
use serde::Deserialize;

/// The top-level `mdk.toml` configuration.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Dialect used when the command line does not specify one.
    pub dialect: Option<Dialect>,
    /// Lint settings.
    pub lint: LintConfig,
    /// Formatter settings.
    pub fmt: FormatOptions,
    /// Renderer settings.
    pub render: RenderConfig,
}

/// Renderer settings.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RenderConfig {
    /// HTML renderer defaults.
    pub html: HtmlOptions,
    /// Terminal renderer defaults.
    pub term: TermOptions,
}

impl Config {
    /// Loads the configuration, returning the config and the path it came
    /// from (when one was found).
    ///
    /// # Errors
    ///
    /// Fails when an explicit configuration file is missing, unreadable or
    /// invalid.
    pub fn load(explicit: Option<&Path>) -> Result<(Self, Option<PathBuf>)> {
        let path = match explicit {
            Some(path) => Some(path.to_path_buf()),
            None => discover(),
        };
        let Some(path) = path else {
            return Ok((Self::default(), None));
        };
        let text = fs::read_to_string(&path).with_context(|| {
            format!("cannot read configuration file {}", path.display())
        })?;
        let config: Self = toml::from_str(&text).with_context(|| {
            format!("invalid configuration file {}", path.display())
        })?;
        config.lint.validate().map_err(|message| {
            anyhow::anyhow!(
                "invalid lint configuration in {}: {message}",
                path.display()
            )
        })?;
        Ok((config, Some(path)))
    }

    /// The effective dialect, given an optional command-line override.
    #[must_use]
    pub fn dialect(&self, override_: Option<Dialect>) -> Dialect {
        override_.or(self.dialect).unwrap_or(Dialect::Gfm)
    }
}

/// Searches the current directory and its ancestors for `mdk.toml` or
/// `.mdk.toml`.
fn discover() -> Option<PathBuf> {
    let mut directory = std::env::current_dir().ok()?;
    loop {
        for name in ["mdk.toml", ".mdk.toml"] {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if !directory.pop() {
            return None;
        }
    }
}
