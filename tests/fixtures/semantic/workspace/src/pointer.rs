pub fn indirect(line: &str) -> crate::Record {
    let parse: fn(&str) -> crate::Record = crate::a::parse_record;
    parse(line)
}
