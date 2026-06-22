//! Packet type enumerations for the Tibia 1.03 network protocol.
//!
//! Each enum variant's discriminant is the one-byte (or two-byte for aux packets)
//! identifier used on the wire. [`PacketOut`] and [`PacketOutAux`] are server->client;
//! [`PacketIn`] is client->server.

use crate::player::Direction;
use num_enum::TryFromPrimitive;

/// Top-level outgoing packet identifiers (server -> client).
#[repr(u16)]
#[derive(Debug, Copy, Clone, TryFromPrimitive)]
pub enum PacketOut {
    Login           = 0x0001,
    Error           = 0x0002,
    DataWindow      = 0x0003,
    Info            = 0x0004,
    MessageOfTheDay = 0x0005,
    Map             = 0x000A,
    MoveNorth       = 0x000B,
    MoveEast        = 0x000C,
    MoveSouth       = 0x000D,
    MoveWest        = 0x000E,
    CloseContainer  = 0x0012,
    OpenContainer   = 0x0013,
    EquippedItem    = 0x0014,
    RemoveEquipped  = 0x0015,
    UpdateObject    = 0x0019,
    LookMessage     = 0x0064,
    Chat            = 0x0065,
    UserList        = 0x0066,
    UserInfo        = 0x0067,
    StatusMessage   = 0x0068,
    Echo            = 0x00C8,
}

/// Converts a [`Direction`] into the corresponding movement packet.
impl From<Direction> for PacketOut {
    fn from(dir: Direction) -> Self {
        match dir {
            Direction::North => Self::MoveNorth,
            Direction::East  => Self::MoveEast,
            Direction::South => Self::MoveSouth,
            Direction::West  => Self::MoveWest,
        }
    }
}

/// Auxiliary packet identifiers embedded within map packets (server -> client).
#[repr(u16)]
#[derive(Debug, Copy, Clone, TryFromPrimitive)]
pub enum PacketOutAux {
    ChangeDirection = 0x00FA,
    Character       = 0x00FB,
}

/// Incoming packet identifiers (client -> server).
#[repr(u16)]
#[derive(Debug, Copy, Clone, TryFromPrimitive)]
pub enum PacketIn {
    UserList          = 0x0003,
    PlayerInfo        = 0x0004,
    Walk              = 0x0005,
    AutoWalk          = 0x0006,
    LookAt            = 0x0007,
    Chat              = 0x0009,
    ChangeDirection   = 0x000A,
    Comment           = 0x000B,
    Push              = 0x0014,
    UseItem           = 0x001E,
    CloseContainer    = 0x001F,
    RequestChangeData = 0x0020,
    SetData           = 0x0021,
    Echo              = 0x00C8,
    Logout            = 0x00FF,
}
