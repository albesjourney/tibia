//! World position and related types for Tibia 1.03.

use crate::player::{Direction, InventorySlot};
use anyhow::Result;
use std::{
    convert::TryInto,
    fmt::Display,
    ops::{Add, Sub},
};

/// In Tibia 1.03, there is no Z position, but we add it anyways and
/// always default it to 7 whenever we reference it.
///
/// Positions can also encode inventory slots by setting
/// `x = 0x00FF`- see [`Position::get_qualifier`].
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Position {
    pub x: u16,
    pub y: u16,
    pub z: u8,
}

impl Position {
    /// Creates a new position from raw X, Y, Z coordinates.
    pub const fn new(x: u16, y: u16, z: u8) -> Self {
        Self { x, y, z }
    }

    /// Interprets this position and returns what it refers to.
    ///
    /// In Tibia 1.03, `x = 0x00FF` is a sentinel value indicating that the position
    /// refers to an inventory or container slot rather than a map tile. The slot
    /// index is encoded in `y` (1–8). Any other position returns [`PositionQualifier::None`].
    pub fn get_qualifier(&self) -> Result<PositionQualifier> {
        if self.x == 0x00FF && self.y > 0 && self.y <= 8 {
            return Ok(PositionQualifier::Inventory((self.y as u8).try_into()?));
        }
        Ok(PositionQualifier::None)
    }
}

/// Formats a position as `(x,y,z)`.
impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{},{})", self.x, self.y, self.z)
    }
}

/// Describes what a [`Position`] refers to in the Tibia 1.03 protocol.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PositionQualifier {
    /// A normal map tile position.
    None,
    /// An inventory slot, encoded as `x = 0x00FF`, `y = slot index`.
    Inventory(InventorySlot),
}

/// Offsets a position by a raw `(dx, dy, dz)` tuple.
impl Add<(i16, i16, i8)> for Position {
    type Output = Self;

    fn add(self, rhs: (i16, i16, i8)) -> Self {
        Self {
            x: (self.x as i16 + rhs.0) as u16,
            y: (self.y as i16 + rhs.1) as u16,
            z: (self.z as i8  + rhs.2) as u8,
        }
    }
}

/// Steps a position one tile in the given [`Direction`].
impl Add<Direction> for Position {
    type Output = Self;

    fn add(self, rhs: Direction) -> Self {
        self + match rhs {
            Direction::North => ( 0, -1, 0),
            Direction::East  => ( 1,  0, 0),
            Direction::South => ( 0,  1, 0),
            Direction::West  => (-1,  0, 0),
        }
    }
}

/// Offsets a position by subtracting a raw `(dx, dy, dz)` tuple.
impl Sub<(i16, i16, i8)> for Position {
    type Output = Self;

    fn sub(self, rhs: (i16, i16, i8)) -> Self {
        Self {
            x: (self.x as i16 - rhs.0) as u16,
            y: (self.y as i16 - rhs.1) as u16,
            z: (self.z as i8  - rhs.2) as u8,
        }
    }
}
