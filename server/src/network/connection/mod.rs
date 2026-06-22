//! Per-connection state and the login handshake for Tibia 1.03.
//!
//! Each accepted TCP connection is handed to [`Connection::handle_login`], which reads
//! the login packet, authenticates the player, registers them with the world, and returns
//! a ready-to-run [`Connection`]. The caller then drives the connection with [`Connection::run`].

mod receive;
mod send;

use crate::{
    debug_log,
    io::ReadExt,
    map::MAP,
    player::{Direction, Player},
    world::message::{PlayerToWorld, WorldToPlayer},
};
use anyhow::Result;
use crossbeam_queue::SegQueue;
use tokio::{
    io::AsyncReadExt,
    net::TcpStream,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::{timeout, Duration},
};

/// All state associated with a single connected client.
pub struct Connection {
    stream:         TcpStream,
    player:         Player,
    /// Outgoing packets queued for the next [`flush`](Connection::flush) call.
    message_queue:  SegQueue<Vec<u8>>,
    world_sender:   UnboundedSender<PlayerToWorld>,
    world_receiver: UnboundedReceiver<WorldToPlayer>,
    /// Other players currently visible to this connection.
    other_players:  Vec<Player>,
    /// Remaining steps in the current auto-walk path, processed one per loop tick.
    pending_path:   Vec<Direction>,
}

impl Connection {
    /// Creates a new connection from an authenticated stream and world channel pair.
    fn new(
        stream:         TcpStream,
        player:         Player,
        world_sender:   UnboundedSender<PlayerToWorld>,
        world_receiver: UnboundedReceiver<WorldToPlayer>,
    ) -> Self {
        Self {
            stream,
            player,
            message_queue: SegQueue::new(),
            world_sender,
            world_receiver,
            other_players: Vec::new(),
            pending_path:  Vec::new(),
        }
    }

    /// Reads and processes the login handshake from `stream`.
    ///
    /// The packet length determines the login type:
    /// - `67` bytes = existing character login ("Journey Onward")
    /// - `221` bytes = new character creation ("New Game")
    ///
    /// On success, returns `Some(conn)` with the connection ready to run.
    /// Returns `Ok(None)` if the login was rejected or the client disconnected cleanly.
    pub async fn handle_login(
        mut stream:   TcpStream,
        world_sender: UnboundedSender<PlayerToWorld>,
    ) -> Result<Option<Self>> {
        let length = stream.read_u16_le().await?;

        match length {
            // Existing player login.
            67 => {
                let player = login_existing(&mut stream).await?;
                Self::complete_login(stream, world_sender, player).await
            }

            // New character creation.
            221 => {
                let player = login_new(&mut stream).await?;
                Self::complete_login(stream, world_sender, player).await
            }

            _ => {
                debug_log!("Unknown login packet length: {}", length);
                Ok(None)
            }
        }
    }

