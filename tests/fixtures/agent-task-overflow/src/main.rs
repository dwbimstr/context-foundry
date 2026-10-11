mod records;

fn main() {
    for (unit_price, quantity, fee) in [(12, 3, 4), (0, 3, 5), (30, 0, 6)] {
        println!("{:?}", records::checked_total(unit_price, quantity, fee));
    }
}
