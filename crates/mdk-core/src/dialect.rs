//! Markdown dialects understood by the toolkit.

use std::fmt;
use std::str::FromStr;

use comrak::Options;

/// A Markdown dialect.
///
/// The toolkit currently speaks the two dialects with formal specifications:
/// CommonMark and GitHub Flavored Markdown (GFM).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Dialect {
    /// [CommonMark](https://commonmark.org/) only, with every extension
    /// disabled.
    #[default]
    Commonmark,
    /// CommonMark plus the GitHub Flavored Markdown extensions: tables,
    /// strikethrough, autolinks, task lists and footnotes.
    Gfm,
}

impl Dialect {
    /// The canonical lowercase name of the dialect.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Commonmark => "commonmark",
            Self::Gfm => "gfm",
        }
    }

    /// A one-line human-readable description.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Commonmark => "CommonMark (no extensions)",
            Self::Gfm => "GitHub Flavored Markdown",
        }
    }

    /// Builds the `comrak` parser options for this dialect.
    ///
    /// Character-based source positions are always enabled so diagnostics and
    /// formatter decisions can address UTF-8 documents by character column.
    #[must_use]
    pub fn comrak_options(self) -> Options<'static> {
        let mut options = Options::default();
        options.parse.sourcepos_chars = true;
        self.apply_extensions(&mut options);
        options
    }

    /// Enables the dialect's extensions on an existing option set.
    pub fn apply_extensions(self, options: &mut Options<'_>) {
        match self {
            Self::Commonmark => {}
            Self::Gfm => {
                options.extension.strikethrough = true;
                options.extension.table = true;
                options.extension.autolink = true;
                options.extension.tasklist = true;
                options.extension.footnotes = true;
            }
        }
    }
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Error returned when a dialect name cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown Markdown dialect `{0}` (expected `commonmark` or `gfm`)")]
pub struct UnknownDialect(pub String);

impl FromStr for Dialect {
    type Err = UnknownDialect;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "commonmark" | "common-mark" | "cm" => Ok(Self::Commonmark),
            "gfm" | "github" | "github-flavored-markdown" => Ok(Self::Gfm),
            _ => Err(UnknownDialect(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_case_insensitively() {
        assert_eq!("GFM".parse(), Ok(Dialect::Gfm));
        assert_eq!("commonmark".parse(), Ok(Dialect::Commonmark));
        assert!("markdown".parse::<Dialect>().is_err());
    }

    #[test]
    fn gfm_options_enable_extensions() {
        let options = Dialect::Gfm.comrak_options();
        assert!(options.extension.table);
        assert!(options.extension.tasklist);
        assert!(options.parse.sourcepos_chars);
    }
}
