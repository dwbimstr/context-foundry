//! Record helpers.

/// Split a `key=value` record into its two halves.
pub fn parse_record(input: &str) -> Option<(&str, &str)> {
    input.split_once('=')
}

/// Join two halves back into a record.
pub fn join_record(key: &str, value: &str) -> String {
    format!("{key}={value}")
}
