//! Chat message types for Tibia 1.03.
//!
//! Tibia 1.03 (released 1997-02-08) supported a limited set of chat modes. According to the
//! game manual archived on 1997-05-13, the following three were documented for players:
//!
//! - `#W` -> Whisper
//! - `#Y` -> Yell
//! - `#B` -> Broadcast
//!
//! It was also possible to send private messages using `*Name* message` and `@Name@ message`.
//!
//! Reference: <https://web.archive.org/web/19970513130635/http://www-wi.uni-regensburg.de/~vos19618/tibia/e_anleitung.html>

pub mod encoding;

use num_enum::TryFromPrimitive;

/// Identifies the type of a chat message, as sent in the Tibia 1.03 protocol.
///
/// The discriminant values are the single-byte protocol identifiers used in packets.
/// `Look` and `Normal` are not player-selectable via chat prefix - they are assigned
/// by the server based on context.
#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, TryFromPrimitive)]
pub enum ChatType {
    Broadcast = 0x0042, // 'B'
    Look      = 0x004D, // 'M'
    Private   = 0x0050, // 'P'
    Normal    = 0x0053, // 'S'
    Whisper   = 0x0057, // 'W'
    Yell      = 0x0059, // 'Y'
}

impl From<Option<char>> for ChatType {
    /// Parses a chat prefix character (e.g. from `#W`) into a [`ChatType`].
    ///
    /// Matches the three player-accessible prefixes case-insensitively.
    /// Any unrecognised or absent prefix defaults to [`ChatType::Normal`].
    fn from(value: Option<char>) -> Self {
        match value {
            Some('w') | Some('W') => ChatType::Whisper,
            Some('y') | Some('Y') => ChatType::Yell,
            Some('b') | Some('B') => ChatType::Broadcast,
            _ => ChatType::Normal,
        }
    }
}
