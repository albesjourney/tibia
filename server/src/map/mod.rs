/********************************************************************************
 * 
 * This file controls the map:
 * its size, how it loads, JSON deserialization, borders, item ID list, etc.
 * 
 ********************************************************************************/
pub mod position;
pub mod borders;
pub mod items;

use crate::player::OutfitColors;
use anyhow::Result;
use position::Position;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    sync::OnceLock,
};


/********************************************************************************
 * 
 * Global map instance.
 * 
 ********************************************************************************/
pub static MAP: OnceLock<Map> = OnceLock::new();


/********************************************************************************
 * 
 * Map configuration.
 * 
 ********************************************************************************/
const MAP_WIDTH:  u16 = 160;
const MAP_HEIGHT: u16 = 160;
const RESPAWN:    Position = Position::new(50, 100, 7);


/********************************************************************************
 * 
 * World map.
 * 
 ********************************************************************************/
#[derive(Debug)]
pub struct Map {
    pub respawn:    Position,
    tiles:          BTreeMap<Position, Tile>,
    width:          u16,
    height:         u16,
    offset_x:       u16,
    offset_y:       u16,
}


/********************************************************************************
 * 
 * Objects that can exist on a tile.
 * 
 ********************************************************************************/
#[derive(Debug, Clone)]
pub enum TileObject {
    Ground(u16),
    Item(u16),
    Creature(u32, String, OutfitColors),
}


/********************************************************************************
 * 
 * JSON deserialization.
 * 
 ********************************************************************************/
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



/********************************************************************************
 * 
 * Map initialization.
 * 
 ********************************************************************************/
pub fn init(path: &str) -> Result<()> {
    let map = Map::load(path, MAP_WIDTH, MAP_HEIGHT, 0, 0, RESPAWN)?;
    MAP.set(map).map_err(|_| anyhow::anyhow!("Map already initialised"))?;
    Ok(())
}


/********************************************************************************
 * 
 * Map implementation.
 * 
 ********************************************************************************/
impl Map {
    fn load(
        path: &str,
        width: u16,
        height: u16,
        offset_x: u16,
        offset_y: u16,
        respawn: Position,
    ) -> Result<Self> {
        let raw  = fs::read_to_string(path)?;
        let file: MapFile = serde_json::from_str(&raw)?;

        let tile_map = tile_id_map();
        let item_map = item_id_map();

        let mut map = Self {
            respawn,
            tiles: BTreeMap::new(),
            width,
            height,
            offset_x,
            offset_y,
        };


        /********************************************************************************
         * 
         * Start by filling the entire map (160x160) with water.
         * 
         ********************************************************************************/
        for x in 0..width {
            for y in 0..height {
                map.set_ground(Position::new(x + offset_x, y + offset_y, 7), 0x000E);
            }
        }


        /********************************************************************************
         * 
         * Load the map.
         * 
         ********************************************************************************/
        let border_rules = borders::rules();

        for t in file.tiles {
            let pos = Position::new(t.x, t.y, 7);
            let mut items = t.items.unwrap_or_default();

            /********************************************************************************
             * 
             * Load all border rules.
             * For more information, check "src/map/borders.rs".
             * 
             ********************************************************************************/
            let mut matched_rule: Option<&borders::BorderTile> = None;
            for rule in &border_rules {
                if items.iter().any(|item| rule.matches(t.tileid, item.id)) {
                    matched_rule = Some(rule);
                    break;
                }
            }


            /********************************************************************************
             * 
             * Apply borders to tiles
             * 
             ********************************************************************************/
            if let Some(rule) = matched_rule {
                map.set_ground(pos, rule.result_tileid);

                /********************************************************************************
                 * 
                 * Merge "tile + border" -> "bordertile"
                 * 
                 ********************************************************************************/
                if rule.merge {
                    if let Some(idx) = items.iter().position(|item| item.id == rule.item_id) {
                        items.remove(idx);
                    }
                }
            } else if let Some(tileid) = t.tileid {
                /********************************************************************************
                 * 
                 * Tiles that have a border item on it, but that has no tileid,
                 * will fallback to a grass border on water (0x000E) because it is the most common border.
                 * 
                 ********************************************************************************/
                let base = tile_map.get(&tileid).copied().unwrap_or(0x000E);
                map.set_ground(pos, base);
            }


            /********************************************************************************
             * 
             * Add remaining items to the map. If an item is unidentifiable, it will fallback
             * to a "red arrow" (0x00) which is non-existant on the map.
             * 
             ********************************************************************************/
            for item in items.iter().rev() {
                let mapped = item_map.get(&item.id).copied().unwrap_or(0x00);
                if mapped != 0x00 {
                    map.add_item(pos, mapped);
                }
            }
        }

        Ok(map)
    }


