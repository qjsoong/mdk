#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! Markdown renderers for the `mdk` toolkit.
//!
//! Three output formats share the same parser:
//!
//! - [`render_html`]: CommonMark/GFM HTML, optionally with heading anchors
//!   and a minimal standalone document wrapper;
//! - [`render_terminal`]: ANSI-coloured output for reading Markdown in a
//!   terminal, with wrapped prose, list bullets, box-drawing tables and
//!   footnote sections;
//! - [`TermOptions::plain`] produces the same layout as plain text for
//!   piping into other tools.
//!
//! [`render_terminal_document`] exposes the same terminal rendering as a
//! structured [`term::Document`]: every line and span knows which part of the
//! Markdown source it came from, so interactive readers can implement
//! selection, copy and navigation without re-parsing the document.
//!
//! ```rust
//! use mdk_core::Dialect;
//! use mdk_render::{HtmlOptions, TermOptions, render_html, render_terminal};
//!
//! let html = render_html("# Hi\n", Dialect::Commonmark, &HtmlOptions::default());
//! assert_eq!(html, "<h1>Hi</h1>\n");
//!
//! let text = render_terminal("# Hi\n", Dialect::Commonmark, &TermOptions::plain(40));
//! assert_eq!(text, "# Hi\n");
//! ```

mod html;
mod options;
pub mod term;

pub use html::render_html;
pub use options::{HtmlOptions, TermOptions};
pub use term::{render_terminal, render_terminal_document};
