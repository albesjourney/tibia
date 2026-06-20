use crate::player::{Direction, InventorySlot};
use anyhow::Result;
use std::{
    convert::TryInto,
    fmt::Display,
    ops::{Add, Sub}
};


/********************************************************************************
 * 
 * World position
 * 
 ********************************************************************************/
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Position {
    pub x: u16,
    pub y: u16,
    pub z: u8,
}

impl Position {
    /********************************************************************************
     * 
     * Create a new position
     * 
     ********************************************************************************/
    pub const fn new(x: u16, y: u16, z: u8) -> Self {
        Self { x, y, z }
    }


    /********************************************************************************
     * 
     * Determine whether this position refers to:
     * - A map tile
     * - An inventory slot
     * - Something else
     * 
     ********************************************************************************/
    pub fn get_qualifier(&self) -> Result<PositionQualifier> {
        /********************************************************************************
         * 
         * In Tibia 1.03:
         * x == 0xFF signals an inventory or container slot.
         * 
         ********************************************************************************/
        if self.x == 0xff && self.y > 0 && self.y <= 8 {
            return Ok(PositionQualifier::Inventory((self.y as u8).try_into()?));
        }

        Ok(PositionQualifier::None)
    }
}


/********************************************************************************
 * 
 * Position formatting (x, y, z)
 * 
 ********************************************************************************/
impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{},{})", self.x, self.y, self.z)
    }
}


/********************************************************************************
 * 
 * Position qualifiers
 * 
 ********************************************************************************/
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PositionQualifier {
    None,
    Inventory(InventorySlot),
}


/********************************************************************************
 * 
 * Position arithmetic
 * 
 ********************************************************************************/
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


/********************************************************************************
 * 
 * Move a position in a direction
 * 
 ********************************************************************************/
impl Add<Direction> for Position {
    type Output = Self;

    fn add(self, rhs: Direction) -> Self {
        self + match rhs {
            Direction::North => (0, -1, 0),
            Direction::East  => (1,  0, 0),
            Direction::South => (0,  1, 0),
            Direction::West  => (-1, 0, 0),
        }
    }
}


/********************************************************************************
 * 
 * Position subtraction
 * 
 ********************************************************************************/
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
