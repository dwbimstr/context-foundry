mod parser;

fn main() {
    let record = parser::parse_record("mode=local");
    println!("{record:?}");
}
