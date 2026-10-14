//! Grid rendering for GFM tables in the terminal.

use comrak::nodes::TableAlignment;

use super::emit::{Span, Style, wrap_spans};

/// Box-drawing characters used for table borders.
pub(crate) struct TableChars {
    vertical: char,
    horizontal: char,
    cross: char,
    top_left: char,
    top_mid: char,
    top_right: char,
    mid_left: char,
    mid_right: char,
    bottom_left: char,
    bottom_mid: char,
    bottom_right: char,
}

impl TableChars {
    pub fn unicode() -> Self {
        Self {
            vertical: '│',
            horizontal: '─',
            cross: '┼',
            top_left: '┌',
            top_mid: '┬',
            top_right: '┐',
            mid_left: '├',
            mid_right: '┤',
            bottom_left: '└',
            bottom_mid: '┴',
            bottom_right: '┘',
        }
    }

    pub fn ascii() -> Self {
        Self {
            vertical: '|',
            horizontal: '-',
            cross: '+',
            top_left: '+',
            top_mid: '+',
            top_right: '+',
            mid_left: '+',
            mid_right: '+',
            bottom_left: '+',
            bottom_mid: '+',
            bottom_right: '+',
        }
    }
}

const MIN_COLUMN: usize = 3;

/// Renders a table to styled lines. Returns `None` when the table is not
/// rectangular, in which case the caller falls back to plain text.
pub(crate) fn render_table(
    rows: &[Vec<Vec<Span>>],
    alignments: &[TableAlignment],
    width: usize,
    prefix: &[Span],
    chars: &TableChars,
) -> Option<Vec<Vec<Span>>> {
    let columns = alignments.len();
    if columns == 0 || rows.is_empty() || rows.iter().any(|row| row.len() != columns) {
        return None;
    }

    let mut widths = vec![MIN_COLUMN; columns];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell_width(cell));
        }
    }

    let prefix_width: usize = prefix.iter().map(Span::width).sum();
    // Per column: "│ " + cell + " " = 3 border columns, plus the left border.
    let available = width.saturating_sub(prefix_width + columns.saturating_mul(3) + 1);
    shrink_to_fit(&mut widths, available);

    let mut wrapped: Vec<Vec<Vec<Vec<Span>>>> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut wrapped_row = Vec::with_capacity(columns);
        for (index, cell) in row.iter().enumerate() {
            let lines = wrap_spans(cell, widths[index]);
            wrapped_row.push(lines);
        }
        wrapped.push(wrapped_row);
    }

    let mut lines: Vec<Vec<Span>> = Vec::new();
    lines.push(border(
        prefix,
        chars.top_left,
        chars.top_mid,
        chars.top_right,
        &widths,
        chars,
    ));

    for (row_index, row) in wrapped.iter().enumerate() {
        let height = row.iter().map(Vec::len).max().unwrap_or(1);
        let header = row_index == 0;
        for line_index in 0..height {
            let mut line = prefix.to_vec();
            push_char(&mut line, chars.vertical);
            for (column, cell_lines) in row.iter().enumerate() {
                let empty: Vec<Span> = Vec::new();
                let cell = cell_lines.get(line_index).unwrap_or(&empty);
                let pad = widths[column].saturating_sub(cell_width(cell));
                let (left, right) = padding(alignments[column], pad);
                push_char(&mut line, ' ');
                line.extend(std::iter::repeat_n(Span::plain(" "), left));
                let mut spans = cell.clone();
                if header {
                    for span in &mut spans {
                        span.style.bold = true;
                    }
                }
                line.extend(spans);
                line.extend(std::iter::repeat_n(Span::plain(" "), right));
                push_char(&mut line, ' ');
                push_char(&mut line, chars.vertical);
            }
            lines.push(line);
        }
        if row_index == 0 && wrapped.len() > 1 {
            lines.push(border(
                prefix,
                chars.mid_left,
                chars.cross,
                chars.mid_right,
                &widths,
                chars,
            ));
        }
    }

    lines.push(border(
        prefix,
        chars.bottom_left,
        chars.bottom_mid,
        chars.bottom_right,
        &widths,
        chars,
    ));
    Some(lines)
}

fn cell_width(cell: &[Span]) -> usize {
    cell.iter().map(Span::width).sum()
}

fn shrink_to_fit(widths: &mut [usize], available: usize) {
    let mut total: usize = widths.iter().sum();
    while total > available {
        let Some((index, _)) = widths
            .iter()
            .enumerate()
            .filter(|(_, width)| **width > MIN_COLUMN)
            .max_by_key(|(_, width)| **width)
        else {
            break;
        };
        widths[index] -= 1;
        total -= 1;
    }
}

fn padding(alignment: TableAlignment, pad: usize) -> (usize, usize) {
    match alignment {
        TableAlignment::Right => (pad, 0),
        TableAlignment::Center => (pad / 2, pad - pad / 2),
        _ => (0, pad),
    }
}

fn border(
    prefix: &[Span],
    left: char,
    middle: char,
    right: char,
    widths: &[usize],
    chars: &TableChars,
) -> Vec<Span> {
    let style = Style::default().dim();
    let mut line = prefix.to_vec();
    line.push(Span::new(style, left.to_string()));
    for (index, width) in widths.iter().enumerate() {
        line.push(Span::new(
            style,
            chars.horizontal.to_string().repeat(*width + 2),
        ));
        if index + 1 < widths.len() {
            line.push(Span::new(style, middle.to_string()));
        }
    }
    line.push(Span::new(style, right.to_string()));
    line
}

fn push_char(line: &mut Vec<Span>, ch: char) {
    let style = if matches!(ch, '│' | '|') {
        Style::default().dim()
    } else {
        Style::default()
    };
    line.push(Span::new(style, ch.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_grid() {
        let rows = vec![
            vec![vec![Span::plain("a")], vec![Span::plain("b")]],
            vec![vec![Span::plain("1")], vec![Span::plain("2")]],
        ];
        let alignments = vec![TableAlignment::None, TableAlignment::None];
        let lines =
            render_table(&rows, &alignments, 40, &[], &TableChars::unicode()).unwrap();
        assert_eq!(lines.len(), 5);
        let text: String = lines[1].iter().map(|span| span.text.as_str()).collect();
        assert_eq!(text, "│ a   │ b   │");
    }

    #[test]
    fn rejects_ragged_rows() {
        let rows = vec![vec![vec![Span::plain("a")]], vec![]];
        let alignments = vec![TableAlignment::None];
        assert!(
            render_table(&rows, &alignments, 40, &[], &TableChars::ascii()).is_none()
        );
    }

    #[test]
    fn long_words_are_split_to_fit_the_width() {
        let rows = vec![
            vec![
                vec![Span::plain("aaaaaaaaaaaaaaa")],
                vec![Span::plain("bbbbbbbbbbbbbbb")],
            ],
            vec![vec![Span::plain("x")], vec![Span::plain("y")]],
        ];
        let alignments = vec![TableAlignment::None, TableAlignment::None];
        let lines =
            render_table(&rows, &alignments, 30, &[], &TableChars::ascii()).unwrap();
        for line in &lines {
            let width: usize = line.iter().map(Span::width).sum();
            assert!(width <= 30, "line too wide ({width})");
        }
    }
}
