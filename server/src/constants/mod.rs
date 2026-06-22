//! Shared constants and enumerations used across the server.

use num_enum::TryFromPrimitive;

/// Describes how a tile's object stack should be updated when sending map data to the client.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum ObjectUpdateType {
    Remove = 0,
    Add    = 1,
    Update = 2,
}
