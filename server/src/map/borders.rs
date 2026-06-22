//! Border tile merging for map conversion from Tibia 8.60 to Tibia 1.03.
//!
//! The map was built in Remere's Map Editor using Tibia 8.60 assets, where border sprites
//! are separate items placed on top of tiles. Tibia 1.03 has no concept of separate border
//! items - instead, borders are baked into the tile sprite itself ("bordertiles").
//!
//! During map load, each `(tile_id, border_item_id)` pair is looked up against the rules
//! returned by [`rules`] and replaced with the corresponding Tibia 1.03 bordertile id.

/// A rule that maps a `(tile_id, border_item_id)` pair to a Tibia 1.03 bordertile id.
///
/// When `merge` is `true`, the original tile is replaced by `result_tileid`. When `false`,
/// the result is a standalone bordertile with no underlying tile (e.g. grass bordering water).
pub struct BorderTile {
    pub tileid: Option<u16>,
    pub item_id: u16,
    pub result_tileid: u16,
    pub merge: bool,
}

impl BorderTile {
    const fn new(tileid: Option<u16>, item_id: u16, result_tileid: u16, merge: bool) -> Self {
        Self { tileid, item_id, result_tileid, merge }
    }

    /// Returns `true` if this rule matches the given tile id and border item id.
    pub fn matches(&self, tileid: Option<u16>, item_id: u16) -> bool {
        self.tileid == tileid && self.item_id == item_id
    }
}

/// Returns all border merging rules applied during map load.
///
/// Each rule encodes a direction or corner position - the inline comments indicate
/// where the border strip sits relative to the tile it is applied to.
pub fn rules() -> Vec<BorderTile> {
    vec![
        // Grass borders on tiles with no tile id (e.g. water).
        BorderTile::new(None, 7653, 0x5A0E, false), // top
        BorderTile::new(None, 7709, 0x5B0E, false), // bottom
        BorderTile::new(None, 7656, 0x5C0E, false), // left
        BorderTile::new(None, 7710, 0x5D0E, false), // right
        BorderTile::new(None, 7661, 0x5E0E, false), // corner, top-left
        BorderTile::new(None, 7662, 0x5F0E, false), // corner, top-right
        BorderTile::new(None, 7664, 0x600E, false), // corner, bottom-right
        BorderTile::new(None, 7663, 0x610E, false), // corner, bottom-left
        BorderTile::new(None, 7657, 0x620E, false), // edge, top-left
        BorderTile::new(None, 7658, 0x630E, false), // edge, top-right
        BorderTile::new(None, 7660, 0x640E, false), // edge, bottom-right
        BorderTile::new(None, 7659, 0x650E, false), // edge, bottom-left

        // Grass borders on sand (tile id 104).
        BorderTile::new(Some(104), 7653, 0x140A, true), // top
        BorderTile::new(Some(104), 7709, 0x150A, true), // bottom
        BorderTile::new(Some(104), 7656, 0x160A, true), // left
        BorderTile::new(Some(104), 7710, 0x170A, true), // right
        BorderTile::new(Some(104), 7661, 0x180A, true), // corner, top-left
        BorderTile::new(Some(104), 7662, 0x190A, true), // corner, top-right
        BorderTile::new(Some(104), 7664, 0x1A0A, true), // corner, bottom-right
        BorderTile::new(Some(104), 7663, 0x1B0A, true), // corner, bottom-left
        BorderTile::new(Some(104), 7657, 0x1C0A, true), // edge, top-left
        BorderTile::new(Some(104), 7658, 0x1D0A, true), // edge, top-right
        BorderTile::new(Some(104), 7660, 0x1E0A, true), // edge, bottom-right
        BorderTile::new(Some(104), 7659, 0x1F0A, true), // edge, bottom-left

        // Grass borders on gravel (tile id 4566).
        BorderTile::new(Some(4566), 7653, 0xB30A, true), // top
        BorderTile::new(Some(4566), 7709, 0xB40A, true), // bottom
        BorderTile::new(Some(4566), 7656, 0xB50A, true), // left
        BorderTile::new(Some(4566), 7710, 0xB60A, true), // right
        BorderTile::new(Some(4566), 7661, 0xB70A, true), // corner, top-left
        BorderTile::new(Some(4566), 7662, 0xB80A, true), // corner, top-right
        BorderTile::new(Some(4566), 7664, 0xB90A, true), // corner, bottom-right
        BorderTile::new(Some(4566), 7663, 0xBA0A, true), // corner, bottom-left
        BorderTile::new(Some(4566), 7657, 0xBB0A, true), // edge, top-left
        BorderTile::new(Some(4566), 7658, 0xBC0A, true), // edge, top-right
        BorderTile::new(Some(4566), 7660, 0xBD0A, true), // edge, bottom-right
        BorderTile::new(Some(4566), 7659, 0xBE0A, true), // edge, bottom-left

        // Grass borders on dirt floor (tile id 351).
        BorderTile::new(Some(351), 7653, 0x080A, true), // top
        BorderTile::new(Some(351), 7709, 0x090A, true), // bottom
        BorderTile::new(Some(351), 7656, 0x0A0A, true), // left
        BorderTile::new(Some(351), 7710, 0x0B0A, true), // right
        BorderTile::new(Some(351), 7661, 0x0C0A, true), // corner, top-left
        BorderTile::new(Some(351), 7662, 0x0D0A, true), // corner, top-right
        BorderTile::new(Some(351), 7664, 0x0E0A, true), // corner, bottom-right
        BorderTile::new(Some(351), 7663, 0x0F0A, true), // corner, bottom-left
        BorderTile::new(Some(351), 7657, 0x100A, true), // edge, top-left
        BorderTile::new(Some(351), 7658, 0x110A, true), // edge, top-right
        BorderTile::new(Some(351), 7660, 0x120A, true), // edge, bottom-right
        BorderTile::new(Some(351), 7659, 0x130A, true), // edge, bottom-left

        // Grass borders on rock soil (tile id 4405).
        BorderTile::new(Some(4405), 7653, 0xA60A, true), // top
        BorderTile::new(Some(4405), 7709, 0xA70A, true), // bottom
        BorderTile::new(Some(4405), 7656, 0xA80A, true), // left
        BorderTile::new(Some(4405), 7710, 0xA90A, true), // right
        BorderTile::new(Some(4405), 7661, 0xAA0A, true), // corner, top-left
        BorderTile::new(Some(4405), 7662, 0xAB0A, true), // corner, top-right
        BorderTile::new(Some(4405), 7664, 0xAC0A, true), // corner, bottom-right
        BorderTile::new(Some(4405), 7663, 0xAD0A, true), // corner, bottom-left
        BorderTile::new(Some(4405), 7657, 0xAE0A, true), // edge, top-left
        BorderTile::new(Some(4405), 7658, 0xAF0A, true), // edge, top-right
        BorderTile::new(Some(4405), 7660, 0xB00A, true), // edge, bottom-right
        BorderTile::new(Some(4405), 7659, 0xB10A, true), // edge, bottom-left
    ]
}
