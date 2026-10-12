use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;

const TAB_WIDTH: usize = 4;

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn exact_span_with_surrounding_lines() {
        assert_eq!(
            render("src/main.msb", "before\nbad value\nafter", 7, 10, "invalid"),
            "error: invalid\n   --> src/main.msb:2:1\n  |\n1 | before\n2 | bad value\n  | ^^^\n3 | after\n  |\n"
        );
    }

    #[test]
    fn unicode_columns_count_characters() {
        assert_eq!(
            render("a.msb", "あx", 3, 4, "invalid"),
            "error: invalid\n   --> a.msb:1:2\n  |\n1 | あx\n  |  ^\n  |\n"
        );
    }

    #[test]
    fn tab_padding_and_crlf_are_rendered() {
        assert_eq!(
            render("a.msb", "\tx\r\nnext", 1, 2, "invalid"),
            "error: invalid\n   --> a.msb:1:2\n  |\n1 |     x\n  |     ^\n2 | next\n  |\n"
        );
    }

    #[test]
    fn multiline_span_highlights_each_intersection() {
        assert_eq!(
            render("a.msb", "abc\ndef\nghi", 1, 6, "invalid"),
            "error: invalid\n   --> a.msb:1:2\n  |\n1 | abc\n  |  ^^\n2 | def\n  | ^^\n3 | ghi\n  |\n"
        );
    }

    #[test]
    fn empty_input_still_has_a_caret() {
        assert_eq!(
            render("a.msb", "", 0, 0, "expected expression"),
            "error: expected expression\n   --> a.msb:1:1\n  |\n1 | \n  | ^\n  |\n"
        );
    }

    #[test]
    fn eof_span_is_clamped() {
        assert_eq!(
            render("a.msb", "x\n", usize::MAX, usize::MAX, "unexpected EOF"),
            "error: unexpected EOF\n   --> a.msb:2:1\n  |\n1 | x\n2 | \n  | ^\n  |\n"
        );
    }

    #[test]
    fn non_boundary_offsets_do_not_split_unicode() {
        assert_eq!(
            render("a", "あx", 1, 2, "bad"),
            render("a", "あx", 0, 0, "bad")
        );
    }
}

fn boundary(source: &str, offset: usize) -> usize {
    let mut offset = offset.min(source.len());
    while !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn expand_tabs(text: &str) -> String {
    text.replace('\t', &" ".repeat(TAB_WIDTH))
}

pub(crate) fn render(
    filename: &str,
    source: &str,
    start: usize,
    end: usize,
    message: &str,
) -> String {
    let start = boundary(source, start);
    let end = boundary(source, end).max(start);
    let lines = source.split('\n').collect::<Vec<_>>();
    // Line = number of preceding newlines + 1; column = preceding Unicode scalars + 1.
    let line_index = source[..start]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    let line_start = source[..start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let column = source[line_start..start].chars().count() + 1;
    let last_offset = if end > start {
        boundary(source, end - 1)
    } else {
        start
    };
    let last_line = source[..last_offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    let first_context = line_index.saturating_sub(1);
    let last_context = (last_line + 1).min(lines.len() - 1);
    let width = (last_context + 1).to_string().len();
    let mut output = format!(
        "error: {message}\n   --> {filename}:{}:{column}\n{:width$} |\n",
        line_index + 1,
        ""
    );
    let mut offset = 0;
    for (index, raw_line) in lines.iter().enumerate().take(last_context + 1) {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if index >= first_context {
            let _ = writeln!(output, "{:>width$} | {}", index + 1, expand_tabs(line));
            if index >= line_index && index <= last_line {
                // Per-line highlight = intersection of the source span and the line's byte range.
                let local_start = start.saturating_sub(offset).min(line.len());
                let local_end = end.saturating_sub(offset).min(line.len()).max(local_start);
                let padding = expand_tabs(&line[..local_start]).chars().count();
                let length = expand_tabs(&line[local_start..local_end])
                    .chars()
                    .count()
                    .max(1);
                let _ = writeln!(
                    output,
                    "{:width$} | {}{}",
                    "",
                    " ".repeat(padding),
                    "^".repeat(length)
                );
            }
        }
        offset += raw_line.len() + 1;
    }
    let _ = writeln!(output, "{:width$} |", "");
    output
}
