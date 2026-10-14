//! End-to-end tests for the `mdk` binary.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Sandbox {
    directory: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir()
            .join(format!("mdk-cli-test-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create sandbox");
        Self { directory }
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.directory.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, contents).expect("write file");
        path
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.directory.join(name)).expect("read file")
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mdk"));
        command.args(args).current_dir(&self.directory);
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("run mdk")
    }

    fn run_stdin(&self, args: &[&str], input: &str) -> Output {
        let mut child = self
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn mdk");
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("write stdin");
        child.wait_with_output().expect("wait")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

const DIRTY: &str = "# Title\n\ntext   \n#NoSpace\n\n-   item\n";
const CLEAN: &str = "# Title\n\nA clean document.\n";

#[test]
fn version_and_help() {
    let sandbox = Sandbox::new();
    let version = sandbox.run(&["--version"]);
    assert!(version.status.success());
    assert!(stdout(&version).contains("mdk"));

    let help = sandbox.run(&["--help"]);
    assert!(help.status.success());
    let text = stdout(&help);
    for command in ["lint", "fmt", "render", "rules"] {
        assert!(text.contains(command), "help mentions {command}");
    }
}

#[test]
fn lint_reports_and_exits_nonzero_for_errors() {
    let sandbox = Sandbox::new();
    sandbox.write("dirty.md", DIRTY);
    sandbox.write("clean.md", CLEAN);

    let output = sandbox.run(&["lint", "--no-ignore", "."]);
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    assert!(text.contains("MD018"), "{text}");
    assert!(text.contains("2 file(s)"), "{text}");

    let clean = sandbox.run(&["lint", "clean.md"]);
    assert!(clean.status.success());
    assert!(stdout(&clean).contains("no problems found"));
}

#[test]
fn lint_json_output_is_machine_readable() {
    let sandbox = Sandbox::new();
    sandbox.write("dirty.md", DIRTY);
    let output = sandbox.run(&["lint", "dirty.md", "--format", "json"]);
    let document: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("valid JSON");
    assert_eq!(document["version"], 1);
    let diagnostics = document["diagnostics"].as_array().expect("diagnostics");
    assert!(!diagnostics.is_empty());
    assert!(diagnostics.iter().any(|entry| entry["rule"] == "MD018"));
    assert!(document["summary"]["errors"].as_u64().unwrap_or(0) >= 1);
}

#[test]
fn lint_github_output_uses_workflow_commands() {
    let sandbox = Sandbox::new();
    sandbox.write("dirty.md", DIRTY);
    let output = sandbox.run(&["lint", "dirty.md", "--format", "github"]);
    let text = stdout(&output);
    assert!(text.contains("::error file=dirty.md"), "{text}");
    assert!(text.contains("title=MD018"), "{text}");
}

#[test]
fn lint_fix_rewrites_files() {
    let sandbox = Sandbox::new();
    sandbox.write("dirty.md", DIRTY);
    let output =
        sandbox.run(&["lint", "dirty.md", "--fix", "--disable", "MD025,MD013"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let fixed = sandbox.read("dirty.md");
    assert!(!fixed.contains("#NoSpace"));
    assert!(fixed.contains("# NoSpace"));
    assert!(!fixed.contains("text   "));
    assert!(fixed.contains("- item"));
}

#[test]
fn fmt_check_and_write() {
    let sandbox = Sandbox::new();
    sandbox.write("messy.md", "hello   world\n");
    sandbox.write("clean.md", CLEAN);

    let check = sandbox.run(&["fmt", "--check", "messy.md", "clean.md"]);
    assert_eq!(check.status.code(), Some(1));
    assert!(stdout(&check).contains("messy.md"));

    let write = sandbox.run(&["fmt", "--write", "messy.md"]);
    assert!(write.status.success());
    assert_eq!(sandbox.read("messy.md"), "hello world\n");

    let recheck = sandbox.run(&["fmt", "--check", "messy.md"]);
    assert!(recheck.status.success());
}

#[test]
fn fmt_stdin_to_stdout() {
    let sandbox = Sandbox::new();
    let output = sandbox.run_stdin(&["fmt", "-"], "hello   world\n");
    assert!(output.status.success());
    assert_eq!(stdout(&output), "hello world\n");
}

#[test]
fn fmt_diff_output() {
    let sandbox = Sandbox::new();
    sandbox.write("messy.md", "hello   world\n");
    let output = sandbox.run(&["fmt", "--diff", "messy.md"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("--- messy.md"), "{text}");
    assert!(text.contains("+++ messy.md"), "{text}");
    assert!(text.contains("-hello   world"), "{text}");
    assert!(text.contains("+hello world"), "{text}");
}

#[test]
fn render_html_and_standalone() {
    let sandbox = Sandbox::new();
    let fragment = sandbox.run_stdin(&["render", "-"], "# Hi\n");
    assert_eq!(stdout(&fragment), "<h1>Hi</h1>\n");

    let standalone = sandbox.run_stdin(&["render", "--standalone", "-"], "# Hi\n");
    let text = stdout(&standalone);
    assert!(text.starts_with("<!DOCTYPE html>"));
    assert!(text.contains("<title>Hi</title>"));
}

#[test]
fn render_ansi_and_text() {
    let sandbox = Sandbox::new();
    let ansi = sandbox.run_stdin(
        &["render", "--to", "ansi", "--color", "always", "-"],
        "**bold**\n",
    );
    assert!(stdout(&ansi).contains('\u{1b}'));

    let text = sandbox.run_stdin(&["render", "--to", "text", "-"], "**bold**\n");
    assert_eq!(stdout(&text), "bold\n");
    assert!(!stdout(&text).contains('\u{1b}'));
}

#[test]
fn rules_lists_every_rule() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&["rules", "--format", "json"]);
    assert!(output.status.success());
    let document: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("JSON");
    let rules = document["rules"].as_array().expect("rules");
    assert!(rules.len() >= 40);
    assert!(rules.iter().any(|rule| rule["id"] == "MD013"));
    assert!(rules.iter().any(|rule| rule["id"] == "MDK001"));
}

#[test]
fn configuration_file_is_honoured() {
    let sandbox = Sandbox::new();
    sandbox.write(
        "mdk.toml",
        r#"
dialect = "gfm"

[lint]
disable = ["MD018"]

[lint.rules.MD013]
line_length = 20
"#,
    );
    sandbox.write("doc.md", "#NoSpace\n\nA long line that exceeds the default limit of eighty columns by quite a lot indeed.\n");
    let output = sandbox.run(&["lint", "doc.md"]);
    let text = stdout(&output);
    assert!(!text.contains("MD018"), "MD018 disabled: {text}");
    assert!(text.contains("MD013"), "custom line length: {text}");
    assert!(text.contains("limit of 20"), "{text}");
}

#[test]
fn ignores_files_listed_in_mdkignore() {
    let sandbox = Sandbox::new();
    sandbox.write(".mdkignore", "vendor/\n");
    sandbox.write("vendor/generated.md", "# Title\n\n#NoSpace\n");
    sandbox.write("doc.md", CLEAN);
    let output = sandbox.run(&["lint", "."]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert!(!stdout(&output).contains("generated"));
}

#[test]
fn dialect_flag_changes_parsing() {
    let sandbox = Sandbox::new();
    let table = "| a |\n|---|\n| 1 |\n";
    let gfm = sandbox.run_stdin(&["render", "--dialect", "gfm", "-"], table);
    assert!(stdout(&gfm).contains("<table>"));
    let commonmark =
        sandbox.run_stdin(&["render", "--dialect", "commonmark", "-"], table);
    assert!(!stdout(&commonmark).contains("<table>"));
}

#[test]
fn missing_files_are_reported_cleanly() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&["lint", "does-not-exist.md"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("error:"), "{}", stderr(&output));
}
