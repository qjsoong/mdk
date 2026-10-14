# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic
Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- **mdk-render**: `render_terminal_document`, a structured terminal render with
  source mapping. Each line records its Markdown construct, block, wrap
  continuation and table cells, and each span records the source bytes it
  displays, so interactive readers can implement selection, copy and navigation
  without re-parsing the document. `render_terminal` now encodes that same
  document.

### Fixed

- **mdk-render**: consecutive blank lines inside code blocks are preserved
  instead of collapsing into one.

## 0.2.0 - 2026-10-14

### Added

- **mdk-core**: `Source::front_matter_value`, a reader for top-level scalar keys
  in YAML or TOML front matter. It replaces the ad-hoc title extraction in the
  renderer and the linter, so all three stages now agree on what a front matter
  key is.

### Fixed

- **mdk-fmt**: structural rewrites no longer overwrite each other on the same
  line (a heading, fence or table edit could be replaced by a later list edit),
  so formatting is idempotent for constructs such as `* # Title ##` and `* ~~~`;
  hard breaks require two literal spaces rather than any trailing whitespace;
  tables inside blockquotes and list items are aligned instead of being
  rewritten as quoted text; blank lines inserted between adjacent blocks keep
  blockquote markers and respect `max_blank_lines`.
- **mdk-lint**: MD048 no longer offers a fix that would let code-block content
  become the closing fence, MD049/MD050 skip intraword emphasis that `_` cannot
  delimit, MD053 recognises resolved shortcut references (no more false "unused"
  errors), MD044 checks inline and block HTML again, and unknown rule selectors
  in configuration are rejected with exit code 2.
- **mdk-render**: standalone document titles derived from headings are escaped,
  and terminal tables fit the requested width by splitting long words.
- **mdk-cli**: `-` cannot be combined with file paths, matching the formatter.
- **mdk-lint**: MD041 and MD025 recognise TOML titles written without spaces
  (`title="..."`) and no longer treat an indented `title` key as a document
  title.

## 0.1.0 - 2026-10-14

### Added

- **mdk-core**: CommonMark and GFM dialects, a source model with byte-offset and
  character-column mapping, YAML/TOML front matter detection, diagnostics with
  autofixes, and AST helpers.
- **mdk-lint**: a rule engine with 44 rules, shared document facts, per-rule
  options, severity overrides and safe autofixes applied to a fixed point.
  Output formats: human, JSON and GitHub Actions.
- **mdk-fmt**: a semantics-preserving formatter with paragraph reflow, heading
  and list normalisation, code fence and table formatting, blank line management
  and HTML-equivalence verification.
- **mdk-render**: HTML output (fragments or standalone documents), an ANSI
  terminal renderer with wrapping, box-drawing tables and footnote sections, and
  a plain-text mode.
- **mdk-cli**: the `mdk` binary with `lint`, `fmt`, `render` and `rules`
  commands, `mdk.toml` configuration discovery, ignore-file support and
  end-to-end tests.
