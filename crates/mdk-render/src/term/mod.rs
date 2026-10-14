//! ANSI and plain-text terminal rendering.

mod emit;
mod table;

use std::collections::BTreeMap;

use comrak::Node;
use comrak::nodes::{NodeValue, TableAlignment};
use mdk_core::ast::collect_text;
use mdk_core::{Dialect, Source, with_parsed_options};

use crate::options::TermOptions;
use emit::{Color, Emitter, Span, Style};
use table::TableChars;

/// Renders Markdown for display in a terminal.
///
/// With `color` enabled the output uses ANSI escape sequences; otherwise the
/// result is plain text with ASCII tables, bullets and rules.
#[must_use]
pub fn render_terminal(
    markdown: &str,
    dialect: Dialect,
    options: &TermOptions,
) -> String {
    let source = Source::new(markdown.to_string());
    let comrak_options = dialect.comrak_options();
    with_parsed_options(&source, &comrak_options, |root| {
        let mut renderer = TerminalRenderer::new(options, root);
        renderer.render_blocks(root, true);
        renderer.render_footnotes(root);
        renderer.emitter.finish()
    })
}

struct TerminalRenderer<'a> {
    emitter: Emitter,
    options: &'a TermOptions,
    footnote_numbers: BTreeMap<String, usize>,
}

impl<'a> TerminalRenderer<'a> {
    fn new(options: &'a TermOptions, root: Node<'_>) -> Self {
        let mut footnote_numbers = BTreeMap::new();
        let mut next = 1usize;
        for node in root.descendants() {
            if let NodeValue::FootnoteReference(reference) = &node.data().value {
                footnote_numbers
                    .entry(reference.name.clone())
                    .or_insert_with(|| {
                        let number = reference.ix as usize;
                        next = next.max(number + 1);
                        number
                    });
            }
        }
        for node in root.descendants() {
            if let NodeValue::FootnoteDefinition(definition) = &node.data().value {
                footnote_numbers
                    .entry(definition.name.clone())
                    .or_insert_with(|| {
                        let number = next;
                        next += 1;
                        number
                    });
            }
        }
        Self {
            emitter: Emitter::new(options.width, options.color, Vec::new()),
            options,
            footnote_numbers,
        }
    }

    fn render_blocks(&mut self, node: Node<'_>, loose: bool) {
        let children: Vec<Node<'_>> = node
            .children()
            .filter(|child| !matches!(child.data().value, NodeValue::FrontMatter(_)))
            .collect();
        for (index, child) in children.iter().enumerate() {
            if index > 0 {
                if loose {
                    self.emitter.blank();
                } else {
                    self.emitter.end_line();
                }
            }
            self.render_block(child);
        }
    }

    fn render_block(&mut self, node: Node<'_>) {
        match &node.data().value {
            NodeValue::Paragraph => {
                let spans = self.inline_spans(node, Style::default());
                self.emitter.push_spans(&spans);
            }
            NodeValue::Heading(heading) => {
                let (style, plain_prefix) = heading_style(heading.level);
                if !self.options.color && plain_prefix > 0 {
                    self.emitter.push_text(
                        Style::default().dim(),
                        &format!("{} ", "#".repeat(plain_prefix)),
                    );
                }
                let spans = self.inline_spans(node, style);
                self.emitter.push_spans(&spans);
            }
            NodeValue::CodeBlock(code) => self.render_code_block(code),
            NodeValue::HtmlBlock(block) => {
                if !self.options.strip_html {
                    self.emitter.push_raw(
                        Style::default().dim(),
                        block.literal.trim_end_matches('\n'),
                    );
                    self.emitter.newline();
                }
            }
            NodeValue::BlockQuote => {
                let quote = if self.options.color { "│ " } else { "> " };
                let mut prefix = self.emitter.prefix().to_vec();
                prefix.push(Span::new(Style::default().dim(), quote));
                let old = self.emitter.set_prefix(prefix);
                self.render_blocks(node, true);
                self.emitter.set_prefix(old);
            }
            NodeValue::List(list) => self.render_list(node, list),
            NodeValue::Table(table) => {
                self.render_table(node, table.alignments.clone())
            }
            NodeValue::ThematicBreak => {
                let width = self.emitter.content_width().max(3);
                let rule: String = if self.options.color {
                    "─".repeat(width)
                } else {
                    "-".repeat(width)
                };
                self.emitter.push_text(Style::default().dim(), &rule);
                self.emitter.newline();
            }
            NodeValue::FootnoteDefinition(_) | NodeValue::FrontMatter(_) => {}
            _ => self.render_blocks(node, true),
        }
    }

