//! Small helpers used by the widget factory.

/// Fold a byte slice into a running checksum.
pub fn fold_checksum(data: &[u8]) -> u32 {
    let mut state: u32 = 0x811c_9dc5;
    for byte in data {
        state ^= u32::from(*byte);
        state = state.wrapping_mul(0x0100_0193);
    }
    state
}

/// Mix two channel values into one, weighting the first by three quarters.
pub fn mix_channels(first: u8, second: u8) -> u8 {
    let weighted = u16::from(first) * 3 + u16::from(second);
    (weighted / 4) as u8
}

/// Milliseconds to pause before the next attempt after `failures` failures:
/// the pause doubles each time and never exceeds thirty seconds.
pub fn pause_before_next_attempt(failures: u32) -> u64 {
    let doubled = 100u64.saturating_mul(1u64 << failures.min(20));
    doubled.min(30_000)
}

/// Pad a serial number with leading zeros to eight digits.
pub fn pad_serial(serial: u32) -> String {
    format!("{serial:08}")
}

/// Uppercase the first character of a label.
pub fn capitalize_label(label: &str) -> String {
    let mut chars = label.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Clamp a percentage into the closed range zero to one hundred.
pub fn clamp_percent(value: i32) -> u8 {
    value.clamp(0, 100) as u8
}

/// Count how many whitespace separated words a line holds.
pub fn count_words(line: &str) -> usize {
    line.split_whitespace().count()
}

/// Return the larger of two serial numbers.
pub fn newest_serial(left: u32, right: u32) -> u32 {
    left.max(right)
}

/// Swap the two bytes of a sixteen bit value.
pub fn swap_halves(value: u16) -> u16 {
    value.rotate_left(8)
}

/// Whether a calendar year has a February twenty-ninth.
pub fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Convert degrees Celsius to degrees Fahrenheit.
pub fn celsius_to_fahrenheit(celsius: f32) -> f32 {
    celsius * 9.0 / 5.0 + 32.0
}

/// Reverse the order of the words in a line, separated by single spaces.
pub fn reverse_words(line: &str) -> String {
    let mut words: Vec<&str> = line.split_whitespace().collect();
    words.reverse();
    words.join(" ")
}

/// Return the middle value of three numbers.
pub fn median_of_three(a: i32, b: i32, c: i32) -> i32 {
    a.max(b).min(a.min(b).max(c))
}

/// Return the mean of a slice, or zero for an empty slice.
pub fn mean_of(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

/// Remove trailing zero characters from a decimal string.
pub fn trim_trailing_zeros(text: &str) -> &str {
    text.trim_end_matches('0')
}

/// Return one when a byte holds an odd number of set bits, otherwise zero.
pub fn parity_bit(byte: u8) -> u8 {
    (byte.count_ones() & 1) as u8
}

/// Sum the decimal digits of a number.
pub fn sum_digits(mut value: u64) -> u64 {
    let mut total = 0;
    while value > 0 {
        total += value % 10;
        value /= 10;
    }
    total
}

/// Return the point halfway between two coordinates without overflowing.
pub fn midpoint(left: i64, right: i64) -> i64 {
    left + (right - left) / 2
}

/// Convert a count of seconds into whole minutes, rounding down.
pub fn whole_minutes(seconds: u64) -> u64 {
    seconds / 60
}

/// Return the smallest power of two that is at least the given value.
pub fn next_power_of_two(value: u32) -> u32 {
    value.max(1).next_power_of_two()
}

/// Return the absolute difference between two unsigned numbers.
pub fn distance_between(left: u32, right: u32) -> u32 {
    left.abs_diff(right)
}

/// Join a list of words with commas and a final "and".
pub fn join_with_and(words: &[&str]) -> String {
    match words {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [first, last] => format!("{first} and {last}"),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// Return how many bytes a string needs once padded to a multiple of four.
pub fn padded_length(text: &str) -> usize {
    text.len().div_ceil(4) * 4
}

/// Return the position of the first uppercase letter in a string, if any.
pub fn first_capital(text: &str) -> Option<usize> {
    text.char_indices()
        .find(|(_, c)| c.is_uppercase())
        .map(|(at, _)| at)
}

/// Turn a snake_case name into kebab-case.
pub fn kebab_name(name: &str) -> String {
    name.replace('_', "-")
}
