//! Item metadata for Tibia 1.03 - names, walkability, container flags, and movement properties.

/// Maps a Tibia 1.03 item id to its display name, shown when a player looks at an item.
///
/// Returns `"unknown"` for any id not in the table.
pub fn item_name(id: u16) -> &'static str {
    match id {
        283     => "a drawbridge",
        90      => "a sword",
        574     => "a bottle",
        105     => "a baking tray",
        391     => "a lump of dough",
        135     => "flour",
        134     => "bread",
        534     => "an archway",
        790     => "an archway",
        20      => "a brick wall",
        276     => "a brick wall",
        532     => "a brick wall",
        1556    => "a brick wall",
        524     => "a passthrough",
        163     => "a bush",
        928     => "a willow",
        778     => "gravel",
        811     => "a chest",
        266     => "dirt floor",
        1069    => "a beer cask",
        12      => "wooden floor",
        114     => "a coal basin",
        268     => "white marble floor",
        10      => "grass",
        102     => "an anvil",
        317     => "a bag",
        1325    => "a barrel",
        2881    => "a candlestick",
        7180    => "a stone tile",
        51      => "a lever",
        307     => "a lever",
        1581    => "a trough of water",
        1837    => "a trough",
        5140    => "a framework wall",
        4116    => "a framework wall",
        4372    => "a framework wall",
        4884    => "a framework wall",
        93      => "a steel shield",
        44      => "a bed",
        300     => "a bed",
        35      => "a knight statue",
        522     => "sand",
        1290    => "rock soil",
        14      => "water",
        873     => "a pot",
        602     => "a battle axe",
        3418    => "a spear",
        346     => "a two-handed sword",
        4618    => "dirt floor",
        45578   => "dirt floor",
        1       => "void",
        642     => "meat",
        1834    => "a chest of drawers",
        29      => "a fountain",
        285     => "a fountain",
        541     => "a fountain",
        797     => "a fountain",
        119     => "a black token",
        375     => "a white token",
        19      => "a chess board",
        275     => "a chess board",
        10003   => "a mill board",
        10259   => "a mill board",
        1043    => "a mill board",
        10515   => "a mill board",
        10771   => "a mill board",
        11027   => "a mill board",
        11283   => "a mill board",
        11539   => "a mill board",
        11795   => "a mill board",
        12051   => "a mill board",
        12307   => "a mill board",
        12563   => "a mill board",
        12819   => "a mill board",
        1299    => "a mill board",
        1555    => "a mill board",
        1811    => "a mill board",
        2067    => "a mill board",
        2323    => "a mill board",
        2579    => "a mill board",
        2835    => "a mill board",
        3091    => "a mill board",
        3347    => "a mill board",
        3603    => "a mill board",
        3859    => "a mill board",
        4115    => "a mill board",
        4371    => "a mill board",
        4627    => "a mill board",
        4883    => "a mill board",
        5139    => "a mill board",
        531     => "a mill board",
        5395    => "a mill board",
        5651    => "a mill board",
        5907    => "a mill board",
        6163    => "a mill board",
        6419    => "a mill board",
        6675    => "a mill board",
        6931    => "a mill board",
        7187    => "a mill board",
        7443    => "a mill board",
        7699    => "a mill board",
        787     => "a mill board",
        7955    => "a mill board",
        8211    => "a mill board",
        8467    => "a mill board",
        8723    => "a mill board",
        8979    => "a mill board",
        9235    => "a mill board",
        9491    => "a mill board",
        9747    => "a mill board",
        631     => "a black pawn",
        887     => "a black castle",
        1143    => "a black knight",
        1399    => "a black bishop",
        1655    => "a black queen",
        1911    => "a black king",
        2167    => "a white pawn",
        2423    => "a white castle",
        2679    => "a white knight",
        2935    => "a white bishop",
        3191    => "a white queen",
        3447    => "a white king",
        13075   => "a tic-tac-toe board",
        13331   => "a tic-tac-toe board",
        13587   => "a tic-tac-toe board",
        13843   => "a tic-tac-toe board",
        14099   => "a tic-tac-toe board",
        14355   => "a tic-tac-toe board",
        14611   => "a tic-tac-toe board",
        14867   => "a tic-tac-toe board",
        15123   => "a tic-tac-toe board",
        3703    => "a tic-tac-toe token",
        3959    => "a tic-tac-toe token",
        // Various kinds of grass borders
        2058    => "grass",
        23054   => "grass",
        2314    => "grass",
        23310   => "grass",
        23566   => "grass",
        23822   => "grass",
        24078   => "grass",
        24334   => "grass",
        24590   => "grass",
        24846   => "grass",
        25102   => "grass",
        25358   => "grass",
        25614   => "grass",
        2570    => "grass",
        25870   => "grass",
        2826    => "grass",
        3082    => "grass",
        3338    => "grass",
        3594    => "grass",
        3850    => "grass",
        4106    => "grass",
        4160    => "grass",
        42506   => "grass",
        42762   => "grass",
        43018   => "grass",
        43274   => "grass",
        43530   => "grass",
        4362    => "grass",
        43786   => "grass",
        44042   => "grass",
        44298   => "grass",
        44554   => "grass",
        44810   => "grass",
        45066   => "grass",
        45322   => "grass",
        45834   => "grass",
        46090   => "grass",
        46346   => "grass",
        46602   => "grass",
        46858   => "grass",
        47114   => "grass",
        47370   => "grass",
        47626   => "grass",
        47882   => "grass",
        48138   => "grass",
        48394   => "grass",
        48650   => "grass",
        4874    => "grass",
        5130    => "grass",
        5386    => "grass",
        5642    => "grass",
        5898    => "grass",
        6154    => "grass",
        6410    => "grass",
        6666    => "grass",
        6922    => "grass",
        7178    => "grass",
        7434    => "grass",
        7690    => "grass",
        7946    => "grass",
        _       => "unknown",
    }
}

