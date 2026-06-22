//! The game world - central message hub between all connected player tasks.
//!
//! [`World`] owns a single async message loop that receives [`PlayerToWorld`] events
//! from every connection and fans responses back out via per-player [`WorldToPlayer`]
//! senders. All shared player and sender state lives inside that loop, so no additional
//! locking is needed beyond the [`RwLock`] used to hand off the receiver at startup.

pub mod message;

use crate::{
    chat::ChatType,
    debug_log,
    player::Player,
};
use message::{PlayerToWorld, WorldToPlayer};
use std::{collections::BTreeMap, sync::Arc};
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        RwLock,
    },
    task,
};

/// The game world, holding the channel used to send events into the world loop.
///
/// Construct with [`World::new`], then call [`World::start`] to spawn the loop,
/// and [`World::sender`] to get a handle for sending messages into it.
pub struct World {
    sender:   UnboundedSender<PlayerToWorld>,
    receiver: UnboundedReceiver<PlayerToWorld>,
}

impl World {
    /// Creates a new `World` and its internal message channel, wrapped in an `Arc<RwLock>`.
    pub fn new() -> Arc<RwLock<Self>> {
        let (sender, receiver) = unbounded_channel();
        Arc::new(RwLock::new(Self { sender, receiver }))
    }

    /// Returns a cloned sender that any task can use to post events to the world loop.
    pub fn sender(&self) -> UnboundedSender<PlayerToWorld> {
        self.sender.clone()
    }

    /// Spawns the world message loop as a background task.
    pub fn start(world: &Arc<RwLock<Self>>) {
        task::spawn(Self::message_loop(world.clone()));
    }

