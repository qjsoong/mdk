//! Core types for the `mdk` Markdown toolkit.
//!
//! This crate provides the shared vocabulary used by the linting, formatting
//! and rendering crates:
//!
//! - [`Dialect`]: which flavour of Markdown to parse (CommonMark or GFM).
//! - [`Source`]: an owned document with line/column and byte-offset mapping.
//! - [`Diagnostic`], [`Severity`] and [`Fix`]: lint results and autofixes.
//! - Small text helpers used by every stage of the pipeline.
//!
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! Parsing itself is delegated to the [`comrak`] crate, a Rust port of
//! `cmark-gfm`, so the toolkit follows the CommonMark and GFM specifications
//! closely. [`with_parsed_options`] wires a [`Source`] to a parsed AST while
//! keeping the arena lifetime local to the callback.

pub mod ast;
pub mod diagnostics;
pub mod dialect;
pub mod linekind;
pub mod source;
pub mod text;

use comrak::Options;

pub use diagnostics::{Diagnostic, Fix, Severity};
pub use dialect::Dialect;
pub use linekind::{LineKind, classify};
pub use source::{LineEnding, Position, Source, Span};

/// Parses `source` and calls `f` with the root of the resulting AST.
///
/// The arena holding the AST lives only for the duration of the callback,
/// which keeps the borrow graph local and safe without self-referential
/// structures. Front matter, when present, is masked out before parsing so
/// that it never influences the document structure.
///
/// # Examples
///
/// ```rust
/// use mdk_core::{with_parsed_options, Dialect, Source};
///
/// let source = Source::new("# Hello\n".to_string());
/// let options = Dialect::Commonmark.comrak_options();
/// let headings = with_parsed_options(&source, &options, |root| {
///     root.children()
///         .filter(|node| matches!(node.data().value, comrak::nodes::NodeValue::Heading(_)))
///         .count()
/// });
/// assert_eq!(headings, 1);
/// ```
pub fn with_parsed_options<R>(
    source: &Source,
    options: &Options<'_>,
    f: impl for<'a> FnOnce(comrak::Node<'a>) -> R,
) -> R {
    let parse_text = source.parse_text();
    let arena = comrak::Arena::new();
    let root = comrak::parse_document(&arena, &parse_text, options);
    f(root)
}
