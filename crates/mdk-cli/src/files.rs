//! Input discovery and reading.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ignore::WalkBuilder;

/// A document to process.
pub struct Input {
    /// Document text.
    pub text: String,
    /// Name used in diagnostics and diffs.
    pub name: String,
    /// Path on disk, if the input came from a file.
    pub path: Option<PathBuf>,
}

/// Reads a file, standard input (`-` or no path) or the named path.
///
/// # Errors
///
/// Fails when the file cannot be read.
pub fn read_input(path: Option<&Path>, stdin_filename: &str) -> Result<Input> {
    match path {
        None => read_stdin(stdin_filename),
        Some(path) if path.as_os_str() == "-" => read_stdin(stdin_filename),
        Some(path) => {
            let text = fs::read_to_string(path)
                .with_context(|| format!("cannot read {}", path.display()))?;
            let name = path.display().to_string();
            Ok(Input {
                text,
                name,
                path: Some(path.to_path_buf()),
            })
        }
    }
}

fn read_stdin(name: &str) -> Result<Input> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .context("cannot read standard input")?;
    Ok(Input {
        text,
        name: name.to_string(),
        path: None,
    })
}

/// Expands paths into a sorted list of Markdown files.
///
/// Directories are walked recursively, respecting `.gitignore`, `.ignore`
/// and `.mdkignore` unless `no_ignore` is set.
///
/// # Errors
///
/// Fails when a path does not exist or the directory walk fails.
pub fn collect_files(paths: &[PathBuf], no_ignore: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut saw_stdin = false;
    for path in paths {
        if path.as_os_str() == "-" {
            saw_stdin = true;
            continue;
        }
        let metadata = fs::metadata(path)
            .with_context(|| format!("cannot access {}", path.display()))?;
        if metadata.is_dir() {
            files.extend(walk_directory(path, no_ignore)?);
        } else {
            files.push(path.clone());
        }
    }
    if saw_stdin {
        bail!("`-` cannot be combined with file paths");
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn walk_directory(directory: &Path, no_ignore: bool) -> Result<Vec<PathBuf>> {
    let mut builder = WalkBuilder::new(directory);
    builder.add_custom_ignore_filename(".mdkignore");
    builder.hidden(false);
    builder.git_ignore(!no_ignore);
    builder.git_global(!no_ignore);
    builder.git_exclude(!no_ignore);
    builder.ignore(!no_ignore);
    builder.parents(!no_ignore);

    let mut files = Vec::new();
    for entry in builder.build() {
        let entry =
            entry.with_context(|| format!("cannot walk {}", directory.display()))?;
        if entry.file_type().is_some_and(|kind| kind.is_file())
            && is_markdown(entry.path())
        {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}

/// Whether a path has a Markdown extension.
#[must_use]
pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown" | "mkd"
            )
        })
}
