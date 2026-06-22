//! World map - loading, tile storage, and spatial queries for Tibia 1.03.
//!
//! The map is loaded from a JSON file exported from Remere's Map Editor (originally built
//! with Tibia 8.60 assets) and converted to Tibia 1.03 item and tile ids on the fly.
//! Border merging (see [`borders`]) and id translation ([`tile_id_map`], [`item_id_map`])
//! happen at load time; the runtime representation is a plain [`BTreeMap`] of [`Tile`]s.

pub mod borders;
pub mod items;
pub mod position;

use anyhow::Result;
use position::Position;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    sync::OnceLock,
};
use tokio::sync::RwLock;

/// Global map instance, populated once by [`init`] at startup.
pub static MAP: OnceLock<RwLock<Map>> = OnceLock::new();

/// Width of the playable map area in tiles.
const MAP_WIDTH:  u16 = 160;
/// Height of the playable map area in tiles.
const MAP_HEIGHT: u16 = 160;
/// Default temple position.
const RESPAWN: Position = Position::new(50, 100, 7);

/// The live world map, holding all tile and object state.
pub struct Map {
    pub respawn: Position,
    tiles: BTreeMap<Position, Tile>,
    /// Persistent contents of containers sitting on the map, keyed by their tile position.
    /// When a container on the ground is closed and reopened, contents are loaded from here.
    pub container_contents: HashMap<Position, Vec<u16>>,
    width: u16,
    height: u16,
    offset_x: u16,
    offset_y: u16,
}

/// An object that can occupy a position on a tile.
///
/// The ground layer is always index 0 in a tile's object list; items are stacked above
/// it. Tibia 1.03 has no NPCs or monsters - the only "creatures" in the game are players,
/// which are tracked separately via [`Connection::other_players`](crate::network::connection::Connection)
/// rather than as tile objects, so there is no generic creature variant here.
#[derive(Debug, Clone)]
pub enum TileObject {
    Ground(u16),
    Item(u16),
}

// JSON deserialization types (map file format)

#[derive(Deserialize)]
struct MapFile {
    tiles: Vec<JsonTile>,
}

#[derive(Deserialize)]
struct JsonTile {
    x:      u16,
    y:      u16,
    #[serde(default)]
    tileid: Option<u16>,
    items:  Option<Vec<JsonItem>>,
}

#[derive(Deserialize)]
struct JsonItem {
    id: u16,
}

/// Loads the map from `path` and stores it in the global [`MAP`] instance.
///
/// Returns an error if the file cannot be read, fails to parse, or if `init`
/// has already been called.
pub fn init(path: &str) -> Result<()> {
    let map = Map::load(path, MAP_WIDTH, MAP_HEIGHT, 0, 0, RESPAWN)?;
    MAP.set(RwLock::new(map)).map_err(|_| anyhow::anyhow!("Map has already been initialized."))?;
    Ok(())
}

impl Map {
    /// Reads and converts the JSON map file into a runtime [`Map`].
    ///
    /// The load sequence is:
    /// 1. Fill the entire world with water (`0x000E`).
    /// 2. For each tile in the file, check border rules (see [`borders`]) and apply
    ///    the merged bordertile if one matches.
    /// 3. Translate remaining Tibia 8.60 item ids to Tibia 1.03 ids and add them
    ///    to the tile. Unrecognised items are silently skipped.
    fn load(
        path: &str,
        width: u16,
        height: u16,
        offset_x: u16,
        offset_y: u16,
        respawn: Position,
    ) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let file: MapFile = serde_json::from_str(&raw)?;

        let tile_map = tile_id_map();
        let item_map = item_id_map();

        let mut map = Self {
            respawn,
            tiles: BTreeMap::new(),
            container_contents: HashMap::new(),
            width,
            height,
            offset_x,
            offset_y,
        };

        // Fill the entire map with water first; tiles loaded from the file will overwrite it.
        for x in 0..width {
            for y in 0..height {
                map.set_ground(Position::new(x + offset_x, y + offset_y, 7), 0x000E);
            }
        }

        let border_rules = borders::rules();

        for t in file.tiles {
            let pos = Position::new(t.x, t.y, 7);
            let mut items = t.items.unwrap_or_default();

            // Check whether any item on this tile matches a border merging rule.
            // Only the first matching rule is applied. See `src/map/borders.rs`.
            let matched_rule = border_rules.iter().find(|rule| items.iter().any(|item| rule.matches(t.tileid, item.id)));

            if let Some(rule) = matched_rule {
                map.set_ground(pos, rule.result_tileid);

                // When merging, remove the border item so it isn't also added as a
                // regular item below - it's now merged into the bordertile sprite.
                if rule.merge {
                    if let Some(idx) = items.iter().position(|item| item.id == rule.item_id) {
                        items.remove(idx);
                    }
                }
            } else if let Some(tileid) = t.tileid {
                // No border rule matched. Tiles without a recognised id fall back to
                // water (0x000E) as base, the most common unmatched border case.
                let base = tile_map.get(&tileid).copied().unwrap_or(0x000E);
                map.set_ground(pos, base);
            }

            // Add remaining items in reverse order so the topmost item ends up last
            // in the object stack. Unknown ids (mapped to 0x0000) are skipped.
            for item in items.iter().rev() {
                let mapped = item_map.get(&item.id).copied().unwrap_or(0x0000);
                if mapped != 0x0000 {
                    map.add_item(pos, mapped);
                }
            }
        }

