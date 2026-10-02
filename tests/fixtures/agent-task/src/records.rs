//! Pre-edit parser (copy of `examples/workspace/src/parser.rs` at the base
//! revision). The agent's task: trim whitespace on both sides of `=` and
//! reject an empty key, preserving the `Option<(&str, &str)>` interface.

pub fn parse_record(input: &str) -> Option<(&str, &str)> {
    input.split_once('=')
}
