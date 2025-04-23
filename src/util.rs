pub fn base_26(mut num: u32) -> String {
    let mut result = String::new();
    while num > 0 {
        num -= 1; // Adjust for 1-based indexing
        let remainder = (num % 26) as u8;
        let letter = (b'A' + remainder) as char; // Convert to letter A-Z
        result.insert(0, letter); // Prepend letter
        num /= 26;
    }
    result
}
