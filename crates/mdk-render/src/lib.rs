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
mod term;

pub use html::render_html;
pub use options::{HtmlOptions, TermOptions};
pub use term::render_terminal;
