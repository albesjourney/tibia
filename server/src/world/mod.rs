pub mod message;

use crate::{
    debug_log,
    player::Player,
    chat::ChatType,
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


/********************************************************************************
 * 
 * World state
 * 
 * Receives messages from all connected players and distributes messages back
 * to them. This acts as the central communication hub between connections.
 * 
 ********************************************************************************/
pub struct World {
    sender:   UnboundedSender<PlayerToWorld>,
    receiver: UnboundedReceiver<PlayerToWorld>,
}


impl World {
    /********************************************************************************
     * 
     * Create a new world instance and communication channel.
     * 
     ********************************************************************************/
    pub fn new() -> Arc<RwLock<Self>> {
        let (sender, receiver) = unbounded_channel();
        Arc::new(RwLock::new(Self { sender, receiver }))
    }


    /********************************************************************************
     * 
     * Get a sender that can be used to send messages to the world.
     * 
     ********************************************************************************/
    pub fn sender(&self) -> UnboundedSender<PlayerToWorld> {
        self.sender.clone()
    }


    /********************************************************************************
     * 
     * Start the world message processing loop.
     * 
     ********************************************************************************/
    pub fn start(world: &Arc<RwLock<Self>>) {
        task::spawn(Self::message_loop(world.clone()));
    }


    /********************************************************************************
     * 
     * Main world message loop.
     * 
     ********************************************************************************/
    async fn message_loop(world: Arc<RwLock<Self>>) {
        let mut senders: BTreeMap<u32, UnboundedSender<WorldToPlayer>> = BTreeMap::new();
        let mut players: BTreeMap<u32, Player> = BTreeMap::new();

        loop {
            let msg = world.write().await.receiver.recv().await;
            match msg {
                /********************************************************************************
                 * 
                 * Player login
                 * 
                 * Register the player's communication channel so the world can send messages back
                 * to that player.
                 * 
                 ********************************************************************************/
                Some(PlayerToWorld::Login(player, sender)) => {
                    debug_log!("Player {} (id={}) has logged in.", player.name, player.id);
                    // Print the entire Player object (verbose logging)
                    // debug_log!("Player logged in:\n{:#?}", player);

                    /********************************************************************************
                     * 
                     * Send the newly connected player the current list of everyone already online.
                     * 
                     ********************************************************************************/
                    let others: Vec<Player> = players.values().cloned().collect();
                    let _ = sender.send(WorldToPlayer::PlayerList(others));
                    players.insert(player.id, player.clone());
                    senders.insert(player.id, sender);


                    /********************************************************************************
                     * 
                     * Tell everyone else that someone logged in.
                     * 
                     ********************************************************************************/
                    let all: Vec<Player> = players.values().cloned().collect();
                    for (id, tx) in &senders {
                        if *id != player.id {
                            let _ = tx.send(WorldToPlayer::PlayerList(all.clone()));
                        }
                    }
                }


                /********************************************************************************
                 * 
                 * Player logout
                 * 
                 * Remove the player's communication channel from the world.
                 * 
                 ********************************************************************************/
                Some(PlayerToWorld::Logout(player)) => {
                    debug_log!("Player {} (id={}) has logged out.", player.name, player.id);
                    players.remove(&player.id);
                    senders.remove(&player.id);

                    /********************************************************************************
                     * 
                     * Refresh the player list for all remaining players.
                     * 
                     ********************************************************************************/
                    let others: Vec<Player> = players.values().cloned().collect();
                    for tx in senders.values() {
                        let _ = tx.send(WorldToPlayer::PlayerList(others.clone()));
                    }
                }


                /********************************************************************************
                 * 
                 * Player walked
                 * 
                 * Update the position of players as they move around on the map.
                 * This broadcasts to all players nearby that the player walked.
                 * 
                 ********************************************************************************/
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


                /********************************************************************************
                 * 
                 * Process chat messages.
                 * This determines who receives the message and what they see, based on the chat type
                 * and distance to the sender.
                 * 
                 ********************************************************************************/
                Some(PlayerToWorld::Chat(pos, chat_type, encoded, name)) => {
                    for (id, tx) in &senders {
                        if let Some(viewer) = players.get(id) {
                            /********************************************************************************
                             * 
                             * Get the viewer's position on the map.
                             * 
                             ********************************************************************************/
                            let dx = (viewer.position.x as i32 - pos.x as i32).abs();
                            let dy = (viewer.position.y as i32 - pos.y as i32).abs();

                            match chat_type {
                                /********************************************************************************
                                 * 
                                 * Normal chat message - only visible if the sender is on screen (within viewport).
                                 * 
                                 ********************************************************************************/
                                ChatType::Normal => {
                                    if dx <= 14 && dy <= 11 {
                                        let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                    }
                                }


                                /********************************************************************************
                                 * 
                                 * Whisper - Only visible to players within 2 squares. Players further away see "pspsps".
                                 * 
                                 ********************************************************************************/
                                ChatType::Whisper => {
                                    if dx <= 2 && dy <= 2 {
                                        let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                    } else if dx <= 14 && dy <= 11 {
                                        let pspsps = crate::chat::encoding::translate("pspsps");
                                        let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, pspsps, name.clone()));
                                    }
                                }


                                /********************************************************************************
                                 * 
                                 * Yelling - Audible up to 32 squares away for all players.
                                 * 
                                 ********************************************************************************/
                                ChatType::Yell => {
                                    if dx <= 32 && dy <= 32 {
                                        let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                    }
                                }


                                /********************************************************************************
                                 * 
                                 * Broadcast - Everyone on the server can see this.
                                 * 
                                 ********************************************************************************/
                                ChatType::Broadcast => {
                                    let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                }


                                /********************************************************************************
                                 * 
                                 * Looking - Only the sender sees these, handled client-side.
                                 * 
                                 ********************************************************************************/
                                ChatType::Look => {
                                    // if *id == players.iter().find(|(_, p)| p.name == name).map(|(id, _)| *id).unwrap_or(0) {
                                    //     let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                    // }

                                    if let Some(sender_id) = players.iter().find(|(_, p)| p.name == name).map(|(id, _)| *id) {
                                        if *id == sender_id {
                                            let _ = tx.send(WorldToPlayer::Chat(pos, chat_type, encoded.clone(), name.clone()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }


                /********************************************************************************
                 * 
                 * Player updated their information (name, location, email, etc).
                 * 
                 ********************************************************************************/
                Some(PlayerToWorld::UpdateInfo(player)) => {
                    players.insert(player.id, player.clone());
                    let all: Vec<Player> = players.values().cloned().collect();
                    for tx in senders.values() {
                        let _ = tx.send(WorldToPlayer::PlayerList(all.clone()));
                    }
                }


                /********************************************************************************
                 * 
                 * World channel closed.
                 * 
                 ********************************************************************************/
                None => break,
            }
        }
    }
}
