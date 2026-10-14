//! `mdk fmt`.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use mdk_core::Dialect;
use mdk_fmt::format_text;

use crate::cli::{Cli, FmtArgs, apply_fmt_args};
use crate::config::Config;
use crate::files::{collect_files, read_input};
use crate::paint::Paint;

/// Runs `mdk fmt`.
pub fn run(
    cli: &Cli,
    args: &FmtArgs,
    config: &Config,
    dialect: Dialect,
) -> Result<ExitCode> {
    let options = apply_fmt_args(config.fmt.clone(), args);
    let paint = Paint::stdout(cli.color);

    if args.paths.is_empty() || args.paths.iter().any(|path| path.as_os_str() == "-") {
        if args.paths.len() > 1 {
            bail!("`-` cannot be combined with file paths");
        }
        let input = read_input(Some(Path::new("-")), &args.stdin_filename)?;
        let result = format_text(&input.text, dialect, &options)
            .with_context(|| format!("cannot format {}", input.name))?;
        if args.check {
            return Ok(if result.changed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            });
        }
        if args.diff {
            print_diff(&paint, &input.name, &input.text, &result.output);
            return Ok(ExitCode::SUCCESS);
        }
        print!("{}", result.output);
        return Ok(ExitCode::SUCCESS);
    }

    let files = collect_files(&args.paths, args.no_ignore)?;
    if files.is_empty() {
        if !cli.quiet {
            eprintln!("warning: no Markdown files found");
        }
        return Ok(ExitCode::SUCCESS);
    }

    if files.len() == 1 && !args.write && !args.check && !args.diff {
        let input = read_input(Some(&files[0]), &args.stdin_filename)?;
        let result = format_text(&input.text, dialect, &options)
            .with_context(|| format!("cannot format {}", input.name))?;
        print!("{}", result.output);
        return Ok(ExitCode::SUCCESS);
    }

    if !args.write && !args.check && !args.diff {
        bail!("specify --write, --check or --diff when formatting multiple files");
    }

    let mut changed = Vec::new();
    let mut failures = 0usize;
    for file in &files {
        let input = read_input(Some(file), &args.stdin_filename)?;
        match format_text(&input.text, dialect, &options) {
            Ok(result) => {
                if !result.changed {
                    continue;
                }
                changed.push(input.name.clone());
                if args.check {
                    println!("{}", paint.yellow(&input.name));
                }
                if args.write {
                    fs::write(file, &result.output)
                        .with_context(|| format!("cannot write {}", file.display()))?;
                    if cli.verbose > 0 {
                        eprintln!("formatted {}", input.name);
                    }
                }
                if args.diff {
                    print_diff(&paint, &input.name, &input.text, &result.output);
                }
            }
            Err(error) => {
                failures += 1;
                eprintln!("error: {}: {error}", input.name);
            }
        }
    }

    if !cli.quiet {
        if args.write {
            println!(
                "{}",
                paint.green(&format!(
                    "formatted {} of {} file(s)",
                    changed.len(),
                    files.len()
                ))
            );
        } else if args.check {
            if changed.is_empty() {
                println!(
                    "{}",
                    paint.green(&format!("{} file(s) already formatted", files.len()))
                );
            } else {
                println!(
                    "{}",
                    paint.yellow(&format!(
                        "{} of {} file(s) would be reformatted",
                        changed.len(),
                        files.len()
                    ))
                );
            }
        }
    }

    if failures > 0 {
        return Ok(ExitCode::from(2));
    }
    if args.check && !changed.is_empty() {
        return Ok(ExitCode::from(1));
    }
    Ok(ExitCode::SUCCESS)
}

fn print_diff(paint: &Paint, name: &str, before: &str, after: &str) {
    let diff = similar::TextDiff::from_lines(before, after);
    let text = diff
        .unified_diff()
        .context_radius(3)
        .header(name, name)
        .to_string();
    for line in text.lines() {
        if line.starts_with("@@") {
            println!("{}", paint.cyan(line));
        } else if line.starts_with('+') && !line.starts_with("+++") {
            println!("{}", paint.green(line));
        } else if line.starts_with('-') && !line.starts_with("---") {
            println!("{}", paint.red(line));
        } else {
            println!("{line}");
        }
    }
}