    /********************************************************************************
     * 
     * Set tile ground.
     * 
     ********************************************************************************/
    fn set_ground(&mut self, pos: Position, id: u16) {
        let tile = self.tiles.entry(pos).or_insert_with(Tile::new);
        tile.set_ground(TileObject::Ground(id));
    }


    /********************************************************************************
     * 
     * Add item to tile.
     * 
     ********************************************************************************/
    fn add_item(&mut self, pos: Position, id: u16) {
        let tile = self.tiles.entry(pos).or_insert_with(Tile::new);
        tile.add_object(TileObject::Item(id));
    }


    /********************************************************************************
     * 
     * Get all objects on a tile.
     * 
     ********************************************************************************/
    pub fn get_tile_objects(&self, pos: Position) -> Option<&[TileObject]> {
        let in_bounds = pos.x >= self.offset_x
            && pos.x < self.offset_x + self.width
            && pos.y >= self.offset_y
            && pos.y < self.offset_y + self.height;

        if in_bounds {
            self.tiles.get(&pos).map(|t| t.objects.as_slice())
        } else if pos.z == 7 {
            // Water outside map bounds
            Some(&[])
        } else {
            None
        }
    }
}


/********************************************************************************
 * 
 * Internal tile representation.
 * 
 ********************************************************************************/
#[derive(Debug)]
struct Tile {
    objects: Vec<TileObject>,
}

impl Tile {
    /********************************************************************************
     * 
     * Create empty tile.
     * 
     ********************************************************************************/
    fn new() -> Self {
        Self { objects: Vec::new() }
    }


    /********************************************************************************
     * 
     * Set ground object
     * 
     ********************************************************************************/
    fn set_ground(&mut self, obj: TileObject) {
        if self.objects.is_empty() {
            self.objects.push(obj);
        } else {
            self.objects[0] = obj;
        }
    }


    /********************************************************************************
     * 
     * Add object above ground layer
     * 
     ********************************************************************************/
    fn add_object(&mut self, obj: TileObject) {
        self.objects.push(obj);
    }
}


/********************************************************************************
 * 
 * Conversion from Tibia 8.60 tile IDs (OTBM) to Tibia 1.03
 * The map was built using Remere's Map Editor using Tibia 8.60 items.
 * Make sure to not add items that did not exist in Tibia 1.03!
 * 
 ********************************************************************************/
fn tile_id_map() -> HashMap<u16, u16> {
    let mut m = HashMap::new();
    m.insert(100,  0x0001); // void
    m.insert(4608, 0x000E); // water
    m.insert(4526, 0x000A); // grass
    m.insert(104,  0x020A); // sand
    m.insert(405,  0x000C); // wooden floor
    m.insert(424,  0x1C0C); // stone tile
    m.insert(406,  0x010C); // white marble floor
    m.insert(280,  0xB20A); // dirt floor (textured with lines)
    m.insert(351,  0x010A); // dirt floor
    m.insert(352,  0xB20A); // dirt floor
    m.insert(353,  0x120A); // dirt floor
    m.insert(1284, 0x011B); // drawbridge
    m.insert(4566, 0x030A); // gravel
    m.insert(4554, 0x1040); // gravel (border)
    m.insert(4405, 0x050A); // rock soil
    m.insert(965,  0x0113); // chess board (white)
    m.insert(966,  0x0013); // chess board (black)

    // tic-tac-toe board
    for (i, id) in (1016u16..=1024).enumerate() {
        m.insert(id, 0x3313u16 + (i as u16) * 0x0100);
    }

    // mill board
    for (i, id) in (967u16..=1015).enumerate() {
        m.insert(id, 0x0213u16 + (i as u16) * 0x0100);
    }
    m
}


