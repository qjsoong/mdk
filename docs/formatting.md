# Formatting

`mdk fmt` rewrites whitespace, block markers and inline delimiters to a
canonical style. Every option lives under `[fmt]` in `mdk.toml`; the command
line can override the most common ones.

## Guarantees

The formatter is built around one rule: **formatting must not change the
document's meaning.** That is enforced in three layers.

1. **Targeted rewrites.** The formatter never parses and reprints a document
   from scratch. It edits the original lines, so anything it does not understand
   survives byte for byte: code blocks, HTML blocks, front matter, reference
   definitions, raw URLs and unusual constructs.
2. **Reflow safety.** Paragraph wrapping only replaces whitespace that is
   already there. It never splits code spans, autolinks, link destinations,
   reference labels or entities, and it never lets a reflowed line begin with
   something that would start a new block (a `>`, `#`, list marker, thematic
   break, fence or raw HTML tag).
3. **Semantic verification.** After formatting, both versions are rendered to
   HTML with the same parser options and compared with insignificant whitespace
   collapsed outside `<pre>` elements. If they differ, formatting fails with a
   diagnostic instead of returning a changed document. Disable this with
   `--no-verify` only if you have another way to check the result.

Together these also give idempotency: formatting an already formatted document
produces the same bytes.

## Options

| Option                          | Default    | Values                                    |
| ------------------------------- | ---------- | ----------------------------------------- |
| `line_width`                    | `80`       | any positive integer                      |
| `reflow`                        | `true`     | `true`, `false`                           |
| `hard_break_style`              | `spaces`   | `spaces`, `backslash`, `preserve`         |
| `trim_trailing_whitespace`      | `true`     | `true`, `false`                           |
| `max_blank_lines`               | `1`        | any positive integer                      |
| `blank_lines_around_blocks`     | `true`     | `true`, `false`                           |
| `blank_line_after_front_matter` | `true`     | `true`, `false`                           |
| `heading_style`                 | `atx`      | `atx`, `atx_closed`, `setext`, `preserve` |
| `list_bullet`                   | `preserve` | `preserve`, `dash`, `asterisk`, `plus`    |
| `list_marker_space`             | `1`        | any non-negative integer                  |
| `ordered_list_numbering`        | `preserve` | `preserve`, `sequential`, `one`           |
| `emphasis_style`                | `preserve` | `preserve`, `asterisk`, `underscore`      |
| `strong_style`                  | `preserve` | `preserve`, `asterisk`, `underscore`      |
| `code_fence_style`              | `preserve` | `preserve`, `backtick`, `tilde`           |
| `close_code_fences`             | `true`     | `true`, `false`                           |
| `thematic_break`                | `preserve` | `preserve`, `---`, `***`, `___`           |
| `align_tables`                  | `true`     | `true`, `false`                           |
| `table_pipes`                   | `preserve` | `preserve`, `always`                      |
| `final_newline`                 | `true`     | `true`, `false`                           |
| `line_ending`                   | `preserve` | `preserve`, `lf`, `crlf`                  |
| `verify`                        | `true`     | `true`, `false`                           |

Notes on the less obvious options:

- **`hard_break_style`**: a line ending in two or more spaces or a backslash is
  a hard break. `spaces` keeps two trailing spaces, `backslash` converts to `\`,
  and `preserve` keeps the original marker. Hard breaks survive reflow.
- **`blank_lines_around_blocks`** inserts a blank line between adjacent
  block-level elements, but only where it is safe: never inside a tight list,
  where a blank line would turn the list loose. Runs of blank lines are
  collapsed to `max_blank_lines` everywhere else.
- **`ordered_list_numbering`**: `sequential` renumbers items from the list's
  start value; `one` writes `1.` for every item. When the new marker changes the
  item's content column, continuation lines are re-indented; if that is not
  possible without ambiguity, the list keeps its numbers.
- **`list_bullet`** is ignored for a list that directly neighbours another
  bullet list: unifying the markers would merge the two lists and change
  tight/loose rendering.
- **`align_tables`** pads cells to consistent widths and canonicalises the
  delimiter row (`:---`, `---:`, `:---:`), respecting display width so CJK text
  lines up. Tables whose rows have the wrong cell count are left untouched so no
  content is lost.
- **`line_ending`** `preserve` keeps CRLF documents as CRLF, including inside
  code blocks.

## Examples

```toml
[fmt]
line_width = 100
heading_style = "atx_closed"
list_bullet = "dash"
ordered_list_numbering = "sequential"
emphasis_style = "asterisk"
strong_style = "asterisk"
code_fence_style = "backtick"
thematic_break = "---"
```

```console
$ mdk fmt --check --line-width 100 .
$ mdk fmt --write .
$ mdk fmt --diff docs/guide.md
```

## CLI behaviour

- With no paths, or with `-`, the formatted document is written to standard
  output.
- A single file without `--write`, `--check` or `--diff` is printed to standard
  output.
- `--check` prints the files that would change and exits 1 when there are any.
- `--diff` prints unified diffs and always exits 0.
- Verification failures are reported per file, the file is left unchanged, and
  the command exits 2 after processing the remaining files.