    /// Shared post-login setup used by both login paths.
    ///
    /// Registers the player with the world, sends the login packet sequence, then waits
    /// for the initial [`WorldToPlayer::PlayerList`] so other online players appear on
    /// the map immediately when the connection's run loop begins.
    async fn complete_login(
        stream:       TcpStream,
        world_sender: UnboundedSender<PlayerToWorld>,
        player:       Option<Player>,
    ) -> Result<Option<Self>> {
        let Some(player) = player else {
            return Ok(None);
        };

        let (game_sender, world_receiver) = unbounded_channel();
        world_sender.send(PlayerToWorld::Login(player.clone(), game_sender))?;

        let mut conn = Connection::new(stream, player, world_sender, world_receiver);
        conn.send_login_sequence().await?;
        conn.flush().await?;

        // Spin until the world sends back the initial PlayerList, buffering any chat
        // messages that arrive in the meantime so they are not lost.
        loop {
            if let Ok(msg) = conn.world_receiver.try_recv() {
                match msg {
                    WorldToPlayer::PlayerList(players) => {
                        conn.other_players = players.into_iter().filter(|p| p.id != conn.player.id).collect();
                        let pos = conn.player.position;
                        let map = conn.send_map(pos, 18, 14).await?;
                        conn.enqueue(map);
                        conn.flush().await?;
                        break;
                    }

                    WorldToPlayer::Chat(sender_pos, chat_type, encoded, sender_name) => {
                        let pkt = conn.build_chat_packet(chat_type, &encoded, sender_pos, &sender_name).await?;
                        conn.enqueue(pkt);
                    }

                    WorldToPlayer::MapUpdate => {}

                    WorldToPlayer::ContainerUpdate(_, _, _) => {}

                    WorldToPlayer::StatusMessage(message) => {
                        let pkt = conn.send_status_message(&message).await?;
                        conn.enqueue(pkt);
                    }
                }
            } else {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        Ok(Some(conn))
    }

    /// Drains [`WorldToPlayer`] messages and reacts to each.
    ///
    /// Called once per iteration of the main run loop before reading incoming packets.
    async fn process_world_messages(&mut self) -> Result<()> {
        while let Ok(msg) = self.world_receiver.try_recv() {
            match msg {
                WorldToPlayer::PlayerList(players) => {
                    let new_others: Vec<Player> = players.into_iter().filter(|p| p.id != self.player.id).collect();

                    // Only redraw the map if something visible actually changed, such as:
                    // a player moved, changed outfit, appeared, or disconnected.
                    let needs_redraw = new_others.iter().any(|new| {
                        match self.other_players.iter().find(|o| o.id == new.id) {
                            None      => true, // new player appeared
                            Some(old) => old.position != new.position || old.outfit != new.outfit,
                        }
                    }) || self.other_players.len() != new_others.len(); // someone left

                    self.other_players = new_others;

                    if needs_redraw {
                        let pos = self.player.position;
                        let map = self.send_map(pos, 18, 14).await?;
                        self.enqueue(map);
                    }
                }

                WorldToPlayer::Chat(sender_pos, chat_type, encoded, sender_name) => {
                    let pkt = match chat_type {
                        // Broadcast has no dedicated message type in Tibia 1.03 - there is no
                        // red centred text equivalent. Instead we print it at the top of the screen.
                        crate::chat::ChatType::Broadcast => {
                            let top_center = crate::map::position::Position::new(
                                self.player.position.x.saturating_sub(0),
                                self.player.position.y.saturating_sub(2),
                                self.player.position.z,
                            );
                            self.build_chat_packet(chat_type, &encoded, top_center, &sender_name).await?
                        }

                        // Private messages only ever reach this player's connection when they
                        // are the intended receiver (the world loop filters everyone else out).
                        // Similar to broadcast messages, they appear at the top of the screen.
                        crate::chat::ChatType::Private => {
                            let top_center = crate::map::position::Position::new(
                                self.player.position.x.saturating_sub(0),
                                self.player.position.y.saturating_sub(2),
                                self.player.position.z,
                            );
                            self.build_chat_packet(chat_type, &encoded, top_center, &sender_name).await?
                        }

                        // All other message types are positioned relative to the sender's map position.
                        _ => {
                            self.build_chat_packet(chat_type, &encoded, sender_pos, &sender_name).await?
                        }
                    };
                    self.enqueue(pkt);
                }

                WorldToPlayer::MapUpdate => {
                    let pos = self.player.position;
                    let map = self.send_map(pos, 18, 14).await?;
                    self.enqueue(map);
                }

                WorldToPlayer::ContainerUpdate(pos, container_id, items) => {
                    // Find any open window this player has for that container at that position.
                    for c in self.player.containers.iter_mut() {
                        if c.container_id == container_id {
                            if let crate::player::ContainerSource::Map(c_pos) = c.source {
                                if c_pos == pos {
                                    c.items = items.clone();
                                }
                            }
                        }
                    }
                    
                    // Re-send the open-container packet for each matching window.
                    let matching: Vec<_> = self.player.containers.iter()
                        .filter(|c| c.container_id == container_id && matches!(c.source, crate::player::ContainerSource::Map(p) if p == pos))
                        .cloned()
                        .collect();
                    for c in matching {
                        let msg = self.send_open_container(&c).await?;
                        self.enqueue(msg);
                    }
                }

                WorldToPlayer::StatusMessage(message) => {
                    let pkt = self.send_status_message(&message).await?;
                    self.enqueue(pkt);
                }
            }
        }
        Ok(())
    }

    /// Main connection loop - runs until the client disconnects.
    ///
    /// Each iteration:
    /// 1. Drains and handles any pending world messages.
    /// 2. Advances one auto-walk step if a path is queued.
    /// 3. Waits up to 16 ms for an incoming packet and dispatches it.
    /// 4. Flushes all queued outgoing packets.
    ///
    /// On clean disconnect ([`UnexpectedEof`] or [`ConnectionReset`]), the loop exits
    /// and a [`PlayerToWorld::Logout`] is sent to the world.
    pub async fn run(&mut self) -> Result<()> {
        loop {
            self.process_world_messages().await?;

            // Advance one step of the auto-walk path per tick.
            if !self.pending_path.is_empty() {
                let direction = self.pending_path.remove(0);
                self.player.direction = direction;
                self.player.position  = self.player.position + direction;
                let new_pos = self.player.position;
                self.world_sender.send(PlayerToWorld::UpdatePosition(self.player.clone()))?;
                let map = self.send_map(new_pos, 18, 14).await?;
                self.enqueue(map);
                self.flush().await?;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }

            match timeout(Duration::from_millis(16), self.stream.read_u16_le()).await {
                // Packet received - read the payload and dispatch it to the appropriate handler.
                Ok(Ok(length)) => {
                    let mut buf = vec![0u8; length as usize];
                    self.stream.read_exact(&mut buf).await?;
                    self.handle_packet(&buf).await?;
                }

                // Clean disconnect by the client.
                Ok(Err(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof || e.kind() == std::io::ErrorKind::ConnectionReset => {
                    break;
                }

                // Unexpected network error - propagate to the caller.
                Ok(Err(e)) => return Err(e.into()),

                // 16 ms timeout elapsed with no packet - loop and check world messages again.
                Err(_elapsed) => {}
            }

            // Send queued outgoing packets.
            self.flush().await?;
        }

        // Notify the world that this player has logged out, so it removes this player from the active list.
        self.world_sender.send(PlayerToWorld::Logout(self.player.clone()))?;
        Ok(())
    }
}

/// Reads and validates an existing-player login packet ("Journey Onward", 67 bytes).
///
/// The 5-byte header is skipped as its contents are unknown. Only protocol version
/// 103 is accepted; anything else is rejected and `Ok(None)` is returned.
async fn login_existing(stream: &mut TcpStream) -> Result<Option<Player>> {
    stream.skip(5).await?; // 5 unknown header bytes

    let protocol = stream.read_u16_le().await?;
    if protocol != 103 {
        debug_log!("Login rejected: unsupported Tibia protocol {}", protocol);
        return Ok(None);
    }

    let mut name = String::new();
    stream.read_string(&mut name, 30).await?;

    let mut password = String::new();
    stream.read_string(&mut password, 30).await?;

    let mut player = Player::new(&name, MAP.get().unwrap().read().await.respawn);
    player.password = password;
    Ok(Some(player))
}

/// Reads and validates a new-character login packet ("New Game", 221 bytes).
///
/// The 5-byte header is skipped as its contents are unknown. Only protocol version
/// 103 is accepted; anything else is rejected and `Ok(None)` is returned.
async fn login_new(stream: &mut TcpStream) -> Result<Option<Player>> {
    stream.skip(5).await?; // 5 unknown header bytes

    let protocol = stream.read_u16_le().await?;
    if protocol != 103 {
        debug_log!("Character creation rejected: unsupported Tibia version {}", protocol);
        return Ok(None);
    }

    let mut name = String::new();
    stream.read_string(&mut name, 30).await?;

    let mut password = String::new();
    stream.read_string(&mut password, 30).await?;

    let gender = stream.read_gender().await?;
    let outfit = stream.read_outfit_colors().await?;

    let mut real_name = String::new();
    stream.read_string(&mut real_name, 50).await?;

    let mut location = String::new();
    stream.read_string(&mut location, 50).await?;

    let mut email = String::new();
    stream.read_string(&mut email, 50).await?;

    let mut player = Player::new(&name, MAP.get().unwrap().read().await.respawn);
    player.password = password;
    player.gender = gender;
    player.outfit = outfit;
    player.real_name = real_name;
    player.location = location;
    player.email = email;
    Ok(Some(player))
}
