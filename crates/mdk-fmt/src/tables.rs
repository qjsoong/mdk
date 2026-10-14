//! GFM table alignment.

use comrak::nodes::{NodeTable, TableAlignment};
use mdk_core::text::display_width;

use crate::options::{FormatOptions, TablePipes};

/// A parsed table.
pub(crate) struct Table {
    pub indent: String,
    pub leading_pipe: bool,
    pub trailing_pipe: bool,
    /// Raw cell text, trimmed. Row 0 is the header and row 1 the delimiter.
    pub rows: Vec<Vec<String>>,
    pub alignments: Vec<TableAlignment>,
}

/// Parses the source lines of a table node.
///
/// Returns `None` when a row's cell count does not match the header, in which
/// case the table is left untouched so no content is lost.
pub(crate) fn parse(lines: &[&str], table: &NodeTable) -> Option<Table> {
    let first = lines.first()?;
    let indent_len = first.len() - first.trim_start_matches(' ').len();
    if indent_len > 3 {
        return None;
    }
    let indent = first[..indent_len].to_string();
    let mut rows = Vec::with_capacity(lines.len());
    let mut leading_pipe = false;
    let mut trailing_pipe = false;
    for (index, line) in lines.iter().enumerate() {
        let (cells, leading, trailing) = split_row(line)?;
        if index == 0 {
            leading_pipe = leading;
            trailing_pipe = trailing;
        }
        rows.push(cells);
    }
    let columns = rows.first()?.len();
    if columns == 0 || rows.iter().any(|row| row.len() != columns) {
        return None;
    }
    if rows.len() < 2 || table.alignments.len() != columns {
        return None;
    }
    Some(Table {
        indent,
        leading_pipe,
        trailing_pipe,
        rows,
        alignments: table.alignments.clone(),
    })
}

/// Renders the table with padded cells.
pub(crate) fn render(table: &Table, options: &FormatOptions) -> Vec<String> {
    let columns = table.alignments.len();
    let mut widths = vec![3usize; columns];
    for (row_index, row) in table.rows.iter().enumerate() {
        if row_index == 1 {
            continue;
        }
        for (column, cell) in row.iter().enumerate() {
            widths[column] = widths[column].max(display_width(cell));
        }
    }

    let use_pipes = match options.table_pipes {
        TablePipes::Always => (true, true),
        TablePipes::Preserve => (table.leading_pipe, table.trailing_pipe),
    };

    let mut out = Vec::with_capacity(table.rows.len());
    for (row_index, row) in table.rows.iter().enumerate() {
        let mut cells = Vec::with_capacity(columns);
        for (column, cell) in row.iter().enumerate() {
            if row_index == 1 {
                cells.push(delimiter(table.alignments[column], widths[column]));
            } else {
                cells.push(pad(cell, widths[column], table.alignments[column]));
            }
        }
        let mut line = table.indent.clone();
        if use_pipes.0 {
            line.push_str("| ");
        }
        line.push_str(&cells.join(" | "));
        if use_pipes.1 {
            line.push_str(" |");
        }
        out.push(line);
    }
    out
}

fn pad(cell: &str, width: usize, alignment: TableAlignment) -> String {
    let current = display_width(cell);
    let missing = width.saturating_sub(current);
    match alignment {
        TableAlignment::Right => format!("{}{cell}", " ".repeat(missing)),
        TableAlignment::Center => {
            let left = missing / 2;
            let right = missing - left;
            format!("{}{cell}{}", " ".repeat(left), " ".repeat(right))
        }
        _ => format!("{cell}{}", " ".repeat(missing)),
    }
}

fn delimiter(alignment: TableAlignment, width: usize) -> String {
    let width = width.max(3);
    match alignment {
        TableAlignment::Left => format!(":{}", "-".repeat(width - 1)),
        TableAlignment::Right => format!("{}:", "-".repeat(width - 1)),
        TableAlignment::Center => format!(":{}:", "-".repeat(width - 2)),
        TableAlignment::None => "-".repeat(width),
    }
}

/// Whether the row ends with a structural trailing pipe rather than an
/// escaped `\|` written as cell content.
fn ends_with_unescaped_pipe(trimmed: &str) -> bool {
    let Some(prefix) = trimmed.strip_suffix('|') else {
        return false;
    };
    let backslashes = prefix.chars().rev().take_while(|ch| *ch == '\\').count();
    backslashes % 2 == 0
}

/// Splits a table row into trimmed cells, respecting escaped pipes and pipes
/// inside code spans.
///
/// Returns the cells and whether the row had leading and trailing pipes.
pub(crate) fn split_row(line: &str) -> Option<(Vec<String>, bool, bool)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let leading = trimmed.starts_with('|') && trimmed.len() > 1;
    let trailing = ends_with_unescaped_pipe(trimmed) && trimmed.len() > 1;
    let body = if leading { &trimmed[1..] } else { trimmed };
    let body = if trailing && !body.is_empty() {
        &body[..body.len() - 1]
    } else {
        body
    };

    let mut cells = Vec::new();
    let mut current = String::new();
    let mut backticks = 0usize;
    let mut escaped = false;
    for ch in body.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => {
                current.push(ch);
                escaped = true;
            }
            '`' => {
                backticks += 1;
                current.push(ch);
            }
            '|' if backticks.is_multiple_of(2) => {
                cells.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    cells.push(current.trim().to_string());
    Some((cells, leading, trailing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_rows_into_trimmed_cells() {
        let (cells, leading, trailing) = split_row("| a | b |").unwrap();
        assert_eq!(cells, vec!["a", "b"]);
        assert!(leading && trailing);
        let (cells, leading, trailing) = split_row("a | b").unwrap();
        assert_eq!(cells, vec!["a", "b"]);
        assert!(!leading && !trailing);
    }

    #[test]
    fn respects_escaped_pipes_and_code() {
        let (cells, _, _) = split_row(r"| a \| b | `c | d` |").unwrap();
        assert_eq!(cells, vec![r"a \| b", "`c | d`"]);
    }

    fn table_with(widths: &[usize]) -> Table {
        let row = |n: usize| -> Vec<String> {
            widths
                .iter()
                .enumerate()
                .map(|(i, _)| format!("{}{i}", "x".repeat(n)))
                .collect()
        };
        Table {
            indent: String::new(),
            leading_pipe: true,
            trailing_pipe: true,
            rows: vec![row(1), row(1), row(3)],
            alignments: vec![TableAlignment::None; widths.len()],
        }
    }

    #[test]
    fn pads_cells_to_column_widths() {
        let table = table_with(&[1, 1]);
        let lines = render(&table, &FormatOptions::default());
        assert_eq!(lines[0], "| x0   | x1   |");
        assert_eq!(lines[2], "| xxx0 | xxx1 |");
    }

    #[test]
    fn renders_alignment_markers() {
        let mut table = table_with(&[1]);
        table.alignments = vec![TableAlignment::Right];
        let lines = render(&table, &FormatOptions::default());
        assert_eq!(lines[1], "| ---: |");
    }
}
