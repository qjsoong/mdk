//! `mdk render`.

use std::fs;
use std::io::IsTerminal;
use std::process::ExitCode;

use anyhow::{Context, Result};
use mdk_core::Dialect;
use mdk_render::{TermOptions, render_html, render_terminal};

use crate::cli::{Cli, ColorChoice, RenderArgs, RenderTarget};
use crate::config::Config;
use crate::files::read_input;

/// Runs `mdk render`.
pub fn run(
    cli: &Cli,
    args: &RenderArgs,
    config: &Config,
    dialect: Dialect,
) -> Result<ExitCode> {
    let input = read_input(args.path.as_deref(), "stdin")?;
    let output = match args.to {
        RenderTarget::Html => {
            let mut options = config.render.html.clone();
            options.unsafe_html |= args.allow_html;
            options.full_document |= args.standalone;
            options.smart |= args.smart;
            options.hardbreaks |= args.hardbreaks;
            if let Some(prefix) = &args.header_ids {
                options.header_ids = Some(prefix.clone());
            }
            render_html(&input.text, dialect, &options)
        }
        target => {
            let mut options = match target {
                RenderTarget::Text => TermOptions::plain(term_width(args, config)),
                _ => config.render.term.clone(),
            };
            options.width = term_width(args, config);
            if args.strip_html {
                options.strip_html = true;
            }
            options.color = match target {
                RenderTarget::Text => false,
                _ => match cli.color {
                    ColorChoice::Always => true,
                    ColorChoice::Never => false,
                    ColorChoice::Auto => {
                        options.color && std::io::stdout().is_terminal()
                    }
                },
            };
            render_terminal(&input.text, dialect, &options)
        }
    };

    match &args.output {
        Some(path) => {
            fs::write(path, output)
                .with_context(|| format!("cannot write {}", path.display()))?;
        }
        None => print!("{output}"),
    }
    Ok(ExitCode::SUCCESS)
}

fn term_width(args: &RenderArgs, config: &Config) -> usize {
    args.width
        .or(Some(config.render.term.width).filter(|width| *width > 0))
        .unwrap_or_else(detect_width)
}

fn detect_width() -> usize {
    terminal_size::terminal_size()
        .map(|(width, _)| usize::from(width.0))
        .filter(|width| *width > 0)
        .unwrap_or(80)
}
