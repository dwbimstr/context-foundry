pub fn parse_record(input: &str) -> Option<(&str, &str)> {
    input.split_once('=')
}
