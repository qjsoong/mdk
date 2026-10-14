//! `mdk lint`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use mdk_core::Dialect;
use mdk_lint::{Diagnostic, LintConfig, Severity, apply_fixes, lint_text};
use serde_json::json;

use crate::cli::{Cli, LintArgs, LintOutput};
use crate::config::Config;
use crate::files::{Input, collect_files, read_input};
use crate::paint::Paint;

struct FileReport {
    input: Input,
    diagnostics: Vec<Diagnostic>,
    fixed: usize,
}

#[derive(Default)]
struct Summary {
    files: usize,
    files_with_issues: usize,
    errors: usize,
    warnings: usize,
    infos: usize,
    fixable: usize,
    fixed: usize,
}

impl Summary {
    fn collect(reports: &[FileReport]) -> Self {
        let mut summary = Self {
            files: reports.len(),
            ..Self::default()
        };
        for report in reports {
            if !report.diagnostics.is_empty() {
                summary.files_with_issues += 1;
            }
            summary.fixed += report.fixed;
            for diagnostic in &report.diagnostics {
                match diagnostic.severity {
                    Severity::Error => summary.errors += 1,
                    Severity::Warning => summary.warnings += 1,
                    Severity::Info => summary.infos += 1,
                }
                if diagnostic.is_fixable() {
                    summary.fixable += 1;
                }
            }
        }
        summary
    }
}

/// Runs `mdk lint`.
pub fn run(
    cli: &Cli,
    args: &LintArgs,
    config: &Config,
    dialect: Dialect,
) -> Result<ExitCode> {
    let lint_config = effective_config(&config.lint, args);
    let paint = Paint::stdout(cli.color);
    let mut reports = Vec::new();

    if args.paths.iter().any(|path| path.as_os_str() == "-") {
        if args.paths.len() > 1 {
            bail!("`-` cannot be combined with file paths");
        }
        let input = read_input(Some(Path::new("-")), &args.stdin_filename)?;
        reports.push(process(input, &lint_config, dialect, args)?);
    } else {
        let paths = if args.paths.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            args.paths.clone()
        };
        let files = collect_files(&paths, args.no_ignore)?;
        if files.is_empty() && !cli.quiet {
            eprintln!("warning: no Markdown files found");
        }
        for file in &files {
            let input = read_input(Some(file), &args.stdin_filename)?;
            reports.push(process(input, &lint_config, dialect, args)?);
        }
    }

    let summary = Summary::collect(&reports);
    match args.format {
        LintOutput::Human => print_human(&reports, &summary, &paint, cli.quiet),
        LintOutput::Json => print_json(&reports, &summary)?,
        LintOutput::Github => print_github(&reports),
    }

    let failed = summary.errors > 0
        || (args.warn_as_error && summary.warnings > 0)
        || args
            .max_warnings
            .is_some_and(|maximum| summary.warnings > maximum);
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn effective_config(base: &LintConfig, args: &LintArgs) -> LintConfig {
    let mut config = base.clone();
    if !args.enable.is_empty() {
        config.enable = args.enable.clone();
    }
    for rule in &args.disable {
        if !config
            .disable
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(rule))
        {
            config.disable.push(rule.clone());
        }
    }
    config
}

fn process(
    input: Input,
    config: &LintConfig,
    dialect: Dialect,
    args: &LintArgs,
) -> Result<FileReport> {
    let mut diagnostics = lint_text(&input.text, dialect, config);
    let mut fixed = 0;
    let mut text = input.text.clone();

    if args.fix {
        let outcome = apply_fixes(&input.text, dialect, config);
        fixed = outcome.applied;
        let changed = outcome.changed();
        diagnostics = outcome.diagnostics;
        text = outcome.text;
        if changed {
            match &input.path {
                Some(path) => {
                    fs::write(path, &text)
                        .with_context(|| format!("cannot write {}", path.display()))?;
                }
                None => print!("{text}"),
            }
        }
    }

    Ok(FileReport {
        input: Input {
            text,
            name: input.name,
            path: input.path,
        },
        diagnostics,
        fixed,
    })
}

fn print_human(reports: &[FileReport], summary: &Summary, paint: &Paint, quiet: bool) {
    for report in reports {
        for diagnostic in &report.diagnostics {
            let position = format!(
                "{}:{}:{}",
                report.input.name,
                diagnostic.span.start.line,
                diagnostic.span.start.column
            );
            println!(
                "{}: {}[{}]: {}",
                paint.dim(&position),
                paint.severity(diagnostic.severity),
                paint.bold(diagnostic.rule),
                diagnostic.message
            );
        }
    }
    if quiet {
        return;
    }
    let problems = summary.errors + summary.warnings + summary.infos;
    if problems == 0 {
        println!(
            "{}",
            paint.green(&format!(
                "checked {} file(s), no problems found",
                summary.files
            ))
        );
        return;
    }
    let mut parts = vec![format!("{} error(s)", summary.errors)];
    if summary.warnings > 0 {
        parts.push(format!("{} warning(s)", summary.warnings));
    }
    if summary.infos > 0 {
        parts.push(format!("{} info(s)", summary.infos));
    }
    if summary.fixed > 0 {
        parts.push(format!("{} fixed", summary.fixed));
    } else if summary.fixable > 0 {
        parts.push(format!("{} fixable", summary.fixable));
    }
    println!(
        "{}",
        paint.bold(&format!(
            "{} problem(s) in {} of {} file(s): {}",
            problems,
            summary.files_with_issues,
            summary.files,
            parts.join(", ")
        ))
    );
}

fn print_json(reports: &[FileReport], summary: &Summary) -> Result<()> {
    let diagnostics: Vec<_> = reports
        .iter()
        .flat_map(|report| {
            report.diagnostics.iter().map(|diagnostic| {
                json!({
                    "file": report.input.name,
                    "rule": diagnostic.rule,
                    "severity": diagnostic.severity.as_str(),
                    "message": diagnostic.message,
                    "line": diagnostic.span.start.line,
                    "column": diagnostic.span.start.column,
                    "end_line": diagnostic.span.end.line,
                    "end_column": diagnostic.span.end.column,
                    "fixable": diagnostic.is_fixable(),
                })
            })
        })
        .collect();
    let document = json!({
        "version": 1,
        "diagnostics": diagnostics,
        "summary": {
            "files": summary.files,
            "files_with_issues": summary.files_with_issues,
            "errors": summary.errors,
            "warnings": summary.warnings,
            "infos": summary.infos,
            "fixable": summary.fixable,
            "fixed": summary.fixed,
        },
    });
    println!("{}", serde_json::to_string_pretty(&document)?);
    Ok(())
}

fn print_github(reports: &[FileReport]) {
    for report in reports {
        for diagnostic in &report.diagnostics {
            let level = match diagnostic.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "notice",
            };
            println!(
                "::{level} file={file},line={line},col={column},endLine={end_line},endColumn={end_column},title={rule}::{message}",
                file = escape_property(&report.input.name),
                line = diagnostic.span.start.line,
                column = diagnostic.span.start.column,
                end_line = diagnostic.span.end.line,
                end_column = diagnostic.span.end.column,
                rule = diagnostic.rule,
                message = escape_data(&diagnostic.message),
            );
        }
    }
}

fn escape_property(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(':', "%3A")
        .replace(',', "%2C")
}

fn escape_data(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}
