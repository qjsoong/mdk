//! Formatter options.

use serde::{Deserialize, Serialize};

/// How hard line breaks (two trailing spaces or a backslash) are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardBreakStyle {
    /// Write hard breaks as two trailing spaces.
    #[default]
    Spaces,
    /// Write hard breaks as a trailing backslash.
    Backslash,
    /// Keep whatever marker the source used.
    Preserve,
}

/// Preferred heading style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadingStyle {
    /// ATX headings without closing hashes (`# Title`).
    #[default]
    Atx,
    /// ATX headings with closing hashes (`# Title #`).
    AtxClosed,
    /// Setext headings for levels 1 and 2 (`Title` over `====`).
    Setext,
    /// Keep the existing style; only normalise spacing.
    Preserve,
}

/// Preferred unordered list bullet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BulletStyle {
    /// Keep the existing bullet characters.
    #[default]
    Preserve,
    /// Use `-`.
    Dash,
    /// Use `*`.
    Asterisk,
    /// Use `+`.
    Plus,
}

/// How ordered list numbers are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderedNumbering {
    /// Keep the numbers written by the author.
    #[default]
    Preserve,
    /// Renumber sequentially from the list's start value.
    Sequential,
    /// Use `1.` for every item.
    One,
}

/// Preferred emphasis or strong delimiter character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelimiterStyle {
    /// Keep the existing delimiters.
    #[default]
    Preserve,
    /// Use asterisks.
    Asterisk,
    /// Use underscores.
    Underscore,
}

/// Preferred code fence character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FenceStyle {
    /// Keep the existing fence character.
    #[default]
    Preserve,
    /// Use backticks.
    Backtick,
    /// Use tildes.
    Tilde,
}

/// Whether tables gain outer pipes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TablePipes {
    /// Mirror the header row's leading and trailing pipes.
    #[default]
    Preserve,
    /// Always write leading and trailing pipes.
    Always,
}

/// Line ending written by the formatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEndingStyle {
    /// Keep the document's dominant line ending.
    #[default]
    Preserve,
    /// Unix line endings.
    Lf,
    /// Windows line endings.
    Crlf,
}

/// Options controlling the Markdown formatter.
///
/// Every field has a default; unknown keys in configuration files are
/// rejected so typos do not silently change formatting behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct FormatOptions {
    /// Target maximum line width in display columns for reflowed prose.
    pub line_width: usize,
    /// Wrap paragraphs to `line_width`.
    pub reflow: bool,
    /// How hard line breaks are written.
    pub hard_break_style: HardBreakStyle,
    /// Remove trailing whitespace outside verbatim blocks.
    pub trim_trailing_whitespace: bool,
    /// Maximum number of consecutive blank lines to keep.
    pub max_blank_lines: usize,
    /// Insert a blank line between adjacent block-level elements where it is
    /// safe to do so.
    pub blank_lines_around_blocks: bool,
    /// Require a blank line between front matter and the document body.
    pub blank_line_after_front_matter: bool,
    /// Preferred heading style.
    pub heading_style: HeadingStyle,
    /// Preferred unordered list bullet.
    pub list_bullet: BulletStyle,
    /// Spaces between a list marker and its content.
    pub list_marker_space: usize,
    /// How ordered list numbers are written.
    pub ordered_list_numbering: OrderedNumbering,
    /// Preferred emphasis delimiter.
    pub emphasis_style: DelimiterStyle,
    /// Preferred strong-emphasis delimiter.
    pub strong_style: DelimiterStyle,
    /// Preferred code fence character.
    pub code_fence_style: FenceStyle,
    /// Add a closing fence to fenced code blocks that lack one.
    pub close_code_fences: bool,
    /// Preferred thematic break text (`---`, `***`, `___`) or `preserve`.
    pub thematic_break: String,
    /// Pad table cells so columns line up.
    pub align_tables: bool,
    /// Whether tables gain outer pipes.
    pub table_pipes: TablePipes,
    /// Ensure the file ends with exactly one newline.
    pub final_newline: bool,
    /// Line ending style to write.
    pub line_ending: LineEndingStyle,
    /// Verify that formatting preserves the rendered HTML before returning.
    pub verify: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            line_width: 80,
            reflow: true,
            hard_break_style: HardBreakStyle::Spaces,
            trim_trailing_whitespace: true,
            max_blank_lines: 1,
            blank_lines_around_blocks: true,
            blank_line_after_front_matter: true,
            heading_style: HeadingStyle::Atx,
            list_bullet: BulletStyle::Preserve,
            list_marker_space: 1,
            ordered_list_numbering: OrderedNumbering::Preserve,
            emphasis_style: DelimiterStyle::Preserve,
            strong_style: DelimiterStyle::Preserve,
            code_fence_style: FenceStyle::Preserve,
            close_code_fences: true,
            thematic_break: "preserve".to_string(),
            align_tables: true,
            table_pipes: TablePipes::Preserve,
            final_newline: true,
            line_ending: LineEndingStyle::Preserve,
            verify: true,
        }
    }
}

impl FormatOptions {
    /// Whether the thematic break option selects a concrete marker.
    #[must_use]
    pub fn thematic_break_marker(&self) -> Option<&'static str> {
        match self.thematic_break.as_str() {
            "---" => Some("---"),
            "***" => Some("***"),
            "___" => Some("___"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let options = FormatOptions::default();
        assert_eq!(options.line_width, 80);
        assert!(options.reflow);
        assert_eq!(options.max_blank_lines, 1);
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(toml::from_str::<FormatOptions>("nope = true").is_err());
    }

    #[test]
    fn parses_a_config_table() {
        let options: FormatOptions = toml::from_str(
            "line_width = 100\nheading_style = \"atx\"\nlist_bullet = \"dash\"\n",
        )
        .unwrap();
        assert_eq!(options.line_width, 100);
        assert_eq!(options.list_bullet, BulletStyle::Dash);
        assert_eq!(options.heading_style, HeadingStyle::Atx);
    }
}
