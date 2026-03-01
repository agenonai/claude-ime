//! UTF-8 boundary detection for split PTY reads.
//!
//! When data is read from a PTY in fixed-size chunks the read boundary may
//! fall in the middle of a multi-byte UTF-8 sequence.  Forwarding an
//! incomplete sequence to the terminal causes the display to show the Unicode
//! replacement character (U+FFFD) or garbled output.
//!
//! This module provides a safe-boundary finder: given a buffer that contains
//! `len` freshly-read bytes, it walks backwards from the end to find the
//! largest prefix that ends on a complete UTF-8 code-point boundary.  The
//! trailing incomplete bytes are left in the buffer so they can be prepended
//! to the next read.
//!
//! # UTF-8 encoding recap
//!
//! | Code-point range  | Byte 1     | Byte 2     | Byte 3     | Byte 4     |
//! |-------------------|------------|------------|------------|------------|
//! | U+0000..U+007F    | 0xxxxxxx   |            |            |            |
//! | U+0080..U+07FF    | 110xxxxx   | 10xxxxxx   |            |            |
//! | U+0800..U+FFFF    | 1110xxxx   | 10xxxxxx   | 10xxxxxx   |            |
//! | U+10000..U+10FFFF | 11110xxx   | 10xxxxxx   | 10xxxxxx   | 10xxxxxx   |
//!
//! A *continuation byte* has the bit pattern `10xxxxxx` (0x80–0xBF).
//! A *leading byte* starts with `11xxxxxx` (0xC0 or higher, excluding
//! continuation bytes).

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return the index of the last safe UTF-8 code-point boundary within the
/// first `len` bytes of `buf`.
///
/// "Safe" means that `buf[..result]` is a valid, complete UTF-8 string.
/// Any bytes from `result` to `len` form the prefix of an incomplete
/// multi-byte sequence and should be buffered until more data arrives.
///
/// # Special cases
///
/// * If `len == 0` the function returns `0`.
/// * If all `len` bytes form valid, complete sequences the function returns
///   `len` — i.e. the entire slice is safe to forward.
/// * Pure ASCII input always returns `len` because every byte is a complete
///   one-byte sequence.
///
/// # Panics
///
/// Panics if `len > buf.len()`.
pub fn find_safe_boundary(buf: &[u8], len: usize) -> usize {
    assert!(len <= buf.len(), "len ({len}) > buf.len() ({})", buf.len());
    if len == 0 {
        return 0;
    }
    match std::str::from_utf8(&buf[..len]) {
        Ok(_) => len,
        Err(e) => e.valid_up_to(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- find_safe_boundary: empty buffer ---

    #[test]
    fn empty_buffer_returns_zero() {
        let buf = [];
        assert_eq!(find_safe_boundary(&buf, 0), 0);
    }

    // --- find_safe_boundary: pure ASCII ---

    #[test]
    fn pure_ascii_boundary_equals_len() {
        let buf = b"hello world";
        assert_eq!(find_safe_boundary(buf, buf.len()), buf.len());
    }

    #[test]
    fn single_ascii_byte() {
        let buf = b"A";
        assert_eq!(find_safe_boundary(buf, 1), 1);
    }

    // --- find_safe_boundary: complete multi-byte sequences ---

    /// "ệ" is U+1EB9, encoded as 0xE1 0xBA 0xB9 (3 bytes).
    #[test]
    fn complete_vietnamese_e_with_dot_below() {
        // U+1EB9 LATIN SMALL LETTER E WITH DOT BELOW  →  ệ
        let buf: &[u8] = b"\xE1\xBA\xB9";
        assert_eq!(find_safe_boundary(buf, 3), 3);
    }

    /// A full 3-byte sequence preceded by ASCII should be safe in its
    /// entirety.
    #[test]
    fn ascii_then_complete_3byte() {
        // "xin" + "ệ" — all bytes complete
        let buf: Vec<u8> = b"xin"
            .iter()
            .chain(b"\xE1\xBA\xB9".iter())
            .copied()
            .collect();
        assert_eq!(find_safe_boundary(&buf, buf.len()), buf.len());
    }

    // --- find_safe_boundary: split in the middle of a 3-byte sequence ---

    /// Buffer holds the leading byte + first continuation byte of a 3-byte
    /// sequence; the third byte is missing.  Boundary should be at index 0
    /// (nothing safe to forward — the sequence started at byte 0).
    #[test]
    fn split_3byte_sequence_missing_last_byte() {
        // U+1EB9 = 0xE1 0xBA 0xB9; we present only 0xE1 0xBA
        let buf: &[u8] = &[0xE1, 0xBA];
        assert_eq!(find_safe_boundary(buf, 2), 0);
    }

    /// Buffer has "hello" then the first byte of a 3-byte sequence.
    /// Boundary should be after "hello" (index 5).
    #[test]
    fn ascii_then_partial_3byte() {
        let mut buf: Vec<u8> = b"hello".to_vec();
        buf.push(0xE1); // leading byte of a 3-byte sequence — no continuation bytes
        assert_eq!(find_safe_boundary(&buf, buf.len()), 5);
    }

    /// Buffer has "hello" then two bytes of a 3-byte sequence.
    /// Boundary should be after "hello" (index 5).
    #[test]
    fn ascii_then_partial_3byte_two_bytes_present() {
        let mut buf: Vec<u8> = b"hello".to_vec();
        buf.extend_from_slice(&[0xE1, 0xBA]); // 2 of 3 bytes
        assert_eq!(find_safe_boundary(&buf, buf.len()), 5);
    }

    // --- find_safe_boundary: split in the middle of a 4-byte sequence (emoji) ---

    /// U+1F600 GRINNING FACE  →  😀  →  0xF0 0x9F 0x98 0x80 (4 bytes).
    #[test]
    fn complete_4byte_emoji() {
        let buf: &[u8] = &[0xF0, 0x9F, 0x98, 0x80];
        assert_eq!(find_safe_boundary(buf, 4), 4);
    }

    /// Only the first byte of the emoji arrived.
    #[test]
    fn partial_4byte_emoji_one_byte() {
        let buf: &[u8] = &[0xF0];
        assert_eq!(find_safe_boundary(buf, 1), 0);
    }

    /// First two bytes of the emoji arrived.
    #[test]
    fn partial_4byte_emoji_two_bytes() {
        let buf: &[u8] = &[0xF0, 0x9F];
        assert_eq!(find_safe_boundary(buf, 2), 0);
    }

    /// Three bytes of the emoji arrived (missing the last continuation byte).
    #[test]
    fn partial_4byte_emoji_three_bytes() {
        let buf: &[u8] = &[0xF0, 0x9F, 0x98];
        assert_eq!(find_safe_boundary(buf, 3), 0);
    }

    /// ASCII followed by a partial emoji — boundary should be after the ASCII.
    #[test]
    fn ascii_then_partial_emoji() {
        // "ok" + first two bytes of 😀
        let mut buf: Vec<u8> = b"ok".to_vec();
        buf.extend_from_slice(&[0xF0, 0x9F]);
        assert_eq!(find_safe_boundary(&buf, buf.len()), 2);
    }

    /// ASCII followed by a complete emoji — entire buffer is safe.
    #[test]
    fn ascii_then_complete_emoji() {
        let mut buf: Vec<u8> = b"ok".to_vec();
        buf.extend_from_slice(&[0xF0, 0x9F, 0x98, 0x80]);
        assert_eq!(find_safe_boundary(&buf, buf.len()), buf.len());
    }

    // --- find_safe_boundary: len < buf.len() ---

    /// When `len` is smaller than `buf.len()`, only the first `len` bytes are
    /// considered (the rest is leftover from a previous read).
    #[test]
    fn respects_len_parameter() {
        // buf has a complete emoji but we only "received" the first 2 bytes.
        let buf: &[u8] = &[0xF0, 0x9F, 0x98, 0x80];
        assert_eq!(find_safe_boundary(buf, 2), 0);
    }

    // --- find_safe_boundary: real Vietnamese text ---

    /// "Việt Nam" in UTF-8.
    #[test]
    fn complete_vietnamese_phrase() {
        let phrase = "Việt Nam";
        let bytes = phrase.as_bytes();
        assert_eq!(find_safe_boundary(bytes, bytes.len()), bytes.len());
    }

    /// Split "Việt" after the 'i' — the 'ê' with combining marks begins an
    /// incomplete sequence at that point.
    #[test]
    fn split_vietnamese_word_mid_sequence() {
        // "Việt" = V(56) i(69) ệ(E1 BB 87) t(74)
        // We present "Vi" + leading byte of "ệ"
        let mut buf: Vec<u8> = b"Vi".to_vec();
        buf.push(0xE1); // leading byte of ệ (U+1EC7)
        let safe = find_safe_boundary(&buf, buf.len());
        assert_eq!(safe, 2, "boundary must be after 'Vi', before incomplete ệ");
    }

    // --- find_safe_boundary: Bug 1 regression — consecutive incomplete leaders ---

    #[test]
    fn two_consecutive_incomplete_leading_bytes() {
        // ô-start [0xC3] + ơ-start [0xC6], no continuation bytes
        let buf: &[u8] = &[0xC3, 0xC6];
        assert_eq!(find_safe_boundary(buf, 2), 0);
    }

    #[test]
    fn complete_two_byte_then_incomplete_two_byte() {
        // ô = [0xC3, 0xB4] (complete); [0xC6] = ơ start (incomplete)
        let buf: &[u8] = &[0xC3, 0xB4, 0xC6];
        assert_eq!(find_safe_boundary(buf, 3), 2);
    }

    #[test]
    fn complete_3byte_then_incomplete_2byte() {
        // ệ = [0xE1, 0xBA, 0xB9] (complete 3-byte); [0xC6] = incomplete ơ start
        let buf: &[u8] = &[0xE1, 0xBA, 0xB9, 0xC6];
        assert_eq!(find_safe_boundary(buf, 4), 3);
    }
}
