#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! Semantics-preserving Markdown formatter for the `mdk` toolkit.
//!
//! The formatter rewrites whitespace, block structure and inline delimiters
//! while guaranteeing that the rendered HTML stays the same. It never
//! reformats code blocks, HTML blocks or front matter, and it reflows
//! paragraphs only at whitespace, never inside code spans, link destinations
//! or reference labels.
//!
//! ```rust
//! use mdk_core::Dialect;
//! use mdk_fmt::{FormatOptions, format_text};
//!
//! let result = format_text("hello   world\n", Dialect::Commonmark, &FormatOptions::default())
//!     .expect("formatting succeeds");
//! assert_eq!(result.output, "hello world\n");
//! ```
//!
//! When [`FormatOptions::verify`] is enabled (the default), the result is
//! rendered to HTML and compared with the original; a mismatch aborts with
//! [`FormatError::SemanticChange`] instead of returning a document with
//! changed meaning.

mod format;
mod options;
mod prefix;
mod reflow;
mod tables;
mod verify;

pub use format::{FormatError, FormatResult, format_text};
pub use options::{
    BulletStyle, DelimiterStyle, FenceStyle, FormatOptions, HardBreakStyle,
    HeadingStyle, LineEndingStyle, OrderedNumbering, TablePipes,
};
