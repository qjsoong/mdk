# mdk — Markdown Kit

[![CI](https://github.com/qjsoong/mdk/actions/workflows/ci.yml/badge.svg)](https://github.com/qjsoong/mdk/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](https://www.rust-lang.org)

**mdk** is a fast, spec-aware Markdown toolkit written in Rust. It lints,
formats and converts [CommonMark](https://commonmark.org/) and [GitHub Flavored
Markdown](https://github.github.com/gfm/) documents, and it treats one property
as non-negotiable: formatting never changes what a document means.

```console
$ mdk lint docs/
docs/guide.md:12:81: warning[MD013]: line length 92, exceeds limit of 80
docs/guide.md:30:1: error[MD031]: fenced code block should be preceded by a blank line
2 problem(s) in 1 of 2 file(s): 1 error(s), 1 warning(s), 2 fixable

$ mdk fmt --write docs/
formatted 1 of 2 file(s)

$ mdk render --to ansi README.md
# …
```

## Features

- **Lint** with 44 rules modelled on
  [markdownlint](https://github.com/DavidAnson/markdownlint), precise
  character-based spans, JSON and GitHub Actions output, and safe autofixes.
- **Format** paragraphs, headings, lists, fences, tables and blank lines to a
  canonical style. The formatter only rewrites whitespace and marker characters,
  never code blocks, HTML blocks or front matter, and verifies by default that
  the rendered HTML is unchanged.
- **Convert** to HTML (for blogs and static sites), ANSI (for reading in a
  terminal, with box-drawing tables and footnote sections) or plain text.
- **Two dialects**: CommonMark and GFM (tables, task lists, strikethrough,
  autolinks, footnotes).
- **One parser** for every stage, so lint findings, formatting decisions and
  rendered output always agree.

## Installation

Until binaries are published, install from source:

```bash
cargo install --git https://github.com/qjsoong/mdk mdk-cli
```

Or build the workspace:

```bash
git clone https://github.com/qjsoong/mdk
cd mdk
cargo build --release        # target/release/mdk
```

The minimum supported Rust version is 1.89.

## Usage

```text
mdk <COMMAND>

Commands:
  lint    Lint Markdown documents and report problems
  fmt     Format Markdown documents            [aliases: format]
  render  Convert Markdown to HTML, ANSI or plain text  [aliases: translate]
  rules   List the available lint rules
```

Global options apply to every command: `--dialect commonmark|gfm`,
`--config FILE`, `--color auto|always|never`, `--quiet` and `--verbose`.

### Linting

```bash
mdk lint                       # every Markdown file below the CWD
mdk lint docs/ README.md       # specific paths
mdk lint - < draft.md          # standard input
mdk lint --fix                 # apply safe fixes and report what remains
mdk lint --enable MD013,MD032  # only these rules
mdk lint --disable line-length
mdk lint --format json         # machine-readable output
mdk lint --format github       # GitHub Actions annotations
mdk rules                      # list all rules and their severities
```

Diagnostics use `file:line:column` positions. Exit codes:

| Code | Meaning                                                                     |
| ---- | --------------------------------------------------------------------------- |
| 0    | No problems (or only warnings without `--warn-as-error`)                    |
| 1    | Errors found, `--warn-as-error` was given, or `--max-warnings` was exceeded |
| 2    | Usage, configuration or I/O error; formatting verification failed           |

`--fix` applies every safe autofix repeatedly until the document stabilises.
Fixes that would shift continuation lines (for example marker padding on a
multi-line list item) are reported without a fix instead of corrupting the
document.

### Formatting

```bash
mdk fmt --check .              # list files that would change; exit 1 if any
mdk fmt --write .              # rewrite files in place
mdk fmt --diff docs/guide.md   # show a unified diff
mdk fmt draft.md               # print the formatted document
mdk fmt - --line-width 100     # format stdin with a wider margin
```

Formatting is semantics preserving by design:

- paragraphs are reflowed only at existing whitespace, never inside code spans,
  link destinations, reference labels, autolinks or entities;
- words that would start a new block (list markers, blockquote markers,
  headings, fences, raw HTML) never begin a reflowed line;
- code blocks, HTML blocks and front matter are treated as verbatim;
- bullet normalisation is skipped when it would merge adjacent lists and change
  tight/loose rendering;
- before returning, the result is rendered to HTML and compared with the
  original; if anything differs, formatting fails and the file is left
  unchanged.

See [docs/formatting.md](docs/formatting.md) for every option, and use
`--no-verify` to skip the HTML check when speed matters more.

### Rendering

```bash
mdk render README.md                                  # HTML fragment
mdk render --standalone --output site/readme.html README.md
mdk render --to ansi README.md                        # coloured terminal view
mdk render --to text --width 100 README.md            # plain text
mdk render --header-ids user-content- README.md       # anchor ids
mdk render --to html --allow-html docs/post.md        # trust raw HTML
```

The ANSI renderer colours headings, links, code and tables, wraps prose to the
terminal width (or `--width`), draws GFM tables with box characters, renders
task lists and footnotes, and degrades to ASCII when colour is off or
`--to text` is used.

### Configuration

`mdk` reads `mdk.toml` (or `.mdk.toml`) from the working directory upwards;
`--config FILE` overrides discovery, and command-line flags always win.

```toml
dialect = "gfm"

[lint]
disable = ["MD013"]
severity = { MD033 = "error" }

[lint.rules.MD013]
line_length = 100
tables = false

[lint.rules.MD044]
names = ["JavaScript", "CommonMark"]

[fmt]
line_width = 80
heading_style = "atx"
list_bullet = "dash"
ordered_list_numbering = "sequential"
emphasis_style = "asterisk"
strong_style = "asterisk"
code_fence_style = "backtick"
thematic_break = "---"
line_ending = "lf"

[render.html]
unsafe_html = false
header_ids = ""

[render.term]
width = 100
```

Unknown top-level, `[lint]`, `[fmt]` and `[render]` keys are rejected so typos
surface immediately; rule selectors in `[lint]` are validated against the rule
registry. Unknown keys inside `[lint.rules.*]` tables are ignored.

## Rule reference

All 44 rules with their options are documented in
[docs/rules.md](docs/rules.md). Rules are grouped loosely after markdownlint,
and `MDK001` is an mdk-specific check for undefined footnote references.

## Dialects

| Dialect      | Extensions enabled                                      |
| ------------ | ------------------------------------------------------- |
| `commonmark` | none                                                    |
| `gfm`        | tables, strikethrough, autolinks, task lists, footnotes |

`gfm` is the default. A document's dialect affects parsing, linting, formatting
and rendering consistently; for example, in `commonmark` a GFM table is just a
paragraph and a thematic break.

## Project layout

```text
crates/mdk-core     dialects, source model, diagnostics, AST helpers
crates/mdk-lint     the rule engine and every lint rule
crates/mdk-fmt      the formatter
crates/mdk-render   HTML, ANSI and plain-text renderers
crates/mdk-cli      the `mdk` binary
```

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the
development workflow, the checks CI runs and how to add a lint rule.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
