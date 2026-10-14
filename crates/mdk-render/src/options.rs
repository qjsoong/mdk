//! Renderer options.

use serde::{Deserialize, Serialize};

/// Options for the HTML renderer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct HtmlOptions {
    /// Pass raw HTML through instead of escaping it.
    pub unsafe_html: bool,
    /// Prefix for heading anchor ids. `Some("")` enables ids with no prefix.
    pub header_ids: Option<String>,
    /// Apply GitHub-style smart punctuation.
    pub smart: bool,
    /// Render soft breaks as hard breaks.
    pub hardbreaks: bool,
    /// Wrap the fragment in a minimal HTML document.
    pub full_document: bool,
    /// Document title used by `full_document` when the front matter or first
    /// heading does not provide one.
    pub title: String,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        Self {
            unsafe_html: false,
            header_ids: None,
            smart: false,
            hardbreaks: false,
            full_document: false,
            title: "Document".to_string(),
        }
    }
}

/// Options for the ANSI and plain-text terminal renderers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct TermOptions {
    /// Target width in terminal columns.
    pub width: usize,
    /// Emit ANSI escape sequences. When false, output is plain text.
    pub color: bool,
    /// Strip raw HTML instead of showing it.
    pub strip_html: bool,
    /// Draw fenced code blocks with a dim left border (ANSI) or indent them.
    pub code_border: bool,
}

impl Default for TermOptions {
    fn default() -> Self {
        Self {
            width: 80,
            color: true,
            strip_html: false,
            code_border: true,
        }
    }
}

impl TermOptions {
    /// Options for plain-text output: no colour, ASCII tables and markers.
    #[must_use]
    pub fn plain(width: usize) -> Self {
        Self {
            width,
            color: false,
            strip_html: true,
            code_border: false,
        }
    }
}
