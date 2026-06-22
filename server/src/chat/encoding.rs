//! Translates Unicode chat text into Tibia 1.03's single-byte character encoding (Windows-1252).
//!
//! Tibia 1.03 predates Unicode support and expects Windows-1252 byte values for extended
//! characters. The two functions here handle the subset of characters players are likely to
//! type: German umlauts (Ää, Öö, Üü, ß) and Scandinavian letters (Åå, Ææ, Øø). Pure ASCII
//! passes through unchanged in both cases.

/// Translates a Unicode string into Tibia 1.03's Windows-1252 encoding, preserving case.
///
/// Extended characters are mapped to their Windows-1252 byte values. Any character
/// outside the handled set is cast directly to `u8`, which is lossless for ASCII
/// but will silently truncate anything above U+00FF.
pub fn translate(input: &str) -> Vec<u8> {
    input.chars().map(|c| match c {
        'Ä' => 0x00C4,
        'Å' => 0x00C5,
        'Æ' => 0x00C6,
        'Ö' => 0x00D6,
        'Ø' => 0x00D8,
        'Ü' => 0x00DC,
        'ß' => 0x00DF,
        'ä' => 0x00E4,
        'å' => 0x00E5,
        'æ' => 0x00E6,
        'ö' => 0x00F6,
        'ø' => 0x00F8,
        'ü' => 0x00FC,
        c   => c as u8,
    }).collect()
}

/// Translates a Unicode string into Tibia 1.03's Windows-1252 encoding, folding to uppercase.
///
/// Both cases of each supported character map to the same uppercase Windows-1252 byte,
/// matching the game's case-insensitive name and channel comparisons. Note that
/// `ß` has no standard uppercase form in Windows-1252, so it is left as-is (0x00DF).
pub fn translate_upper(input: &str) -> Vec<u8> {
    input.chars().map(|c| match c {
        'Ä' | 'ä' => 0x00C4,
        'Å' | 'å' => 0x00C5,
        'Ö' | 'ö' => 0x00D6,
        'Ü' | 'ü' => 0x00DC,
        'ß'       => 0x00DF,
        'Æ' | 'æ' => 0x00C6,
        'Ø' | 'ø' => 0x00D8,
        c         => c as u8,
    }).collect()
}
