//! Semantic verification based on rendered HTML.
//!
//! Both the original and the formatted document are rendered with the same
//! parser options; the HTML is then compared with whitespace collapsed
//! outside `<pre>` elements, because reflowing changes soft line breaks,
//! which are insignificant in HTML. Inline `<code>` collapses whitespace
//! like any other inline element; code spans themselves are never touched by
//! the formatter.

use mdk_core::{Dialect, Source, with_parsed_options};

/// Renders `markdown` to HTML with raw HTML passed through.
pub(crate) fn render_html(markdown: &str, dialect: Dialect) -> String {
    let source = Source::new(markdown.to_string());
    let mut options = dialect.comrak_options();
    options.render.r#unsafe = true;
    with_parsed_options(&source, &options, |root| {
        let mut out = String::new();
        comrak::format_html(root, &options, &mut out)
            .expect("HTML rendering to a string");
        out
    })
}

/// Verifies that `before` and `after` render to equivalent HTML.
pub(crate) fn verify_semantics(
    before: &str,
    after: &str,
    dialect: Dialect,
) -> Result<(), String> {
    let before_html = render_html(before, dialect);
    let after_html = render_html(after, dialect);
    let before_normalized = normalize_whitespace(&before_html);
    let after_normalized = normalize_whitespace(&after_html);
    if before_normalized == after_normalized {
        return Ok(());
    }
    Err(first_difference(&before_normalized, &after_normalized))
}

/// Collapses insignificant whitespace in HTML text nodes.
///
/// Whitespace inside `<pre>` elements is preserved verbatim, as is the
/// content of tags and attribute values.
pub(crate) fn normalize_whitespace(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut code_depth = 0usize;
    let mut in_tag = false;
    let mut pending_space = false;
    let mut rest = html;

    while let Some(ch) = rest.chars().next() {
        if in_tag {
            out.push(ch);
            if ch == '>' {
                in_tag = false;
            }
            rest = &rest[ch.len_utf8()..];
            continue;
        }
        if ch == '<' {
            let is_closing = rest.starts_with("</pre");
            let is_opening = !is_closing && rest.starts_with("<pre");
            out.push('<');
            in_tag = true;
            pending_space = false;
            if is_opening {
                code_depth += 1;
            } else if is_closing {
                code_depth = code_depth.saturating_sub(1);
            }
            rest = &rest[1..];
            continue;
        }
        if code_depth > 0 {
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
            continue;
        }
        if ch.is_whitespace() {
            pending_space = true;
            rest = &rest[ch.len_utf8()..];
            continue;
        }
        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

/// Describes the first difference between two strings with surrounding text.
fn first_difference(left: &str, right: &str) -> String {
    let mut index = 0;
    for (a, b) in left.chars().zip(right.chars()) {
        if a != b {
            break;
        }
        index += a.len_utf8();
    }
    format!(
        "rendered HTML differs near byte {index}:\n  before: {:?}\n  after:  {:?}",
        context(left, index),
        context(right, index)
    )
}

fn context(text: &str, index: usize) -> String {
    let mut start = index.min(text.len());
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    // Walk back at most 40 characters.
    let slice_start = text[..start]
        .char_indices()
        .rev()
        .nth(40)
        .map_or(0, |(offset, _)| offset);
    let mut end = index.min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    let slice_end = text[end..]
        .char_indices()
        .nth(40)
        .map_or(text.len(), |(offset, _)| end + offset);
    text[slice_start..slice_end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_is_ignored_in_text_but_not_in_pre() {
        assert_eq!(
            normalize_whitespace("<p>a\nb</p>"),
            normalize_whitespace("<p>a b</p>")
        );
        assert_eq!(
            normalize_whitespace("<code>a\nb</code>"),
            normalize_whitespace("<code>a b</code>")
        );
        assert_ne!(
            normalize_whitespace("<pre>a\nb</pre>"),
            normalize_whitespace("<pre>a b</pre>")
        );
    }

    #[test]
    fn detects_semantic_changes() {
        let before = "# title\n\nbody\n";
        let after = "# changed\n\nbody\n";
        assert!(verify_semantics(before, after, Dialect::Commonmark).is_err());
    }

    #[test]
    fn accepts_whitespace_changes() {
        let before = "one soft\nbreak\n";
        let after = "one soft break\n";
        assert!(verify_semantics(before, after, Dialect::Commonmark).is_ok());
    }
}
