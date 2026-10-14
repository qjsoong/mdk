//! Corpus tests: every fixture must format without changing meaning,
//! re-format to itself, and preserve front matter and code content.

use std::fs;
use std::path::{Path, PathBuf};

use mdk_core::{Dialect, Source};
use mdk_fmt::{
    BulletStyle, DelimiterStyle, FenceStyle, FormatOptions, HeadingStyle,
    OrderedNumbering, format_text,
};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

fn fixtures() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("corpus directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no corpus fixtures found");
    files
}

fn profiles() -> Vec<(&'static str, FormatOptions)> {
    vec![
        ("default", FormatOptions::default()),
        (
            "aggressive",
            FormatOptions {
                line_width: 60,
                heading_style: HeadingStyle::AtxClosed,
                list_bullet: BulletStyle::Dash,
                ordered_list_numbering: OrderedNumbering::Sequential,
                emphasis_style: DelimiterStyle::Asterisk,
                strong_style: DelimiterStyle::Underscore,
                code_fence_style: FenceStyle::Backtick,
                thematic_break: "---".to_string(),
                ..FormatOptions::default()
            },
        ),
        (
            "wide",
            FormatOptions {
                line_width: 200,
                reflow: false,
                ..FormatOptions::default()
            },
        ),
    ]
}

fn front_matter(text: &str) -> Option<String> {
    Source::new(text.to_string())
        .front_matter()
        .map(str::to_string)
}

fn code_blocks(text: &str, dialect: Dialect) -> Vec<String> {
    let source = Source::new(text.to_string());
    let options = dialect.comrak_options();
    mdk_core::with_parsed_options(&source, &options, |root| {
        root.descendants()
            .filter_map(|node| match &node.data().value {
                comrak::nodes::NodeValue::CodeBlock(block) => {
                    Some(block.literal.clone())
                }
                _ => None,
            })
            .collect()
    })
}

#[test]
fn corpus_formats_idempotently_and_preserves_meaning() {
    for path in fixtures() {
        let input = fs::read_to_string(&path).expect("read fixture");
        for (profile_name, options) in profiles() {
            for dialect in [Dialect::Commonmark, Dialect::Gfm] {
                let context = format!(
                    "{} [{profile_name}] {dialect}",
                    path.file_name()
                        .expect("fixture file name")
                        .to_string_lossy()
                );
                let first = format_text(&input, dialect, &options)
                    .unwrap_or_else(|error| panic!("{context}: {error}"));
                let second = format_text(&first.output, dialect, &options)
                    .unwrap_or_else(|error| panic!("{context} (second pass): {error}"));
                assert_eq!(
                    first.output, second.output,
                    "{context}: formatting is not idempotent"
                );
                assert_eq!(
                    front_matter(&input),
                    front_matter(&first.output),
                    "{context}: front matter changed"
                );
                assert_eq!(
                    code_blocks(&input, dialect),
                    code_blocks(&first.output, dialect),
                    "{context}: code block content changed"
                );
                if !first.output.is_empty() {
                    assert!(
                        first.output.ends_with('\n'),
                        "{context}: missing final newline"
                    );
                    assert!(
                        !first.output.ends_with("\n\n"),
                        "{context}: trailing blank line"
                    );
                }
            }
        }
    }
}

#[test]
fn reflow_never_exceeds_width_for_breakable_prose() {
    let input = "This is a normal English paragraph that contains enough words to be wrapped across multiple lines by the formatter because it is much longer than the configured width.";
    for width in [50usize, 72, 80, 100] {
        let options = FormatOptions {
            line_width: width,
            ..FormatOptions::default()
        };
        let result = format_text(input, Dialect::Commonmark, &options).expect("format");
        for line in result.output.lines() {
            assert!(
                mdk_core::text::display_width(line) <= width,
                "width {width}: {line:?}"
            );
        }
    }
}
