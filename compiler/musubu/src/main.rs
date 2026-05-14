mod compiler;
use std::fs;

fn main() {
    let content = fs::read_to_string("test.msb").unwrap();

    compiler::compile(content.as_str());
}
