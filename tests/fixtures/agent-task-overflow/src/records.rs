/// Return a total only when it is representable, without wrapping or panicking.
pub fn checked_total(unit_price: u64, quantity: u64, fee: u64) -> Option<u64> {
    Some(unit_price * quantity + fee)
}
