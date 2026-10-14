//! Command-line definition.

use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use mdk_core::Dialect;
use mdk_fmt::{FenceStyle, FormatOptions, HeadingStyle, LineEndingStyle};

/// Markdown Kit: lint, format and render CommonMark and GFM.
#[derive(Debug, Parser)]
#[command(
    name = "mdk",
    version,
    about,
    propagate_version = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Markdown dialect used for parsing
    #[arg(long, global = true, value_name = "DIALECT", value_parser = parse_dialect)]
    pub dialect: Option<Dialect>,

    /// Configuration file (defaults to the nearest mdk.toml upwards)
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// When to use colour in human-readable output
    #[arg(long, global = true, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,

    /// Suppress summaries and informational output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Increase output detail (repeatable)
    #[arg(short, long, global = true, action = ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Lint Markdown documents and report problems
    Lint(LintArgs),
    /// Format Markdown documents
    #[command(visible_alias = "format")]
    Fmt(FmtArgs),
    /// Convert Markdown to HTML, ANSI or plain text
    #[command(visible_alias = "translate")]
    Render(RenderArgs),
    /// List the available lint rules
    Rules(RulesArgs),
}

/// When to use colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorChoice {
    /// Colour only when the output is a terminal.
    Auto,
    /// Always colour.
    Always,
    /// Never colour.
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LintOutput {
    /// Readable diagnostics with file, line and column.
    Human,
    /// Machine-readable JSON.
    Json,
    /// GitHub Actions workflow commands.
    Github,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RenderTarget {
    /// HTML fragment or document.
    Html,
    /// ANSI-coloured text for terminals.
    Ansi,
    /// Plain text with ASCII tables and markers.
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RulesOutput {
    /// Aligned table.
    Text,
    /// JSON array of rule metadata.
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LineEndingArg {
    /// Keep the document's dominant line ending.
    Preserve,
    /// Unix line endings.
    Lf,
    /// Windows line endings.
    Crlf,
}

impl From<LineEndingArg> for LineEndingStyle {
    fn from(value: LineEndingArg) -> Self {
        match value {
            LineEndingArg::Preserve => Self::Preserve,
            LineEndingArg::Lf => Self::Lf,
            LineEndingArg::Crlf => Self::Crlf,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HeadingStyleArg {
    Atx,
    AtxClosed,
    Setext,
    Preserve,
}

impl From<HeadingStyleArg> for HeadingStyle {
    fn from(value: HeadingStyleArg) -> Self {
        match value {
            HeadingStyleArg::Atx => Self::Atx,
            HeadingStyleArg::AtxClosed => Self::AtxClosed,
            HeadingStyleArg::Setext => Self::Setext,
            HeadingStyleArg::Preserve => Self::Preserve,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FenceStyleArg {
    Backtick,
    Tilde,
    Preserve,
}

impl From<FenceStyleArg> for FenceStyle {
    fn from(value: FenceStyleArg) -> Self {
        match value {
            FenceStyleArg::Backtick => Self::Backtick,
            FenceStyleArg::Tilde => Self::Tilde,
            FenceStyleArg::Preserve => Self::Preserve,
        }
    }
}

/// Arguments for `mdk lint`.
#[derive(Debug, clap::Args)]
pub struct LintArgs {
    /// Files or directories to lint; defaults to the current directory.
    /// Use `-` to read from standard input.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Apply safe autofixes
    #[arg(long)]
    pub fix: bool,

    /// Diagnostic output format
    #[arg(long, value_enum, default_value_t = LintOutput::Human)]
    pub format: LintOutput,

    /// Run only these rules (repeatable, comma-separated)
    #[arg(long, value_name = "RULE", value_delimiter = ',')]
    pub enable: Vec<String>,

    /// Never run these rules (repeatable, comma-separated)
    #[arg(long, value_name = "RULE", value_delimiter = ',')]
    pub disable: Vec<String>,

    /// Fail when warnings are reported
    #[arg(long)]
    pub warn_as_error: bool,

    /// Fail when more than this many warnings are reported
    #[arg(long, value_name = "N")]
    pub max_warnings: Option<usize>,

    /// Do not respect .gitignore, .ignore or .mdkignore files when walking
    /// directories
    #[arg(long)]
    pub no_ignore: bool,

    /// File name shown for diagnostics read from standard input
    #[arg(long, value_name = "NAME", default_value = "stdin")]
    pub stdin_filename: String,
}

/// Arguments for `mdk fmt`.
#[derive(Debug, clap::Args)]
pub struct FmtArgs {
    /// Files or directories to format; defaults to standard input.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Rewrite files in place
    #[arg(long, conflicts_with_all = ["check", "diff"])]
    pub write: bool,

    /// Exit non-zero if any file would change
    #[arg(long, conflicts_with_all = ["write", "diff"])]
    pub check: bool,

    /// Print a unified diff instead of rewriting files
    #[arg(long, conflicts_with_all = ["write", "check"])]
    pub diff: bool,

    /// Skip the rendered-HTML equivalence check
    #[arg(long)]
    pub no_verify: bool,

    /// Target line width for reflowed prose
    #[arg(long, value_name = "N")]
    pub line_width: Option<usize>,

    /// Do not reflow paragraphs
    #[arg(long)]
    pub no_wrap: bool,

    /// Preferred heading style
    #[arg(long, value_enum, value_name = "STYLE")]
    pub heading_style: Option<HeadingStyleArg>,

    /// Preferred code fence character
    #[arg(long, value_enum, value_name = "STYLE")]
    pub fence_style: Option<FenceStyleArg>,

    /// Line ending to write
    #[arg(long, value_enum, value_name = "STYLE")]
    pub line_ending: Option<LineEndingArg>,

    /// Do not respect ignore files when walking directories
    #[arg(long)]
    pub no_ignore: bool,

    /// File name used in diffs for standard input
    #[arg(long, value_name = "NAME", default_value = "stdin")]
    pub stdin_filename: String,
}

/// Arguments for `mdk render`.
#[derive(Debug, clap::Args)]
pub struct RenderArgs {
    /// Input file; omit or use `-` for standard input
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Output format
    #[arg(short = 't', long, value_enum, default_value_t = RenderTarget::Html)]
    pub to: RenderTarget,

    /// Write to a file instead of standard output
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Wrap HTML output in a complete document
    #[arg(long)]
    pub standalone: bool,

    /// Pass raw HTML through instead of escaping it
    #[arg(long)]
    pub allow_html: bool,

    /// Add ids to headings, optionally with a prefix
    #[arg(long, value_name = "PREFIX", num_args = 0..=1, default_missing_value = "")]
    pub header_ids: Option<String>,

    /// Apply smart punctuation
    #[arg(long)]
    pub smart: bool,

    /// Render soft breaks as hard breaks
    #[arg(long)]
    pub hardbreaks: bool,

    /// Terminal width for ansi and text output
    #[arg(long, value_name = "N")]
    pub width: Option<usize>,

    /// Drop raw HTML in ansi and text output
    #[arg(long)]
    pub strip_html: bool,
}

/// Arguments for `mdk rules`.
#[derive(Debug, clap::Args)]
pub struct RulesArgs {
    /// Output format
    #[arg(long, value_enum, default_value_t = RulesOutput::Text)]
    pub format: RulesOutput,
}

fn parse_dialect(value: &str) -> Result<Dialect, String> {
    value
        .parse()
        .map_err(|error: mdk_core::dialect::UnknownDialect| error.to_string())
}

/// Applies command-line overrides to formatter options.
pub fn apply_fmt_args(mut options: FormatOptions, args: &FmtArgs) -> FormatOptions {
    if let Some(width) = args.line_width {
        options.line_width = width;
    }
    options.reflow = !args.no_wrap && options.reflow;
    if let Some(style) = args.heading_style {
        options.heading_style = style.into();
    }
    if let Some(style) = args.fence_style {
        options.code_fence_style = style.into();
    }
    if let Some(ending) = args.line_ending {
        options.line_ending = ending.into();
    }
    options.verify = !args.no_verify && options.verify;
    options
}