/// Returns `false` for tiles and items that block movement, used by the auto-walk pathfinder.
pub fn is_walkable(id: u16) -> bool {
    match id {
        20      => false, // a brick wall
        276     => false, // a brick wall
        532     => false, // a brick wall
        1556    => false, // a brick wall
        163     => false, // a bush
        928     => false, // a willow
        811     => false, // a chest
        1069    => false, // a beer cask
        114     => false, // a coal basin
        102     => false, // an anvil
        1325    => false, // a barrel
        1581    => false, // a trough of water
        1837    => false, // a trough
        4116    => false, // a framework wall
        4372    => false, // a framework wall
        4884    => false, // a framework wall
        5140    => false, // a framework wall
        44      => false, // a bed
        300     => false, // a bed
        35      => false, // a knight statue
        14      => false, // water
        873     => false, // a pot
        1       => false, // void
        1834    => false, // chest of drawers
        29      => false, // fountain
        285     => false, // fountain
        541     => false, // fountain
        797     => false, // fountain
        23054   => false, // grass border (water)
        23566   => false, // grass border (water)
        24078   => false, // grass border (water)
        25102   => false, // grass border (water)
        25358   => false, // grass border (water)
        25870   => false, // grass border (water)
        _       => true,
    }
}

/// Returns `true` if the item is a container that players can open and store items in.
pub fn is_container(id: u16) -> bool {
    match id {
        811     => true, // a chest
        317     => true, // a bag
        1325    => true, // a barrel
        1834    => true, // a chest of drawers
        _       => false,
    }
}

/// Physical interaction properties for a Tibia 1.03 item.
///
/// Controls whether the item can be moved, how far it can be moved
/// from the player's position, and whether it blocks projectiles.
pub struct ItemProperties {
    pub movable: bool,
    pub range: u8,
    pub blocks_projectiles: bool,
}

impl ItemProperties {
    /// An immovable item that also blocks projectiles (e.g. a brick wall).
    const fn wall() -> Self {
        Self { movable: false, range: 0, blocks_projectiles: true }
    }