        Ok(map)
    }

    /// Sets the ground layer (stack position 0) of the tile at `pos`.
    fn set_ground(&mut self, pos: Position, id: u16) {
        let tile = self.tiles.entry(pos).or_insert_with(Tile::new);
        tile.set_ground(TileObject::Ground(id));
    }

    /// Appends an item on top of the ground layer at `pos`.
    fn add_item(&mut self, pos: Position, id: u16) {
        let tile = self.tiles.entry(pos).or_insert_with(Tile::new);
        tile.add_object(TileObject::Item(id));
    }

    /// Returns the object stack for the tile at `pos`, if it is in bounds.
    ///
    /// Positions outside the map boundary return `None`.
    pub fn get_tile_objects(&self, pos: Position) -> Option<&[TileObject]> {
        let in_bounds = pos.x >= self.offset_x
            && pos.x < self.offset_x + self.width
            && pos.y >= self.offset_y
            && pos.y < self.offset_y + self.height;

        if in_bounds {
            self.tiles.get(&pos).map(|t| t.objects.as_slice())
        } else {
            // Some(&[])
            None
        }
    }

    /// Removes the first item with `item_id` from the tile at `pos`.
    ///
    /// Returns `true` if an item was removed, `false` if none was found.
    pub fn remove_item(&mut self, pos: Position, item_id: u16) -> bool {
        if let Some(tile) = self.tiles.get_mut(&pos) {
            if let Some(index) = tile.objects.iter().position(|o| {
                matches!(o, TileObject::Item(id) if *id == item_id)
            }) {
                tile.objects.remove(index);
                return true;
            }
        }
        false
    }

    /// Places an item on top of the tile's stack at `pos`, creating the tile if needed.
    ///
    /// Used for drops, throws, and pushes - the item should always end up as the new
    /// topmost item, matching the convention used elsewhere (e.g. `recv_look_at`, which
    /// treats the first item after the ground layer as the topmost).
    pub fn add_item_to_tile(&mut self, pos: Position, item_id: u16) {
        let tile = self.tiles.entry(pos).or_insert_with(Tile::new);
        tile.insert_on_top(TileObject::Item(item_id));
    }

    /// Returns the persistent item list for a container at `pos`, if any.
    pub fn get_container_contents(&self, pos: Position) -> Option<&Vec<u16>> {
        self.container_contents.get(&pos)
    }

    /// Saves the item list for a container at `pos` into persistent map state.
    pub fn set_container_contents(&mut self, pos: Position, items: Vec<u16>) {
        self.container_contents.insert(pos, items);
    }

    /// Removes and returns the persisted item list for a container at `pos`, if any.
    ///
    /// Used when a container is moved on the map - its contents must follow it to the
    /// new tile position rather than remaining keyed under the old one.
    pub fn take_container_contents(&mut self, pos: Position) -> Option<Vec<u16>> {
        self.container_contents.remove(&pos)
    }
}

/// Internal tile representation - a stack of [`TileObject`]s where index 0 is always ground.
#[derive(Debug)]
struct Tile {
    objects: Vec<TileObject>,
}

impl Tile {
    fn new() -> Self {
        Self { objects: Vec::new() }
    }

    /// Replaces the ground layer, or pushes it if the tile is empty.
    fn set_ground(&mut self, obj: TileObject) {
        if self.objects.is_empty() {
            self.objects.push(obj);
        } else {
            self.objects[0] = obj;
        }
    }

    /// Pushes an object above the ground layer.
    fn add_object(&mut self, obj: TileObject) {
        self.objects.push(obj);
    }

    /// Inserts an object directly above the ground layer, so it becomes the new topmost
    /// item on the stack - used when dropping or throwing an item onto a tile that may
    /// already have items on it. Falls back to appending if the tile has no ground layer
    /// yet, which shouldn't normally happen for tiles loaded from the map.
    fn insert_on_top(&mut self, obj: TileObject) {
        let index = if self.objects.is_empty() { 0 } else { 1 };
        self.objects.insert(index, obj);
    }
}

