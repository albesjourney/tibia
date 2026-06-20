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


/********************************************************************************
 * 
 * Client connection
 * 
 ********************************************************************************/
pub struct Connection {
    stream:         TcpStream,
    player:         Player,
    message_queue:  SegQueue<Vec<u8>>,
    world_sender:   UnboundedSender<PlayerToWorld>,
    world_receiver: UnboundedReceiver<WorldToPlayer>,
    other_players:  Vec<Player>,
    pending_path:   Vec<Direction>,
}

impl Connection {
    /********************************************************************************
     * 
     * Create a new client connection.
     * 
     ********************************************************************************/
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
            pending_path: Vec::new(),
        }
    }


    /********************************************************************************
     * 
     * Handle incoming login requests.
     * 
     * Supported login types:
     * - Existing character login
     * - New character creation
     * 
     ********************************************************************************/
    pub async fn handle_login(
        mut stream:   TcpStream,
        world_sender: UnboundedSender<PlayerToWorld>,
    ) -> Result<Option<Self>> {
        /********************************************************************************
         * 
         * Get the length of the login packet.
         * 
         ********************************************************************************/
        let length = stream.read_u16_le().await?;

        match length {
            /********************************************************************************
             * 
             * Existing player login (67 bytes).
             * 
             ********************************************************************************/
            67 => {
                let player = login_existing(&mut stream).await?;

                match player {
                    Some(player) => {
                        let (game_sender, world_receiver) = unbounded_channel();
                        
                        world_sender.send(PlayerToWorld::Login(
                            player.clone(), 
                            game_sender
                        ))?;

                        let mut conn = Connection::new(
                            stream, 
                            player, 
                            world_sender, 
                            world_receiver
                        );

                        conn.send_login_sequence().await?;
                        conn.flush().await?;

                        /********************************************************************************
                         * 
                         * Wait briefly for the world to send back the initial PlayerList so that other
                         * online players appear on the map immediately.
                         * 
                         ********************************************************************************/
                        loop {
                            if let Ok(msg) = conn.world_receiver.try_recv() {
                                match msg {
                                    crate::world::message::WorldToPlayer::PlayerList(players) => {
                                        conn.other_players = players.into_iter().filter(|p| p.id != conn.player.id).collect();
                                        let pos = conn.player.position;
                                        let map = conn.send_map(pos, 18, 14).await?;
                                        conn.enqueue(map);
                                        conn.flush().await?;
                                        break;
                                    }

                                    crate::world::message::WorldToPlayer::Chat(sender_pos, chat_type, encoded, sender_name) => {
                                        let pkt = conn.build_chat_packet(chat_type, &encoded, sender_pos, &sender_name).await?;
                                        conn.enqueue(pkt);
                                    }
                                }
                            } else {
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        }

                        Ok(Some(conn))
                    }
                    None => Ok(None),
                }
            }


            /********************************************************************************
             * 
             * New character creation (221 bytes).
             * 
             ********************************************************************************/
            221 => {
                let player = login_new(&mut stream).await?;

                match player {
                    Some(player) => {
                        let (game_sender, world_receiver) = unbounded_channel();

                        world_sender.send(PlayerToWorld::Login(
                            player.clone(), 
                            game_sender
                        ))?;

                        let mut conn = Connection::new(
                            stream, 
                            player, 
                            world_sender, 
                            world_receiver
                        );

                        conn.send_login_sequence().await?;
                        conn.flush().await?;

                        /********************************************************************************
                         * 
                         * Wait briefly for the world to send back the initial PlayerList so that other
                         * online players appear on the map immediately.
                         * 
                         ********************************************************************************/
                        loop {
                            if let Ok(msg) = conn.world_receiver.try_recv() {
                                match msg {
                                    crate::world::message::WorldToPlayer::PlayerList(players) => {
                                        conn.other_players = players.into_iter().filter(|p| p.id != conn.player.id).collect();
                                        let pos = conn.player.position;
                                        let map = conn.send_map(pos, 18, 14).await?;
                                        conn.enqueue(map);
                                        conn.flush().await?;
                                        break;
                                    }

                                    crate::world::message::WorldToPlayer::Chat(sender_pos, chat_type, encoded, sender_name) => {
                                        let pkt = conn.build_chat_packet(chat_type, &encoded, sender_pos, &sender_name).await?;
                                        conn.enqueue(pkt);
                                    }
                                }
                            } else {
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        }

                        Ok(Some(conn))
                    }
                    None => Ok(None),
                }
            }


            /********************************************************************************
             * 
             * Unknown/invalid login packet.
             * 
             ********************************************************************************/
            _ => {
                debug_log!("Unknown login packet length: {}", length);
                Ok(None)
            }
        }
    }


    /********************************************************************************
     * 
     * Check for world messages and process them.
     * This includes things such as redrawing characters on the map, refreshing chat, etc.
     * 
     ********************************************************************************/
    async fn process_world_messages(&mut self) -> Result<()> {
        while let Ok(msg) = self.world_receiver.try_recv() {
            match msg {
                crate::world::message::WorldToPlayer::PlayerList(players) => {
                    let new_others: Vec<Player> = players.into_iter().filter(|p| p.id != self.player.id).collect();

                    /********************************************************************************
                     * 
                     * Only redraw if another player's position actually changed relative to our viewport.
                     * 
                     ********************************************************************************/
                    let needs_redraw = new_others.iter().any(|new| {
                        let old = self.other_players.iter().find(|o| o.id == new.id);
                        match old {
                            None => true, // new player appeared
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


                /********************************************************************************
                 * 
                 * Handle chat messages.
                 * 
                 ********************************************************************************/
                crate::world::message::WorldToPlayer::Chat(sender_pos, chat_type, encoded, sender_name) => {
                    let pkt = match chat_type {
                        /********************************************************************************
                         * 
                         * Broadcast appears at top-left corner of screen for everyone.
                         * 
                         * Note:
                         * I was unable to find any special broadcast message type in Tibia 1.03.
                         * Typically they would appear as red text on the center of screen.
                         * So we can only assume they appeared in a corner, in uppercase.
                         * 
                         ********************************************************************************/
                        crate::chat::ChatType::Broadcast => {                            
                            let top_left = crate::map::position::Position::new(
                                self.player.position.x.saturating_sub(8),
                                self.player.position.y.saturating_sub(6),
                                self.player.position.z,
                            );
                            
                            self.build_chat_packet(chat_type, &encoded, top_left, &sender_name).await?
                        }
                        _ => {
                            /********************************************************************************
                             * 
                             * All other message types are positioned relative to the sender's map position.
                             * 
                             ********************************************************************************/
                            self.build_chat_packet(chat_type, &encoded, sender_pos, &sender_name).await?
                        }
                    };
                    self.enqueue(pkt);
                }
            }
        }
        Ok(())
    }


    /********************************************************************************
     * 
     * Main connection loop.
     * 
     * Continuously:
     * - Receives incoming packets from the client
     * - Dispatches packets to their handlers
     * - Sends queued outgoing packets
     * - Detects disconnects and connection errors
     * 
     * When the client disconnects, a logout message is sent to the World.
     * 
     ********************************************************************************/
    pub async fn run(&mut self) -> Result<()> {
        loop {
            /********************************************************************************
             * 
             * Check for messages from the world (refresh PlayerList, Chat, etc).
             * 
             ********************************************************************************/
            self.process_world_messages().await?;


            /********************************************************************************
             * 
             * Process one auto-walk step per loop iteration.
             * 
             ********************************************************************************/
            if !self.pending_path.is_empty() {
                let direction = self.pending_path.remove(0);
                self.player.direction = direction;
                self.player.position = self.player.position + direction;
                let new_pos = self.player.position;
                self.world_sender.send(crate::world::message::PlayerToWorld::UpdatePosition(self.player.clone()))?;
                let map = self.send_map(new_pos, 18, 14).await?;
                self.enqueue(map);
                self.flush().await?;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            
            
            /********************************************************************************
             * 
             * Receive incoming packets.
             * 
             ********************************************************************************/
            match timeout(Duration::from_millis(16), self.stream.read_u16_le()).await {
                /********************************************************************************
                * 
                * Packet received from client
                * Read the packet payload and dispatch it to the appropriate handler.
                * 
                ********************************************************************************/
                Ok(Ok(length)) => {
                    let mut buf = vec![0u8; length as usize];
                    self.stream.read_exact(&mut buf).await?;
                    self.handle_packet(&buf).await?;
                }


                /********************************************************************************
                * 
                * Client disconnected
                * 
                * A normal disconnect usually appears as either:
                * - UnexpectedEof
                * - ConnectionReset
                * 
                ********************************************************************************/
                Ok(Err(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof || e.kind() == std::io::ErrorKind::ConnectionReset => {
                    break;
                }


                /********************************************************************************
                * 
                * Unexpected network error. Propagate the error to the caller.
                * 
                ********************************************************************************/
                Ok(Err(e)) => return Err(e.into()),
                Err(_elapsed) => {}
            }


            /********************************************************************************
            * 
            * Send queued outgoing packets.
            * 
            ********************************************************************************/
            self.flush().await?;
        }


        /********************************************************************************
         * 
         * Notify the world that this player has logged out.
         * It removes the player from the world's active player list.
         * 
         ********************************************************************************/
        self.world_sender.send(PlayerToWorld::Logout(self.player.clone()))?;
        Ok(())
    }
}


/********************************************************************************
 * 
 * Handles login sequence for existing players ("Journey Onward").
 * 
 ********************************************************************************/
async fn login_existing(stream: &mut TcpStream) -> Result<Option<Player>> {
    // Get the full raw login packet for debugging purposes:
    // let mut buf = vec![0u8; 221];
    // stream.peek(&mut buf).await?;
    // debug_log!("network/connection/mod::login_existing -> Raw packet: {:02x?}", buf);

    stream.skip(5).await?; // 5 unknown header bytes

    /********************************************************************************
     * 
     * Only accept login requests from the Tibia 1.03 protocol.
     * 
     ********************************************************************************/
    let protocol = stream.read_u16_le().await?;
    if protocol != 103 {
        debug_log!("Login rejected: Unsupported Tibia protocol {}", protocol);
        return Ok(None);
    }

    let mut name = String::new();
    stream.read_string(&mut name, 30).await?;

    let mut password = String::new();
    stream.read_string(&mut password, 30).await?;

    let mut player = Player::new(
        &name,
        MAP.get().unwrap().respawn,
    );

    player.password = password;
    Ok(Some(player))
}


/********************************************************************************
 * 
 * Handles login sequence for new players ("New Game").
 * 
 ********************************************************************************/
async fn login_new(stream: &mut TcpStream) -> Result<Option<Player>> {
    // Get the full raw login packet for debugging purposes:
    // let mut buf = vec![0u8; 221];
    // stream.peek(&mut buf).await?;
    // debug_log!("network/connection/mod::login_new -> Raw packet: {:02x?}", buf);

    stream.skip(5).await?; // 5 unknown header bytes

    /********************************************************************************
     * 
     * Only accept login requests from the Tibia 1.03 protocol.
     * 
     ********************************************************************************/
    let protocol = stream.read_u16_le().await?;
    if protocol != 103 {
        debug_log!("Character creation rejected: Unsupported Tibia version {}", protocol);
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

    let mut player = Player::new(
        &name, 
        MAP.get().unwrap().respawn
    );

    player.password = password;
    player.gender = gender;
    player.outfit = outfit;
    player.real_name = real_name;
    player.location = location;
    player.email = email;
    Ok(Some(player))
}