    /// An immovable item that does not block projectiles (e.g. an archway).
    const fn fixed() -> Self {
        Self { movable: false, range: 0, blocks_projectiles: false }
    }

    /// A movable item with the given throw range (in tiles).
    const fn new(range: u8) -> Self {
        Self { movable: range > 0, range, blocks_projectiles: false }
    }
}

/// Returns the physical interaction properties for a given item id.
pub fn item_properties(id: u16) -> ItemProperties {
    match id {
        // Immovable - blocks projectiles
        20      => ItemProperties::wall(), // a brick wall
        276     => ItemProperties::wall(), // a brick wall
        532     => ItemProperties::wall(), // a brick wall
        1556    => ItemProperties::wall(), // a brick wall
        5140    => ItemProperties::wall(), // a framework wall
        4116    => ItemProperties::wall(), // a framework wall
        4372    => ItemProperties::wall(), // a framework wall
        4884    => ItemProperties::wall(), // a framework wall
        1       => ItemProperties::wall(), // void
        // Immovable - does not block projectiles
        283     => ItemProperties::fixed(), // a drawbridge
        534     => ItemProperties::fixed(), // an archway
        790     => ItemProperties::fixed(), // an archway
        524     => ItemProperties::fixed(), // a passthrough
        163     => ItemProperties::fixed(), // a bush
        928     => ItemProperties::fixed(), // a willow
        778     => ItemProperties::fixed(), // gravel
        266     => ItemProperties::fixed(), // dirt floor
        1069    => ItemProperties::fixed(), // a beer cask
        12      => ItemProperties::fixed(), // wooden floor
        114     => ItemProperties::fixed(), // a coal basin
        268     => ItemProperties::fixed(), // white marble floor
        10      => ItemProperties::fixed(), // grass
        102     => ItemProperties::fixed(), // an anvil
        7180    => ItemProperties::fixed(), // a stone tile
        51      => ItemProperties::fixed(), // a lever
        307     => ItemProperties::fixed(), // a lever
        44      => ItemProperties::fixed(), // a bed
        300     => ItemProperties::fixed(), // a bed
        35      => ItemProperties::fixed(), // a knight statue
        522     => ItemProperties::fixed(), // sand
        1290    => ItemProperties::fixed(), // rock soil
        14      => ItemProperties::fixed(), // water
        4618    => ItemProperties::fixed(), // dirt floor
        45578   => ItemProperties::fixed(), // dirt floor
        29      => ItemProperties::fixed(), // a fountain
        285     => ItemProperties::fixed(), // a fountain
        541     => ItemProperties::fixed(), // a fountain
        797     => ItemProperties::fixed(), // a fountain
        19      => ItemProperties::fixed(), // a chess board
        275     => ItemProperties::fixed(), // a chess board
        10003   => ItemProperties::fixed(), // a mill board
        10259   => ItemProperties::fixed(), // a mill board
        1043    => ItemProperties::fixed(), // a mill board
        10515   => ItemProperties::fixed(), // a mill board
        10771   => ItemProperties::fixed(), // a mill board
        11027   => ItemProperties::fixed(), // a mill board
        11283   => ItemProperties::fixed(), // a mill board
        11539   => ItemProperties::fixed(), // a mill board
        11795   => ItemProperties::fixed(), // a mill board
        12051   => ItemProperties::fixed(), // a mill board
        12307   => ItemProperties::fixed(), // a mill board
        12563   => ItemProperties::fixed(), // a mill board
        12819   => ItemProperties::fixed(), // a mill board
        1299    => ItemProperties::fixed(), // a mill board
        1555    => ItemProperties::fixed(), // a mill board
        1811    => ItemProperties::fixed(), // a mill board
        2067    => ItemProperties::fixed(), // a mill board
        2323    => ItemProperties::fixed(), // a mill board
        2579    => ItemProperties::fixed(), // a mill board
        2835    => ItemProperties::fixed(), // a mill board
        3091    => ItemProperties::fixed(), // a mill board
        3347    => ItemProperties::fixed(), // a mill board
        3603    => ItemProperties::fixed(), // a mill board
        3859    => ItemProperties::fixed(), // a mill board
        4115    => ItemProperties::fixed(), // a mill board
        4371    => ItemProperties::fixed(), // a mill board
        4627    => ItemProperties::fixed(), // a mill board
        4883    => ItemProperties::fixed(), // a mill board
        5139    => ItemProperties::fixed(), // a mill board
        531     => ItemProperties::fixed(), // a mill board
        5395    => ItemProperties::fixed(), // a mill board
        5651    => ItemProperties::fixed(), // a mill board
        5907    => ItemProperties::fixed(), // a mill board
        6163    => ItemProperties::fixed(), // a mill board
        6419    => ItemProperties::fixed(), // a mill board
        6675    => ItemProperties::fixed(), // a mill board
        6931    => ItemProperties::fixed(), // a mill board
        7187    => ItemProperties::fixed(), // a mill board
        7443    => ItemProperties::fixed(), // a mill board
        7699    => ItemProperties::fixed(), // a mill board
        787     => ItemProperties::fixed(), // a mill board
        7955    => ItemProperties::fixed(), // a mill board
        8211    => ItemProperties::fixed(), // a mill board
        8467    => ItemProperties::fixed(), // a mill board
        8723    => ItemProperties::fixed(), // a mill board
        8979    => ItemProperties::fixed(), // a mill board
        9235    => ItemProperties::fixed(), // a mill board
        9491    => ItemProperties::fixed(), // a mill board
        9747    => ItemProperties::fixed(), // a mill board
        13075   => ItemProperties::fixed(), // a tic-tac-toe board
        13331   => ItemProperties::fixed(), // a tic-tac-toe board
        13587   => ItemProperties::fixed(), // a tic-tac-toe board
        13843   => ItemProperties::fixed(), // a tic-tac-toe board
        14099   => ItemProperties::fixed(), // a tic-tac-toe board
        14355   => ItemProperties::fixed(), // a tic-tac-toe board
        14611   => ItemProperties::fixed(), // a tic-tac-toe board
        14867   => ItemProperties::fixed(), // a tic-tac-toe board
        15123   => ItemProperties::fixed(), // a tic-tac-toe board
        2058    => ItemProperties::fixed(), // grass (border)
        23054   => ItemProperties::fixed(), // grass (border)
        2314    => ItemProperties::fixed(), // grass (border)
        23310   => ItemProperties::fixed(), // grass (border)
        23566   => ItemProperties::fixed(), // grass (border)
        23822   => ItemProperties::fixed(), // grass (border)
        24078   => ItemProperties::fixed(), // grass (border)
        24334   => ItemProperties::fixed(), // grass (border)
        24590   => ItemProperties::fixed(), // grass (border)
        24846   => ItemProperties::fixed(), // grass (border)
        25102   => ItemProperties::fixed(), // grass (border)
        25358   => ItemProperties::fixed(), // grass (border)
        25614   => ItemProperties::fixed(), // grass (border)
        2570    => ItemProperties::fixed(), // grass (border)
        25870   => ItemProperties::fixed(), // grass (border)
        2826    => ItemProperties::fixed(), // grass (border)
        3082    => ItemProperties::fixed(), // grass (border)
        3338    => ItemProperties::fixed(), // grass (border)
        3594    => ItemProperties::fixed(), // grass (border)
        3850    => ItemProperties::fixed(), // grass (border)
        4106    => ItemProperties::fixed(), // grass (border)
        4160    => ItemProperties::fixed(), // grass (border)
        42506   => ItemProperties::fixed(), // grass (border)
        42762   => ItemProperties::fixed(), // grass (border)
        43018   => ItemProperties::fixed(), // grass (border)
        43274   => ItemProperties::fixed(), // grass (border)
        43530   => ItemProperties::fixed(), // grass (border)
        4362    => ItemProperties::fixed(), // grass (border)
        43786   => ItemProperties::fixed(), // grass (border)
        44042   => ItemProperties::fixed(), // grass (border)
        44298   => ItemProperties::fixed(), // grass (border)
        44554   => ItemProperties::fixed(), // grass (border)
        44810   => ItemProperties::fixed(), // grass (border)
        45066   => ItemProperties::fixed(), // grass (border)
        45322   => ItemProperties::fixed(), // grass (border)
        45834   => ItemProperties::fixed(), // grass (border)
        46090   => ItemProperties::fixed(), // grass (border)
        46346   => ItemProperties::fixed(), // grass (border)
        46602   => ItemProperties::fixed(), // grass (border)
        46858   => ItemProperties::fixed(), // grass (border)
        47114   => ItemProperties::fixed(), // grass (border)
        47370   => ItemProperties::fixed(), // grass (border)
        47626   => ItemProperties::fixed(), // grass (border)
        47882   => ItemProperties::fixed(), // grass (border)
        48138   => ItemProperties::fixed(), // grass (border)
        48394   => ItemProperties::fixed(), // grass (border)
        48650   => ItemProperties::fixed(), // grass (border)
        4874    => ItemProperties::fixed(), // grass (border)
        5130    => ItemProperties::fixed(), // grass (border)
        5386    => ItemProperties::fixed(), // grass (border)
        5642    => ItemProperties::fixed(), // grass (border)
        5898    => ItemProperties::fixed(), // grass (border)
        6154    => ItemProperties::fixed(), // grass (border)
        6410    => ItemProperties::fixed(), // grass (border)
        6666    => ItemProperties::fixed(), // grass (border)
        6922    => ItemProperties::fixed(), // grass (border)
        7178    => ItemProperties::fixed(), // grass (border)
        7434    => ItemProperties::fixed(), // grass (border)
        7690    => ItemProperties::fixed(), // grass (border)
        7946    => ItemProperties::fixed(), // grass (border)
        // Short range (2 sqm)
        1325    => ItemProperties::new(2), // a barrel
        1581    => ItemProperties::new(2), // a trough of water
        1837    => ItemProperties::new(2), // a trough
        1834    => ItemProperties::new(2), // a chest of drawers
        // Long range (8 sqm)
        90      => ItemProperties::new(8), // a sword
        574     => ItemProperties::new(8), // a bottle
        105     => ItemProperties::new(8), // a baking tray
        391     => ItemProperties::new(8), // a lump of dough
        135     => ItemProperties::new(8), // flour
        134     => ItemProperties::new(8), // bread
        811     => ItemProperties::new(8), // a chest
        317     => ItemProperties::new(8), // a bag
        2881    => ItemProperties::new(8), // a candlestick
        93      => ItemProperties::new(8), // a steel shield
        873     => ItemProperties::new(8), // a pot
        602     => ItemProperties::new(8), // a battle axe
        3418    => ItemProperties::new(8), // a spear
        346     => ItemProperties::new(8), // a two-handed sword
        642     => ItemProperties::new(8), // meat
        119     => ItemProperties::new(8), // a black token
        375     => ItemProperties::new(8), // a white token
        631     => ItemProperties::new(8), // a black pawn
        887     => ItemProperties::new(8), // a black castle
        1143    => ItemProperties::new(8), // a black knight
        1399    => ItemProperties::new(8), // a black bishop
        1655    => ItemProperties::new(8), // a black queen
        1911    => ItemProperties::new(8), // a black king
        2167    => ItemProperties::new(8), // a white pawn
        2423    => ItemProperties::new(8), // a white castle
        2679    => ItemProperties::new(8), // a white knight
        2935    => ItemProperties::new(8), // a white bishop
        3191    => ItemProperties::new(8), // a white queen
        3447    => ItemProperties::new(8), // a white king
        3703    => ItemProperties::new(8), // a tic-tac-toe token
        3959    => ItemProperties::new(8), // a tic-tac-toe token
        _       => ItemProperties::fixed(),
    }
}
