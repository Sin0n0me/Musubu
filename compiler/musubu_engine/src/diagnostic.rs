use alloc::{format, string::String};
use core::fmt::Write;
use musubu_cache::Cache;
use musubu_vm::{TraceFrame, errors::VMError};

pub(super) fn render(
    cache: &Cache,
    filename: &str,
    source: &str,
    error: &VMError,
    trace: &[TraceFrame],
) -> String {
    let mut message = format!("error: VM error: {error:?}\n");
    for frame in trace {
        let name = cache
            .get_function_name(frame.function)
            .unwrap_or("<host IR>");
        let _ = writeln!(
            message,
            "  at {name} (function {}, instruction {})",
            frame.function, frame.instruction
        );
        if let Some(Some((start, end))) = cache
            .get_function(&frame.function)
            .and_then(|function| function.source_ranges.get(frame.instruction))
        {
            append_location(&mut message, filename, source, *start, *end);
        }
    }
    message
}

fn append_location(message: &mut String, filename: &str, source: &str, start: usize, end: usize) {
    let mut start = start.min(source.len());
    while !source.is_char_boundary(start) {
        start -= 1;
    }
    let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |i| start + i);
    let line = source[..start].bytes().filter(|b| *b == b'\n').count() + 1;
    let column = source[line_start..start].chars().count() + 1;
    let mut end = end.max(start).min(line_end);
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    let padding = source[line_start..start]
        .replace('\t', "    ")
        .chars()
        .count();
    let width = source[start..end]
        .replace('\t', "    ")
        .chars()
        .count()
        .max(1);
    let code = source[line_start..line_end]
        .trim_end_matches('\r')
        .replace('\t', "    ");
    let gutter = line.to_string().len();
    let _ = writeln!(
        message,
        "   --> {filename}:{line}:{column}\n{space:>gutter$} |\n{line} | {code}\n{space:>gutter$} | {pad}{caret}\n{space:>gutter$} |",
        space = "",
        pad = " ".repeat(padding),
        caret = "^".repeat(width)
    );
}

use alloc::string::ToString;
