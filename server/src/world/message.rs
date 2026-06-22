//! Message types for communication between player connections and the game world.
//!
//! Two channels exist: [`PlayerToWorld`] carries events from a player's connection
//! task into the world loop, and [`WorldToPlayer`] carries world events back out
//! to one or more connected players.

use crate::chat::ChatType;
use crate::map::position::Position;
use crate::player::Player;
use tokio::sync::mpsc::UnboundedSender;

/// A message sent from a player's connection task to the world loop.
#[derive(Clone, Debug)]
pub enum PlayerToWorld {
    /// A new player has authenticated and is entering the world.
    Login(Player, UnboundedSender<WorldToPlayer>),
    /// A player has disconnected and should be removed from the world.
    Logout(Player),
    /// A player has moved; broadcast their new position to nearby players.
    UpdatePosition(Player),
    /// A player's appearance or stats have changed; broadcast the update.
    UpdateInfo(Player),
    /// A player sent a chat message (position, type, encoded text, speaker name, receiver name).
    /// `receiver` is only meaningful for [`ChatType::Private`](crate::chat::ChatType::Private);
    /// it carries the name parsed out of the `*Name*` / `@Name@` prefix so the world loop can
    /// route the message to that single player instead of by map distance.
    Chat(Position, ChatType, Vec<u8>, String, Option<String>),
    /// A tile on the map has changed and nearby players should receive the update.
    MapUpdate(Position),
    /// A player added or removed an item from a map container.
    /// Other players who have that container open should have their window refreshed.
    /// Fields: map position of the container, container item id, new item list.
    ContainerUpdate(Position, u16, Vec<u16>),
}

/// A message sent from the world loop to a player's connection task.
#[derive(Clone, Debug)]
pub enum WorldToPlayer {
    /// The current list of players visible to this player.
    PlayerList(Vec<Player>),
    /// A chat message to relay to this player (position, type, encoded text, speaker name).
    Chat(Position, ChatType, Vec<u8>, String),
    /// A nearby tile has changed; the player's client should re-request map data.
    MapUpdate,
    /// A status bar message for this player's connection, e.g. a private chat error.
    /// Sent only to the one player's channel - same as any other [`WorldToPlayer`] variant,
    /// the world loop just doesn't broadcast it to anyone else.
    StatusMessage(String),
    /// A map container's contents changed; if this player has it open, refresh the window.
    ContainerUpdate(Position, u16, Vec<u16>),
}