/// Maps Tibia 8.60 tile ids (from Remere's Map Editor / OTBM) to Tibia 1.03 tile ids.
///
/// Only tiles that existed in Tibia 1.03 should be added here.
fn tile_id_map() -> HashMap<u16, u16> {
    let mut m = HashMap::new();
    m.insert(1284, 0x011B); // a drawbridge
    m.insert(4566, 0x030A); // gravel
    m.insert(4554, 0x1040); // gravel (border)
    m.insert(351,  0x010A); // dirt floor
    m.insert(352,  0xB20A); // dirt floor
    m.insert(353,  0x120A); // dirt floor
    m.insert(405,  0x000C); // wooden floor
    m.insert(406,  0x010C); // white marble floor
    m.insert(4526, 0x000A); // grass
    m.insert(424,  0x1C0C); // a stone tile
    m.insert(104,  0x020A); // sand
    m.insert(4405, 0x050A); // rock soil
    m.insert(4608, 0x000E); // water
    m.insert(280,  0xB20A); // dirt floor (textured with lines)
    m.insert(100,  0x0001); // void
    m.insert(966,  0x0013); // chess board (black)
    m.insert(965,  0x0113); // chess board (white)

    // Tic-tac-toe board variants (9 tiles, ids 1016–1024).
    for (i, id) in (1016u16..=1024).enumerate() {
        m.insert(id, 0x3313u16 + (i as u16) * 0x0100);
    }
    
    // Mill board variants (49 tiles, ids 967–1015).
    for (i, id) in (967u16..=1015).enumerate() {
        m.insert(id, 0x0213u16 + (i as u16) * 0x0100);
    }

    m
}

/// Maps Tibia 8.60 item ids (from Remere's Map Editor / OTBM) to Tibia 1.03 item ids.
///
/// Only items that existed in Tibia 1.03 should be added here.
fn item_id_map() -> HashMap<u16, u16> {
    let mut m = HashMap::new();
    m.insert(2376,  0x005A); // a sword
    m.insert(2007,  0x023E); // a bottle
    m.insert(2561,  0x0069); // a baking tray
    m.insert(2693,  0x0187); // a lump of dough
    m.insert(2692,  0x0087); // flour
    m.insert(2689,  0x0086); // bread
    m.insert(1207,  0x0216); // an archway
    m.insert(1208,  0x0316); // an archway
    m.insert(1027,  0x0214); // a brick wall
    m.insert(1028,  0x0114); // a brick wall
    m.insert(1030,  0x0014); // a brick wall
    m.insert(1035,  0x0614); // a brick wall
    m.insert(1636,  0x020C); // a passthrough
    m.insert(2767,  0x00A3); // a bush
    m.insert(2702,  0x03A0); // a willow
    m.insert(1740,  0x032B); // a chest
    m.insert(1771,  0x042D); // a beer cask
    m.insert(1481,  0x0072); // a coal basin
    m.insert(2555,  0x0066); // an anvil
    m.insert(1987,  0x013D); // a bag
    m.insert(1774,  0x052D); // a barrel
    m.insert(2047,  0x0B41); // a candlestick
    m.insert(1945,  0x0033); // a lever
    m.insert(1946,  0x0133); // a lever
    m.insert(17751, 0x062D); // a trough of water
    m.insert(1775,  0x072D); // a trough
    m.insert(1038,  0x1114); // a framework wall
    m.insert(1039,  0x1014); // a framework wall
    m.insert(1040,  0x1314); // a framework wall
    m.insert(1041,  0x1414); // a framework wall
    m.insert(2509,  0x005D); // a steel shield
    m.insert(1754,  0x002C); // a bed
    m.insert(1755,  0x012C); // a bed
    m.insert(1443,  0x0023); // a knight statue
    m.insert(2562,  0x0369); // a pot
    m.insert(2378,  0x025A); // a battle axe
    m.insert(2389,  0x0D5A); // a spear
    m.insert(2377,  0x015A); // a two-handed sword
    m.insert(2666,  0x0282); // meat
    m.insert(1717,  0x072A); // a chest of drawers
    m.insert(1360,  0x001D); // a fountain
    m.insert(1361,  0x011D); // a fountain
    m.insert(1362,  0x021D); // a fountain
    m.insert(1363,  0x031D); // a fountain
    m.insert(2624,  0x0077); // a black token
    m.insert(2625,  0x0177); // a white token
    m.insert(2632,  0x0277); // a black pawn
    m.insert(2633,  0x0377); // a black castle
    m.insert(2634,  0x0477); // a black knight
    m.insert(2635,  0x0577); // a black bishop
    m.insert(2636,  0x0677); // a black queen
    m.insert(2637,  0x0777); // a black king
    m.insert(2626,  0x0877); // a white pawn
    m.insert(2627,  0x0977); // a white castle
    m.insert(2628,  0x0A77); // a white knight
    m.insert(2629,  0x0B77); // a white bishop
    m.insert(2630,  0x0C77); // a white queen
    m.insert(2631,  0x0D77); // a white king
    m.insert(2638,  0x0E77); // a tic-tac-toe token
    m.insert(2639,  0x0F77); // a tic-tac-toe token
    
    m
}