    fn render_code_block(&mut self, code: &comrak::nodes::NodeCodeBlock) {
        let style = Style::default().dim();
        let border = if self.options.color && self.options.code_border {
            "│ "
        } else if self.options.color {
            "  "
        } else {
            "    "
        };
        let mut prefix = self.emitter.prefix().to_vec();
        prefix.push(Span::new(style, border));
        let old = self.emitter.set_prefix(prefix);
        let content = code.literal.trim_end_matches('\n');
        if !content.is_empty() {
            for line in content.split('\n') {
                self.emitter.push_raw(style, line);
                self.emitter.newline();
            }
        }
        self.emitter.set_prefix(old);
    }

    fn render_list(&mut self, node: Node<'_>, list: &comrak::nodes::NodeList) {
        self.emitter.end_line();
        let items: Vec<Node<'_>> = node
            .children()
            .filter(|child| {
                matches!(
                    child.data().value,
                    NodeValue::Item(_) | NodeValue::TaskItem(_)
                )
            })
            .collect();
        let loose = !list.tight;
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                if loose {
                    self.emitter.blank();
                } else {
                    self.emitter.end_line();
                }
            }
            self.render_item(item, list, index, !list.tight);
        }
        self.emitter.end_line();
    }

    fn render_item(
        &mut self,
        item: Node<'_>,
        list: &comrak::nodes::NodeList,
        index: usize,
        loose: bool,
    ) {
        let marker = match list.list_type {
            comrak::nodes::ListType::Bullet => {
                if self.options.color {
                    "•".to_string()
                } else {
                    "-".to_string()
                }
            }
            comrak::nodes::ListType::Ordered => format!("{}.", list.start + index),
        };
        self.emitter
            .push_text(Style::default(), &format!("{marker} "));
        let mut marker_width = mdk_core::text::display_width(&marker) + 1;

        if let NodeValue::TaskItem(task) = &item.data().value {
            let (text, style) = if task.symbol.is_some() {
                ("[x] ", Style::default().fg(Color::Green))
            } else {
                ("[ ] ", Style::default().dim())
            };
            self.emitter.push_text(style, text);
            marker_width += 4;
        }

        let mut prefix = self.emitter.prefix().to_vec();
        prefix.push(Span::plain(" ".repeat(marker_width)));
        let old = self.emitter.set_prefix(prefix);
        self.render_blocks(item, loose);
        self.emitter.set_prefix(old);
    }

    fn render_table(&mut self, node: Node<'_>, alignments: Vec<TableAlignment>) {
        let rows: Vec<Vec<Vec<Span>>> = node
            .children()
            .filter_map(|row| match &row.data().value {
                NodeValue::TableRow(_) => Some(
                    row.children()
                        .filter(|cell| {
                            matches!(cell.data().value, NodeValue::TableCell)
                        })
                        .map(|cell| self.inline_spans(cell, Style::default()))
                        .collect(),
                ),
                _ => None,
            })
            .collect();
        let chars = if self.options.color {
            TableChars::unicode()
        } else {
            TableChars::ascii()
        };
        let prefix = self.emitter.prefix().to_vec();
        if let Some(lines) =
            table::render_table(&rows, &alignments, self.options.width, &prefix, &chars)
        {
            for line in lines {
                self.emitter.push_line(&line);
            }
            return;
        }
        // Fallback: render cells as a paragraph line.
        for (index, row) in rows.iter().enumerate() {
            if index > 0 {
                self.emitter.newline();
            }
            for (cell_index, cell) in row.iter().enumerate() {
                if cell_index > 0 {
                    self.emitter.push_text(Style::default().dim(), " | ");
                }
                self.emitter.push_spans(cell);
            }
        }
        self.emitter.end_line();
    }

    fn render_footnotes(&mut self, root: Node<'_>) {
        let mut definitions: Vec<(usize, Node<'_>)> = Vec::new();
        for node in root.descendants() {
            if let NodeValue::FootnoteDefinition(definition) = &node.data().value {
                let number = self
                    .footnote_numbers
                    .get(&definition.name)
                    .copied()
                    .unwrap_or(1);
                definitions.push((number, node));
            }
        }
        if definitions.is_empty() {
            return;
        }
        definitions.sort_by_key(|(number, _)| *number);
        self.emitter.blank();
        for (number, node) in definitions {
            self.emitter.blank();
            self.emitter
                .push_text(Style::default().fg(Color::Cyan), &format!("[{number}] "));
            let mut prefix = self.emitter.prefix().to_vec();
            prefix.push(Span::plain(" ".repeat(format!("[{number}] ").len())));
            let old = self.emitter.set_prefix(prefix);
            self.render_blocks(node, true);
            self.emitter.set_prefix(old);
        }
    }

    fn inline_spans(&self, node: Node<'_>, style: Style) -> Vec<Span> {
        let mut spans = Vec::new();
        for child in node.children() {
            self.collect_inline(child, style, &mut spans);
        }
        spans
    }

    fn collect_inline(&self, node: Node<'_>, style: Style, out: &mut Vec<Span>) {
        match &node.data().value {
            NodeValue::Text(text) => out.push(Span::new(style, text.to_string())),
            NodeValue::SoftBreak => out.push(Span::plain(" ")),
            NodeValue::LineBreak => out.push(Span::plain("\n")),
            NodeValue::Code(code) => out.push(Span::atomic(
                style.fg(Color::BrightYellow),
                code.literal.clone(),
            )),
            NodeValue::Emph => self.collect_children(node, style.italic(), out),
            NodeValue::Strong => self.collect_children(node, style.bold(), out),
            NodeValue::Strikethrough => {
                self.collect_children(node, style.strikethrough(), out);
            }
            NodeValue::Highlight => {
                self.collect_children(node, style.bg(Color::Yellow), out)
            }
            NodeValue::Insert | NodeValue::Underline => {
                self.collect_children(node, style.underline(), out);
            }
            NodeValue::Superscript | NodeValue::Subscript => {
                self.collect_children(node, style.dim(), out);
            }
            NodeValue::Link(link) => {
                let link_style = style.fg(Color::BrightBlue).underline();
                self.collect_children(node, link_style, out);
                let text = collect_text(node);
                if !link.url.is_empty() && text.trim() != link.url {
                    out.push(Span::plain(" "));
                    out.push(Span::atomic(
                        style.dim().fg(Color::BrightBlack),
                        format!("({})", link.url),
                    ));
                }
            }
            NodeValue::Image(link) => {
                let alt = collect_text(node);
                let label = if alt.trim().is_empty() {
                    "image".to_string()
                } else {
                    format!("image: {}", alt.trim())
                };
                out.push(Span::atomic(
                    style.fg(Color::BrightBlue).underline(),
                    format!("[{label}]"),
                ));
                if !link.url.is_empty() {
                    out.push(Span::plain(" "));
                    out.push(Span::atomic(
                        style.dim().fg(Color::BrightBlack),
                        format!("({})", link.url),
                    ));
                }
            }
            NodeValue::FootnoteReference(reference) => {
                let number = self
                    .footnote_numbers
                    .get(&reference.name)
                    .copied()
                    .unwrap_or(reference.ix as usize);
                out.push(Span::atomic(style.fg(Color::Cyan), format!("[{number}]")));
            }
            NodeValue::HtmlInline(html) => {
                if !self.options.strip_html {
                    out.push(Span::atomic(style.dim(), html.clone()));
                }
            }
            NodeValue::Math(math) => {
                out.push(Span::atomic(style.fg(Color::Magenta), math.literal.clone()))
            }
            NodeValue::Escaped | NodeValue::EscapedTag(_) => {}
            _ => self.collect_children(node, style, out),
        }
    }

    fn collect_children(&self, node: Node<'_>, style: Style, out: &mut Vec<Span>) {
        for child in node.children() {
            self.collect_inline(child, style, out);
        }
    }
}

