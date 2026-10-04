use crate::Record;

pub fn parse_record(line: &str) -> Record {
    Record { raw: line.trim().to_owned() }
}
