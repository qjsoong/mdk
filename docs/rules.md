# Lint rules

`mdk lint` runs every enabled rule over a parsed document. Rule identifiers and
semantics follow [markdownlint](https://github.com/DavidAnson/markdownlint)
where a comparable rule exists; `MDK001` is specific to mdk.

Use `mdk rules` to print this table from the binary, or
`mdk rules --format json` for machine-readable metadata. Enable, disable and
configure rules in `mdk.toml`:

```toml
[lint]
disable = ["MD013"]
severity = { MD033 = "error" }

[lint.rules.MD013]
line_length = 100
```

Severity controls the exit code: `error` diagnostics fail a lint run, `warning`
and `info` do not unless `--warn-as-error` is given. Rules marked *fixable*
provide autofixes for `mdk lint --fix` when applying them cannot change the
document's meaning.

## Headings

| ID    | Name                    | Default | Fix | Description                                            |
| ----- | ----------------------- | ------- | --- | ------------------------------------------------------ |
| MD001 | heading-increment       | error   | no  | Heading levels should only increment by one level      |
| MD003 | heading-style           | warning | no  | Headings should use a consistent style                 |
| MD018 | no-missing-space-atx    | error   | yes | No space after the hash on an ATX heading              |
| MD019 | no-multiple-space-atx   | error   | yes | Multiple spaces after the hash on an ATX heading       |
| MD022 | blanks-around-headings  | error   | yes | Headings should be surrounded by blank lines           |
| MD023 | heading-start-left      | error   | yes | Headings must start at the beginning of the line       |
| MD024 | no-duplicate-heading    | warning | no  | Headings should be unique                              |
| MD025 | single-title            | error   | no  | Multiple top-level headings in the same document       |
| MD026 | no-trailing-punctuation | warning | no  | Trailing punctuation in a heading                      |
| MD041 | first-line-heading      | warning | no  | The first line in a file should be a top-level heading |

Options:

- **MD003** `style`: `consistent` (default), `atx`, `atx_closed`, `setext`.
- **MD022** `lines_above` (default `1`), `lines_below` (default `1`).
- **MD024** `siblings_only` (default `false`): only compare headings that share
  a parent heading.
- **MD025** `level` (default `1`), `front_matter_title` (default `true`): count
  a front matter `title` as the document title.
- **MD026** `punctuation` (default `.,;:!?`). Add CJK punctuation as needed, for
  example `".,;:!?。，；：！"`.
- **MD041** `level` (default `1`), `front_matter_title` (default `true`).

## Lists

| ID    | Name                | Default | Fix | Description                                               |
| ----- | ------------------- | ------- | --- | --------------------------------------------------------- |
| MD004 | ul-style            | warning | no  | Unordered list style                                      |
| MD005 | list-indent         | error   | no  | Inconsistent indentation for list items at the same level |
| MD007 | ul-indent           | error   | no  | Unordered list indentation                                |
| MD029 | ol-prefix           | warning | no  | Ordered list item prefix                                  |
| MD030 | list-marker-space   | warning | yes | Spaces after list markers                                 |
| MD032 | blanks-around-lists | error   | yes | Lists should be surrounded by blank lines                 |

Options:

- **MD004** `style`: `consistent` (default), `dash`, `asterisk`, `plus`,
  `sublist`.
- **MD007** `indent` (default `2`), `start_indented` (default `false`).
- **MD029** `style`: `one_or_ordered` (default), `one`, `ordered`, `zero`.
- **MD030** `ul_single`, `ol_single`, `ul_multi`, `ol_multi` (all default `1`).

## Whitespace

| ID    | Name                         | Default | Fix | Description                                      |
| ----- | ---------------------------- | ------- | --- | ------------------------------------------------ |
| MD009 | no-trailing-spaces           | warning | yes | Trailing spaces                                  |
| MD010 | no-hard-tabs                 | warning | yes | Hard tabs                                        |
| MD012 | no-multiple-blanks           | warning | yes | Multiple consecutive blank lines                 |
| MD013 | line-length                  | warning | no  | Line length                                      |
| MD027 | no-multiple-space-blockquote | error   | yes | Multiple spaces after a blockquote symbol        |
| MD028 | no-blanks-blockquote         | warning | no  | Blank line inside a blockquote                   |
| MD047 | file-end-newline             | error   | yes | Files should end with a single newline character |

Options:

- **MD009** `br_spaces` (default `2`): trailing spaces that count as an
  intentional hard break.
- **MD010** `code_blocks` (default `true`), `spaces_per_tab` (default `4`).
- **MD012** `maximum` (default `1`).
- **MD013** `line_length` (default `80`), `heading_line_length` (defaults to
  `line_length`), `code_blocks` (default `true`), `tables` (default `true`),
  `headings` (default `true`), `strict` (default `false`). Widths count display
  columns, so CJK text counts as two columns per character. Without `strict`,
  lines that contain no whitespace at all (for example long URLs) are exempt.

## Code blocks

| ID    | Name                 | Default | Fix | Description                                       |
| ----- | -------------------- | ------- | --- | ------------------------------------------------- |
| MD031 | blanks-around-fences | error   | yes | Fenced code blocks should be surrounded by blanks |
| MD040 | fenced-code-language | warning | no  | Fenced code blocks should have a language         |
| MD046 | code-block-style     | warning | no  | Code block style                                  |
| MD048 | code-fence-style     | warning | yes | Code fence style                                  |

Options:

- **MD031** `list_items` (default `true`).
- **MD040** `allowed_languages` (default `[]` means any non-empty language).
- **MD046** `style`: `fenced` (default), `indented`, `consistent`.
- **MD048** `style`: `consistent` (default), `backtick`, `tilde`.

## Inline content

| ID    | Name                   | Default | Fix | Description                                         |
| ----- | ---------------------- | ------- | --- | --------------------------------------------------- |
| MD033 | no-inline-html         | warning | no  | Inline HTML                                         |
| MD034 | no-bare-urls           | warning | yes | Bare URL used                                       |
| MD036 | no-emphasis-as-heading | warning | no  | Emphasis used instead of a heading                  |
| MD038 | no-space-in-code       | warning | no  | Spaces inside code span elements                    |
| MD039 | no-space-in-links      | warning | no  | Spaces inside link text                             |
| MD042 | no-empty-links         | error   | no  | Links must have a destination or an alternative     |
| MD044 | proper-names           | error   | yes | Proper names should have the correct capitalisation |
| MD045 | no-alt-text            | warning | no  | Images should have alternate text                   |
| MD049 | emphasis-style         | warning | yes | Emphasis style                                      |
| MD050 | strong-style           | warning | yes | Strong emphasis style                               |

Options:

- **MD033** `allowed_elements`: list of HTML element names to permit.
- **MD044** `names` (default `[]`), `code_blocks` (default `true`), and
  `html_elements` (default `true`).
- **MD049** and **MD050** `style`: `consistent` (default), `asterisk`,
  `underscore`.

## Links and references

| ID    | Name                             | Default | Fix | Description                                           |
| ----- | -------------------------------- | ------- | --- | ----------------------------------------------------- |
| MD051 | link-fragments                   | warning | no  | Link fragments should be valid                        |
| MD052 | reference-links-images           | error   | no  | Link and image references should be defined           |
| MD053 | link-image-reference-definitions | error   | no  | Link and image reference definitions should be needed |

Options:

- **MD052** `ignore`: labels that may be left undefined.
- **MD053** `ignored_definitions`: labels that are never reported as unused.

MD052 checks full (`[text][label]`) and collapsed (`[label][]`) references;
shortcut references (`[label]`) are never reported because they are
indistinguishable from ordinary bracketed prose. MD053 treats a definition as
used when the label appears anywhere in body text, which avoids false "unused"
reports for shortcut references.

## Tables and rules

| ID    | Name                 | Default | Fix | Description                                |
| ----- | -------------------- | ------- | --- | ------------------------------------------ |
| MD035 | hr-style             | warning | yes | Horizontal rule style                      |
| MD056 | table-column-count   | error   | no  | Table column count must match the header   |
| MD058 | blanks-around-tables | error   | yes | Tables should be surrounded by blank lines |

Options:

- **MD035** `style`: `consistent` (default), `---`, `***`, `___`. The autofix is
  skipped when changing the marker could turn a preceding paragraph into a
  setext heading.

## mdk-specific rules

| ID     | Name                       | Default | Fix | Description                                           |
| ------ | -------------------------- | ------- | --- | ----------------------------------------------------- |
| MDK001 | footnote-reference-defined | error   | no  | Footnote references should have a matching definition |

MDK001 only applies to the GFM dialect, where footnotes are enabled.
