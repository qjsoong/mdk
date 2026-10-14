//! HTML rendering built on `comrak`.

use mdk_core::{Dialect, Source, with_parsed_options};

use crate::options::HtmlOptions;

/// Renders Markdown to HTML.
#[must_use]
pub fn render_html(markdown: &str, dialect: Dialect, options: &HtmlOptions) -> String {
    let source = Source::new(markdown.to_string());
    let mut comrak_options = dialect.comrak_options();
    comrak_options.render.r#unsafe = options.unsafe_html;
    comrak_options.render.escape = !options.unsafe_html;
    comrak_options.parse.smart = options.smart;
    comrak_options.render.hardbreaks = options.hardbreaks;
    comrak_options.extension.header_id_prefix = options.header_ids.clone();
    let fragment = with_parsed_options(&source, &comrak_options, |root| {
        let mut output = String::new();
        comrak::format_html(root, &comrak_options, &mut output)
            .expect("HTML rendering to a string");
        output
    });
    if options.full_document {
        wrap_document(&fragment, document_title(markdown, options))
    } else {
        fragment
    }
}

fn document_title(markdown: &str, options: &HtmlOptions) -> String {
    let source = Source::new(markdown.to_string());
    if let Some(front_matter) = source.front_matter() {
        for line in front_matter.lines() {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix("title:") {
                let value = value.trim().trim_matches(['"', '\'']);
                if !value.is_empty() {
                    return escape_html(value);
                }
            }
            if let Some(value) = trimmed.strip_prefix("title =") {
                let value = value.trim().trim_matches(['"', '\'']);
                if !value.is_empty() {
                    return escape_html(value);
                }
            }
        }
    }
    let comrak_options = Dialect::Gfm.comrak_options();
    let title = with_parsed_options(&source, &comrak_options, |root| {
        root.descendants()
            .find(|node| {
                matches!(node.data().value, comrak::nodes::NodeValue::Heading(_))
            })
            .map(mdk_core::ast::collect_text)
    });
    title.unwrap_or_else(|| options.title.clone())
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn wrap_document(fragment: &str, title: String) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n\
         </head>\n<body>\n{fragment}</body>\n</html>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_commonmark_fragment() {
        let html = render_html("# Hi\n", Dialect::Commonmark, &HtmlOptions::default());
        assert_eq!(html, "<h1>Hi</h1>\n");
    }

    #[test]
    fn gfm_tables_and_tasklists() {
        let html = render_html(
            "- [x] done\n\n| a |\n|---|\n| 1 |\n",
            Dialect::Gfm,
            &HtmlOptions::default(),
        );
        assert!(html.contains("checkbox"));
        assert!(html.contains("<table>"));
    }

    #[test]
    fn raw_html_is_escaped_unless_allowed() {
        let escaped =
            render_html("<b>x</b>\n", Dialect::Commonmark, &HtmlOptions::default());
        assert!(escaped.contains("&lt;b&gt;"));
        let options = HtmlOptions {
            unsafe_html: true,
            ..HtmlOptions::default()
        };
        let raw = render_html("<b>x</b>\n", Dialect::Commonmark, &options);
        assert!(raw.contains("<b>x</b>"));
    }

    #[test]
    fn full_document_uses_front_matter_title() {
        let options = HtmlOptions {
            full_document: true,
            ..HtmlOptions::default()
        };
        let html =
            render_html("---\ntitle: Hello\n---\n\nbody\n", Dialect::Gfm, &options);
        assert!(html.contains("<title>Hello</title>"));
        assert!(html.contains("<p>body</p>"));
    }
}
