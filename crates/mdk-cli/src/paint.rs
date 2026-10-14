//! Minimal ANSI colouring for human-readable output.

use std::env;
use std::io::IsTerminal;

use mdk_lint::Severity;

use crate::cli::ColorChoice;

/// A small colour helper that respects `--color`, `NO_COLOR` and `TERM=dumb`.
#[derive(Debug, Clone, Copy)]
pub struct Paint {
    enabled: bool,
}

impl Paint {
    /// Creates a painter for standard output.
    #[must_use]
    pub fn stdout(choice: ColorChoice) -> Self {
        Self::new(choice, std::io::stdout().is_terminal())
    }

    fn new(choice: ColorChoice, is_terminal: bool) -> Self {
        let enabled = match choice {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                is_terminal
                    && env::var_os("NO_COLOR").is_none()
                    && env::var("TERM").map_or(true, |term| term != "dumb")
            }
        };
        Self { enabled }
    }

    fn paint(self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_string()
        }
    }

    /// Bold text.
    #[must_use]
    pub fn bold(self, text: &str) -> String {
        self.paint("1", text)
    }

    /// Dim text.
    #[must_use]
    pub fn dim(self, text: &str) -> String {
        self.paint("2", text)
    }

    /// Green text.
    #[must_use]
    pub fn green(self, text: &str) -> String {
        self.paint("32", text)
    }

    /// Red text.
    #[must_use]
    pub fn red(self, text: &str) -> String {
        self.paint("31", text)
    }

    /// Yellow text.
    #[must_use]
    pub fn yellow(self, text: &str) -> String {
        self.paint("33", text)
    }

    /// Cyan text.
    #[must_use]
    pub fn cyan(self, text: &str) -> String {
        self.paint("36", text)
    }

    /// Colours a severity name.
    #[must_use]
    pub fn severity(self, severity: Severity) -> String {
        match severity {
            Severity::Error => self.red("error"),
            Severity::Warning => self.yellow("warning"),
            Severity::Info => self.cyan("info"),
        }
    }
}
