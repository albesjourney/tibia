mod receive;
mod send;

use crate::{
    debug_log,
    io::ReadExt,
    map::MAP,
    player::Player,
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
        // Optionally log the length for debugging purposes:
        // debug_log!("Incoming login packet length: {}", length);

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
             * Check for messages from the world.
             * 
             ********************************************************************************/
            while let Ok(msg) = self.world_receiver.try_recv() {
                match msg {
                    crate::world::message::WorldToPlayer::PlayerList(players) => {
                        self.other_players = players.into_iter().filter(|p| p.id != self.player.id).collect();
                        let pos = self.player.position;
                        let map = self.send_map(pos, 18, 14).await?;
                        self.enqueue(map);
                    }

                    crate::world::message::WorldToPlayer::Chat(sender_pos, chat_type, encoded, sender_name) => {
                        let pkt = self.build_chat_packet(chat_type, &encoded, sender_pos, &sender_name).await?;
                        self.enqueue(pkt);
                    }
                }
            }
            
            
            /********************************************************************************
             * 
             * Receive incoming packets.
             * 
             ********************************************************************************/
            match timeout(Duration::from_millis(100), self.stream.read_u16_le()).await {
                /********************************************************************************
                * 
                * Packet received from client
                * 
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
    // debug_log!("raw packet: {:02x?}", buf);

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
    // debug_log!("raw packet: {:02x?}", buf);

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
