pub(crate) fn string_to_number(buffer: &str, radix: u32) -> Option<usize> {
    let mut number: usize = 0;

    for c in buffer.chars() {
        number *= radix as usize;
        match c.to_digit(radix) {
            None => {
                return None
            }
            Some(n) => {
                number += n as usize;
            }
        }
    }
    Some(number)
}