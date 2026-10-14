# Contributing to mdk

Thanks for taking the time to contribute. This document covers the development
workflow, the checks CI runs, and what a good change looks like.

## Getting started

```bash
git clone https://github.com/qjsoong/mdk
cd mdk
cargo build
cargo test --workspace
```

The workspace requires Rust 1.89 or newer (pinned in `rust-toolchain.toml`).
`rustfmt` and `clippy` are part of the toolchain.

## Repository layout

| Crate               | Responsibility                                   |
| ------------------- | ------------------------------------------------ |
| `crates/mdk-core`   | dialects, source model, diagnostics, AST helpers |
| `crates/mdk-lint`   | the rule engine and all lint rules               |
| `crates/mdk-fmt`    | the formatter                                    |
| `crates/mdk-render` | HTML, ANSI and plain-text renderers              |
| `crates/mdk-cli`    | the `mdk` binary and its integration tests       |

Parsing is delegated to [`comrak`](https://crates.io/crates/comrak). Every stage
shares the same parser options, so behaviour stays consistent across linting,
formatting and rendering.

## Before opening a pull request

Run the same checks as CI:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The workspace does not allow `unsafe` code, and libraries should avoid
`unwrap`/`expect` outside tests.

## Adding a lint rule

1. Add a module under `crates/mdk-lint/src/rules/` (or extend an existing one)
   with a `RuleMeta` describing the id, name, default severity and fixability.
2. Implement `Rule::check`, using `LintContext` facts and helpers. Rules should
   report precise spans and only attach a `Fix` when applying it cannot change
   the document's meaning.
3. Register the rule in `RuleMeta` order in `src/rules/rule.rs`.
4. Add unit tests that enable only the new rule, plus a fixture in the formatter
   corpus if the formatter should handle the construct.
5. Document the rule and its options in `docs/rules.md`. An integration test
   checks that every registered rule appears in that file.

## Changing the formatter

The formatter must stay semantics preserving and idempotent. Add fixtures to
`crates/mdk-fmt/tests/corpus/` for any new construct you touch; the corpus test
formats each fixture with three option profiles and both dialects, and asserts
that:

- formatting succeeds (including HTML verification),
- formatting twice produces identical output,
- front matter and code block content are unchanged.

If the formatter legitimately cannot handle a construct without changing
meaning, it should leave it untouched rather than return an error.

## Testing the CLI

End-to-end tests live in `crates/mdk-cli/tests/cli.rs` and run the built binary
in a temporary directory. Cover exit codes and output formats, not just
successful parsing.

## Commit conventions

Use [Conventional Commits](https://www.conventionalcommits.org/): `feat`, `fix`,
`docs`, `refactor`, `test`, `build`, `ci`, `perf` or `chore`, optionally with a
scope such as `fix(fmt):`. Keep commits focused: one logical change with its
tests and documentation.

## Code of conduct

Participation in this project is governed by the [Code of
Conduct](CODE_OF_CONDUCT.md). Please report unacceptable behaviour privately as
described there.
