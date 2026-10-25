//! Character positions in strings.
//!
//! Every position and length that Pleno shows for a string counts Unicode
//! scalar values (Rust `char`s), never UTF-8 bytes. These helpers convert
//! between the two. Text that starts with ASCII takes a fast path: there a
//! byte offset and a character position are the same number.

/// The number of characters in `s`.
#[inline]
pub fn char_len(s: &str) -> usize {
    // The standard library counts a word at a time, so this stays cheap.
    s.chars().count()
}

/// The byte offset where character `index` starts, or `s.len()` when `index`
/// is at or past the end. Always a char boundary.
#[inline]
pub fn byte_offset(s: &str, index: usize) -> usize {
    let bytes = s.as_bytes();
    if index >= bytes.len() {
        return bytes.len();
    }
    if bytes[..index].is_ascii() {
        return index;
    }
    s.char_indices().nth(index).map_or(s.len(), |(i, _)| i)
}

/// The character position of byte offset `byte` (a char boundary of `s`).
#[inline]
pub fn char_index(s: &str, byte: usize) -> usize {
    let prefix = &s[..byte];
    if prefix.is_ascii() {
        byte
    } else {
        prefix.chars().count()
    }
}

/// The character at position `index`, if there is one.
#[inline]
pub fn char_at(s: &str, index: usize) -> Option<char> {
    let bytes = s.as_bytes();
    if index < bytes.len() && bytes[..=index].is_ascii() {
        return Some(bytes[index] as char);
    }
    s.chars().nth(index)
}

/// The characters from position `start` up to (not including) `end`.
/// Positions past the end count as the end; `end < start` gives "".
pub fn char_slice(s: &str, start: usize, end: usize) -> &str {
    if end <= start {
        return "";
    }
    let from = byte_offset(s, start);
    let rest = &s[from..];
    let to = from + byte_offset(rest, end - start);
    &s[from..to]
}

/// A position given as an integer, clamped into `0..=len`.
#[inline]
pub fn clamp_position(position: i64, len: usize) -> usize {
    if position <= 0 {
        0
    } else {
        (position as u64).min(len as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_characters() {
        assert_eq!(char_len(""), 0);
        assert_eq!(char_len("abc"), 3);
        assert_eq!(char_len("héllo"), 5);
        assert_eq!(char_len("a€😀"), 3);
    }

    #[test]
    fn converts_positions() {
        let s = "a€😀b";
        assert_eq!(byte_offset(s, 0), 0);
        assert_eq!(byte_offset(s, 1), 1);
        assert_eq!(byte_offset(s, 2), 4);
        assert_eq!(byte_offset(s, 3), 8);
        assert_eq!(byte_offset(s, 4), 9);
        assert_eq!(byte_offset(s, 99), 9);
        assert_eq!(char_index(s, 8), 3);
        assert_eq!(char_at(s, 2), Some('😀'));
        assert_eq!(char_at(s, 4), None);
        assert_eq!(char_slice(s, 1, 3), "€😀");
        assert_eq!(char_slice(s, 3, 1), "");
        assert_eq!(char_slice(s, 2, 99), "😀b");
        assert_eq!(clamp_position(-5, 3), 0);
        assert_eq!(clamp_position(i64::MAX, 3), 3);
    }
}
