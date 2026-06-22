//! Player state and associated types for Tibia 1.03.

use crate::map::position::Position;
use num_enum::TryFromPrimitive;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

/// A single open container window shown on the client.
#[derive(Clone, Debug)]
pub struct OpenContainer {
    /// The item id of the container itself (used as the window icon).
    pub container_id: u16,
    /// The items stored inside this container.
    pub items: Vec<u16>,
    /// The local window index sent to the client (0-based).
    pub local_id: u8,
    /// Where this container lives: either an equipment slot or a map position.
    /// Used to auto-close the window when the container moves out of range or
    /// is unequipped.
    pub source: ContainerSource,
}

/// Where an open container came from.
#[derive(Clone, Debug, PartialEq)]
pub enum ContainerSource {
    /// The container is in the player's equipment at the given slot.
    Equipment(u8),
    /// The container is sitting on the map at the given position.
    Map(Position),
}

/// A connected player and all their associated state.
#[derive(Clone, Debug)]
pub struct Player {
    pub id:        u32,
    pub name:      String,
    pub password:  String,

    pub real_name: String,
    pub location:  String,
    pub email:     String,

    pub gender:    Gender,
    pub outfit:    OutfitColors,
    pub position:  Position,
    pub direction: Direction,

    pub equipment:  HashMap<u8, u16>,
    pub containers: Vec<OpenContainer>,
    pub equipment_container_contents: HashMap<u8, Vec<u16>>,
}

impl Player {
    /// Creates a new player at `position` with a unique id and default starting equipment.
    ///
    /// Player ids start at 256. Ids below that are reserved for non-player use.
    pub fn new(name: &str, position: Position) -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(256);

        let mut equipment = HashMap::new();
        equipment.insert(3, 0x013D); // bag (slot 3)
        equipment.insert(5, 0x005A); // sword (slot 5, right hand)
        equipment.insert(6, 0x0086); // bread (slot 6, left hand)

        Self {
            id:         NEXT_ID.fetch_add(1, Ordering::SeqCst),
            name:       name.to_owned(),
            password:   String::new(),
            real_name:  String::new(),
            location:   String::new(),
            email:      String::new(),
            position,
            outfit:     OutfitColors::default(),
            gender:     Gender::Male,
            direction:  Direction::default(),
            equipment,
            containers: Vec::new(),
            equipment_container_contents: HashMap::new(),
        }
    }
}

/// Four 4-bit color indices describing a player's outfit appearance.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct OutfitColors {
    pub head:  u8,
    pub body:  u8,
    pub legs:  u8,
    pub shoes: u8,
}

impl OutfitColors {
    pub const fn new(head: u8, body: u8, legs: u8, shoes: u8) -> Self {
        Self { head, body, legs, shoes }
    }
}

/// Sets a default outfit color for all players.
impl Default for OutfitColors {
    fn default() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

/// A cardinal direction, used for movement and the facing direction sent to clients.
#[repr(u8)]
#[derive(Debug, Copy, Clone, TryFromPrimitive)]
pub enum Direction {
    North = 0,
    East  = 1,
    South = 2,
    West  = 3,
}

/// Players face south on login by default.
impl Default for Direction {
    fn default() -> Self {
        Direction::South
    }
}

/// The gender of a character.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Gender {
    Male,
    Female,
}

/// An equipment slot in the player's inventory panel.
///
/// The discriminant values match the slot indices used in the Tibia 1.03 protocol,
/// and correspond to the keys stored in [`Player::equipment`].
#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, TryFromPrimitive)]
pub enum InventorySlot {
    Helmet    = 1,
    Necklace  = 2,
    Bag       = 3,
    Armor     = 4,
    RightHand = 5,
    LeftHand  = 6,
    Legs      = 7,
    Boots     = 8,
}
