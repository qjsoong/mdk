# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic
Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

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