/********************************************************************************
 * 
 * Conversion from Tibia 8.60 item IDs (OTBM) to Tibia 1.03
 * The map was built using Remere's Map Editor using Tibia 8.60 items.
 * Make sure to not add items that did not exist in Tibia 1.03!
 * 
 ********************************************************************************/
fn item_id_map() -> HashMap<u16, u16> {
    let mut m = HashMap::new();
    m.insert(1987,  0x013D); // bag
    m.insert(2376,  0x005A); // sword
    m.insert(2377,  0x015A); // two-handed sword
    m.insert(2378,  0x025A); // battle axe
    m.insert(2389,  0x0D5A); // spear
    m.insert(2509,  0x005D); // steel shield
    m.insert(1443,  0x0023); // statue
    m.insert(1945,  0x0033); // lever (left)
    m.insert(1946,  0x0133); // lever (right)
    m.insert(1754,  0x002C); // bed (top)
    m.insert(1755,  0x012C); // bed (bottom)
    m.insert(1360,  0x001D); // fountain (top-left)
    m.insert(1361,  0x011D); // fountain (top-right)
    m.insert(1362,  0x021D); // fountain (bottom-left)
    m.insert(1363,  0x031D); // fountain (bottom-right)
    m.insert(1027,  0x0214); // brick wall (top-left corner)
    m.insert(1028,  0x0114); // brick wall (horizontal)
    m.insert(1030,  0x0014); // brick wall (vertical)
    m.insert(1035,  0x0614); // brick wall (bottom-right corner)
    m.insert(1207,  0x0216); // archway (left)
    m.insert(1208,  0x0316); // archway (right)
    m.insert(1038,  0x1114); // framework wall (top-left corner)
    m.insert(1039,  0x1014); // framework wall (horizontal)
    m.insert(1040,  0x1314); // framework wall (bottom-right corner)
    m.insert(1041,  0x1414); // framework wall (vertical)
    m.insert(2555,  0x0066); // anvil
    m.insert(2561,  0x0069); // baking tray
    m.insert(1481,  0x0072); // coal basin
    m.insert(2624,  0x0077); // black token
    m.insert(2625,  0x0177); // white token
    m.insert(2767,  0x00A3); // bush
    m.insert(2702,  0x03A0); // willow
    m.insert(2666,  0x0282); // meat
    m.insert(2689,  0x0086); // bread
    m.insert(2693,  0x0187); // lump of dough
    m.insert(2007,  0x023E); // bottle
    m.insert(1717,  0x072A); // chest of drawers
    m.insert(1771,  0x042D); // cask of beer
    m.insert(2692,  0x0087); // flour
    m.insert(2047,  0x0B41); // candlestick
    m.insert(1636,  0x020C); // passthrough
    m.insert(2562,  0x0369); // pot
    m.insert(1740,  0x032B); // chest
    m.insert(1774,  0x052D); // barrel
    m.insert(2638,  0x0E77); // tic-tac-toe token
    m.insert(2639,  0x0F77); // tic-tac-toe token
    m.insert(2626,  0x0877); // white pawn
    m.insert(2627,  0x0977); // white castle
    m.insert(2628,  0x0A77); // white knight
    m.insert(2629,  0x0B77); // white bishop
    m.insert(2630,  0x0C77); // white queen
    m.insert(2631,  0x0D77); // white king
    m.insert(2632,  0x0277); // black pawn
    m.insert(2633,  0x0377); // black castle
    m.insert(2634,  0x0477); // black knight
    m.insert(2635,  0x0577); // black bishop
    m.insert(2636,  0x0677); // black queen
    m.insert(2637,  0x0777); // black king
    m.insert(2561,  0x0069); // baking tray
    m.insert(1775,  0x072D); // trough
    m.insert(17751, 0x062D); // trough of water (this item id was manually added in "map.json")
    m
}