    /// The main world loop - receives [`PlayerToWorld`] messages and dispatches responses.
    ///
    /// All online player state (`players`, `senders`) is local to this loop, making it
    /// the single source of truth for who is connected and where they are.
    async fn message_loop(world: Arc<RwLock<Self>>) {
        let mut senders: BTreeMap<u32, UnboundedSender<WorldToPlayer>> = BTreeMap::new();
        let mut players: BTreeMap<u32, Player> = BTreeMap::new();

        loop {
            let msg = world.write().await.receiver.recv().await;
            match msg {
                // Player login
                //
                // Register the player and send them the current player list, then notify
                // everyone else that a new player has joined.
                Some(PlayerToWorld::Login(player, sender)) => {
                    debug_log!("Player {} (id={}) has logged in.", player.name, player.id);

                    // Send the newly connected player the current list of everyone already online.
                    let others: Vec<Player> = players.values().cloned().collect();
                    let _ = sender.send(WorldToPlayer::PlayerList(others));
                    players.insert(player.id, player.clone());
                    senders.insert(player.id, sender);

                    // Tell everyone else that someone logged in.
                    let all: Vec<Player> = players.values().cloned().collect();
                    for (id, tx) in &senders {
                        if *id != player.id {
                            let _ = tx.send(WorldToPlayer::PlayerList(all.clone()));
                        }
                    }
                }

                // Player logout
                //
                // Remove the player and refresh the player list for everyone still online.
                Some(PlayerToWorld::Logout(player)) => {
                    debug_log!("Player {} (id={}) has logged out.", player.name, player.id);
                    
                    players.remove(&player.id);
                    senders.remove(&player.id);

                    // Refresh the player list for all remaining players.
                    let others: Vec<Player> = players.values().cloned().collect();
                    for tx in senders.values() {
                        let _ = tx.send(WorldToPlayer::PlayerList(others.clone()));
                    }
                }

                // Player walked
                // 
                // Update the player's stored position and notify all players within viewport range.
                Some(PlayerToWorld::UpdatePosition(player)) => {
                    players.insert(player.id, player.clone());
                    let all: Vec<Player> = players.values().cloned().collect();

                    for (id, tx) in &senders {
                        if let Some(viewer) = players.get(id) {
                            let dx = (viewer.position.x as i32 - player.position.x as i32).abs();
                            let dy = (viewer.position.y as i32 - player.position.y as i32).abs();
                            if dx <= 18 && dy <= 14 {
                                let _ = tx.send(WorldToPlayer::PlayerList(all.clone()));
                            }
                        }
                    }
                }

                // Chat messages
                // 
                // Route the chat message to players based on chat type and distance to sender.
                Some(PlayerToWorld::Chat(pos, chat_type, encoded, name, receiver)) => {
                    match chat_type {
                        // Private - delivered only to the named receiver, regardless of
                        // distance, and never to anyone else (including the sender).
                        // Self-messages and offline receivers are rejected with a status
                        // message sent back to the sender only.
                        ChatType::Private => {
                            let receiver_name = receiver.unwrap_or_default();

                            let Some(sender_tx) = players
                                .iter()
                                .find(|(_, p)| p.name == name)
                                .and_then(|(id, _)| senders.get(id))
                            else {
                                continue;
                            };

                            if receiver_name.eq_ignore_ascii_case(&name) {
                                let _ = sender_tx.send(WorldToPlayer::StatusMessage(
                                    "You cannot send a private message to yourself.".to_string(),
                                ));
                                continue;
                            }

                            let target = players
                                .iter()
                                .find(|(_, p)| p.name.eq_ignore_ascii_case(&receiver_name))
                                .map(|(id, _)| *id);

                            match target.and_then(|id| senders.get(&id)) {
                                Some(tx) => {
                                    let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded, name));
                                }
                                None => {
                                    let _ = sender_tx.send(WorldToPlayer::StatusMessage(
                                        "A player with this name is not online.".to_string(),
                                    ));
                                }
                            }
                        }

                        _ => {
                            for (id, tx) in &senders {
                                if let Some(viewer) = players.get(id) {
                                    // Get the viewer's position on the map.
                                    let dx = (viewer.position.x as i32 - pos.x as i32).abs();
                                    let dy = (viewer.position.y as i32 - pos.y as i32).abs();

                                    match chat_type {
                                        // Normal - visible to players within the screen viewport.
                                        ChatType::Normal => {
                                            if dx <= 6 && dy <= 4 {
                                                let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                            }
                                        }

                                        // Whisper - full message within 2 tiles; "pspsps" to players further away.
                                        ChatType::Whisper => {
                                            if dx <= 2 && dy <= 2 {
                                                let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                            } else if dx <= 14 && dy <= 11 {
                                                let pspsps = crate::chat::encoding::translate("pspsps");
                                                let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, pspsps, name.clone()));
                                            }
                                        }

                                        // Yell - audible up to 32 tiles in any direction.
                                        ChatType::Yell => {
                                            if dx <= 32 && dy <= 32 {
                                                let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                            }
                                        }

                                        // Broadcast - delivered to every connected player regardless of position.
                                        ChatType::Broadcast => {
                                            let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                        }

                                        // Look - delivered only to the player who triggered the look action.
                                        ChatType::Look => {
                                            if let Some(sender_id) = players.iter().find(|(_, p)| p.name == name).map(|(id, _)| *id) {
                                                if *id == sender_id {
                                                    let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                                }
                                            }
                                        }

                                        // Handled above, before distance-based routing.
                                        ChatType::Private => {}
                                    }
                                }
                            }
                        }
                    }
                }

                // Update the player's stored info and broadcast the refreshed list to everyone.
                Some(PlayerToWorld::UpdateInfo(player)) => {
                    players.insert(player.id, player.clone());
                    let all: Vec<Player> = players.values().cloned().collect();
                    for tx in senders.values() {
                        let _ = tx.send(WorldToPlayer::PlayerList(all.clone()));
                    }
                }

                // Notify all players within viewport range that a tile has changed.
                Some(PlayerToWorld::MapUpdate(pos)) => {
                    for (id, tx) in &senders {
                        if let Some(viewer) = players.get(id) {
                            let dx = (viewer.position.x as i32 - pos.x as i32).abs();
                            let dy = (viewer.position.y as i32 - pos.y as i32).abs();
                            if dx <= 14 && dy <= 11 {
                                let _ = tx.send(WorldToPlayer::MapUpdate);
                            }
                        }
                    }
                }

                Some(PlayerToWorld::ContainerUpdate(pos, container_id, items)) => {
                    // Notify all players who have this container open (same map pos + container_id).
                    // We send them the updated item list via a WorldToPlayer message.
                    for (id, tx) in &senders {
                        let _ = tx.send(WorldToPlayer::ContainerUpdate(pos, container_id, items.clone()));
                    }
                }

                // Channel closed - all senders have been dropped, shut down the loop.
                None => break,
            }
        }
    }
}