fn heading_style(level: u8) -> (Style, usize) {
    match level {
        1 => (
            Style {
                bold: true,
                underline: true,
                ..Style::default()
            },
            1,
        ),
        2 => (Style::default().bold().fg(Color::BrightBlue), 2),
        3 => (Style::default().bold().fg(Color::BrightCyan), 3),
        other => (
            Style::default().bold().fg(Color::BrightYellow),
            usize::from(other),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ansi(markdown: &str) -> String {
        render_terminal(markdown, Dialect::Gfm, &TermOptions::default())
    }

    fn plain(markdown: &str) -> String {
        render_terminal(markdown, Dialect::Gfm, &TermOptions::plain(60))
    }

    #[test]
    fn strips_ansi_for_non_tty() {
        let output = plain("# Title\n\nHello *world*.\n");
        assert!(!output.contains('\u{1b}'));
        assert!(output.contains("# Title"));
        assert!(output.contains("Hello world."));
    }

    #[test]
    fn renders_lists_and_tasks() {
        let output = plain("- one\n- [x] two\n");
        assert!(output.contains("- one"));
        assert!(output.contains("[x] two"));
    }

    #[test]
    fn renders_code_with_indent() {
        let output = plain("```\nlet x = 1;\n```\n");
        assert!(output.contains("    let x = 1;"));
    }

    #[test]
    fn renders_tables() {
        let output = plain("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(output.contains("| a"));
        assert!(output.contains("+--"));
    }

    #[test]
    fn renders_links_with_destination() {
        let output = plain("[text](https://example.com)\n");
        assert!(output.contains("text (https://example.com)"));
    }

    #[test]
    fn renders_footnotes_at_the_end() {
        let output = plain("note[^a]\n\n[^a]: body\n");
        assert!(output.contains("note[1]"));
        assert!(output.contains("[1] body"));
    }

    #[test]
    fn ansi_contains_escape_sequences() {
        let output = ansi("**bold**\n");
        assert!(output.contains("\u{1b}[1m"));
    }
}
