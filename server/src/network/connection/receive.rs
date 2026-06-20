use super::Connection;
use crate::{
    debug_log,
    chat::ChatType,
    io::ReadExt,
    network::packet::PacketIn,
    player::Direction,
};
use anyhow::Result;
use std::{convert::TryInto, io::Cursor};
use tokio::io::{AsyncRead, AsyncReadExt};

impl Connection {
    /********************************************************************************
     * 
     * Handle incoming packets (client -> server)
     * Reads the packet ID and dispatches it to the appropriate handler.
     * 
     ********************************************************************************/
    pub async fn handle_packet(&mut self, buf: &[u8]) -> Result<()> {
        let mut cursor = Cursor::new(buf);

        match cursor.read_u16_le().await?.try_into() {
            Ok(packet) => {
                /********************************************************************************
                 * 
                 * Packet sniffer:
                 * Prints decoded packet information and raw packet bytes.
                 * Useful when reverse engineering or debugging packet structures.
                 * 
                 ********************************************************************************/
                // debug_log!(
                //     "network/connection/receive::handle_packet -> Received incoming packet:\n\
                //     \tType: {:?}\n\
                //     \tSize: {} bytes\n\
                //     \tRaw packet data: {:02X?}",
                //     packet,
                //     buf.len(),
                //     buf
                // );

                self.dispatch_packet(packet, &mut cursor).await?
            }
            Err(_) => {
                debug_log!(
                    "network/connection/receive::handle_packet -> Unknown packet received:\n\
                    \tSize: {} bytes\n\
                    \tRaw packet data: {:02X?}",
                    buf.len(),
                    buf
                );
            }
        }
        Ok(())
    }


    /********************************************************************************
     * 
     * Handle incoming packets (client -> server)
     * Dispatches a received packet to the appropriate handler.
     * 
     ********************************************************************************/
    async fn dispatch_packet<R: AsyncRead + Unpin>(
        &mut self,
        packet: PacketIn,
        r: &mut R,
    ) -> Result<()> {
        match packet {
            PacketIn::UserList          => self.recv_user_list(r).await?,
            PacketIn::PlayerInfo        => self.recv_player_info(r).await?,
            PacketIn::Walk              => self.recv_walk(r).await?,
            PacketIn::AutoWalk          => self.recv_auto_walk(r).await?,
            PacketIn::LookAt            => self.recv_look_at(r).await?,
            PacketIn::Chat              => self.recv_chat(r).await?,
            PacketIn::ChangeDirection   => { /* Not observed in Tibia 1.03 */ },
            PacketIn::Comment           => self.recv_comment(r).await?,
            PacketIn::Push              => self.recv_push(r).await?,
            PacketIn::UseItem           => self.recv_use_item(r).await?,
            PacketIn::CloseContainer    => self.recv_close_container(r).await?,
            PacketIn::RequestChangeData => self.recv_request_change_data(r).await?,
            PacketIn::SetData           => self.recv_set_data(r).await?,
            PacketIn::Echo              => { /* No operation */ }
            PacketIn::Logout            => { /* No operation; here we can add a function to save player state later */ }
        }
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles walking in the game
     * 
     ********************************************************************************/
    async fn recv_walk<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        debug_log!("network/connection/receive::recv_walk -> Received incoming packet for walking.");

        /********************************************************************************
         * 
         * Get direction, current position, and compute new position.
         * 
         ********************************************************************************/
        let direction: Direction = r.read_u8().await?.try_into()?;
        self.player.direction = direction;
        let old_pos = self.player.position;
        self.player.position = self.player.position + direction;
        let new_pos = self.player.position;

        debug_log!(
            "network/connection/receive::recv_walk -> Player walked:\n\
            \tPlayer name: {}\n\
            \tOld position: {:?}\n\
            \tNew position: {:?}\n\
            \tDirection: {:?} ({:?})",
            self.player.name,
            old_pos,
            new_pos,
            direction as u8, direction
        );


        /********************************************************************************
         * 
         * Tell the world that I have walked.
         * 
         ********************************************************************************/
        self.world_sender.send(
            crate::world::message::PlayerToWorld::UpdatePosition(self.player.clone())
        )?;


        /********************************************************************************
         * 
         * Refreshes the map
         * In Tibia 1.03 the map is refreshed every time a player walks
         * 
         ********************************************************************************/
        let msg = self.send_map(new_pos, 18, 14).await?;
        self.enqueue(msg);
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles incoming chat messages.
     * 
     * Messages beginning with '#' are treated as qualified chat messages:
     * - Whisper
     * - Yell
     * - Broadcast
     * 
     ********************************************************************************/
    async fn recv_chat<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        debug_log!("network/connection/receive::recv_chat -> Received incoming packet for a chat message.");

        let length = r.read_u16_le().await?;
        let mut raw = vec![0u8; length as usize];
        r.read_exact(&mut raw).await?;
        let msg = unsafe { String::from_utf8_unchecked(raw) };

        if msg.starts_with('#') {
            self.recv_qualified_chat(&msg).await?;
        } else {
            let encoded = crate::chat::encoding::translate(&msg);
            self.world_sender.send(crate::world::message::PlayerToWorld::Chat(
                self.player.position,
                ChatType::Normal,
                encoded,
                self.player.name.clone(),
            ))?;
        }
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles qualified chat messages.
     * 
     * The character after '#' determines the chat type.
     * See "src/chat/mod.rs" for more details.
     * 
     ********************************************************************************/
    async fn recv_qualified_chat(&mut self, msg: &str) -> Result<()> {
        debug_log!("network/connection/receive::recv_qualified_chat -> Received incoming packet for a qualified chat message.");

        use std::convert::TryInto;

        match msg.chars().nth(1).try_into() {
            Ok(chat_type) => {
                let encoded = match chat_type {
                    ChatType::Yell | ChatType::Broadcast => crate::chat::encoding::translate_upper(&msg[3..].to_uppercase()),
                    _              => crate::chat::encoding::translate(&msg[3..]),
                };
                self.world_sender.send(crate::world::message::PlayerToWorld::Chat(
                    self.player.position,
                    chat_type,
                    encoded,
                    self.player.name.clone(),
                ))?;
            }
            Err(_) => {
                debug_log!("network/connection/receive::recv_qualified_chat -> Unknown chat qualifier: {:?}", msg.chars().nth(1));
            }
        }
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles client comments.
     * These are messages sent in by the players and are not displayed in the game.
     * 
     ********************************************************************************/
    async fn recv_comment<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        debug_log!("network/connection/receive::recv_comment -> Received incoming packet for submitting a comment.");

        let mut msg = String::new();
        r.read_string_to_end(&mut msg).await?;

        debug_log!("network/connection/receive::recv_comment -> Player {} submitted a comment:\n\t{}", self.player.name, msg);

        Ok(())
    }


    /********************************************************************************
     * 
     * Handles requests to open the character information window.
     * The server responds by sending the player's editable data.
     * 
     ********************************************************************************/
    async fn recv_request_change_data<R: AsyncRead + Unpin>(&mut self, _r: &mut R) -> Result<()> {
        debug_log!("network/connection/receive::recv_request_change_data -> Received incoming packet for requesting change of user data.");

        let msg = self.send_data_window().await?;
        self.enqueue(msg);
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles applying new user data.
     * Sent when a player submits user data via the "Change Data" dialog.
     * 
     ********************************************************************************/
    async fn recv_set_data<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        debug_log!("network/connection/receive::recv_set_data -> Received incoming packet for saving user data.");

        let mut password = String::new();
        r.read_string(&mut password, 30).await?;

        let outfit = r.read_outfit_colors().await?;

        let mut real_name = String::new();
        r.read_string(&mut real_name, 50).await?;

        let mut location = String::new();
        r.read_string(&mut location, 50).await?;

        let mut email = String::new();
        r.read_string(&mut email, 50).await?;

        /********************************************************************************
         * 
         * Store the submitted information on the player.
         * Next step is to permanently store these in a database. Maybe SQLite will do?
         * 
         ********************************************************************************/
        self.player.password = password;
        self.player.outfit = outfit;
        self.player.real_name = real_name;
        self.player.location = location;
        self.player.email = email;

        /********************************************************************************
         * 
         * Send the updated player data to the world so that all other players can access it.
         * 
         ********************************************************************************/
        self.world_sender.send(crate::world::message::PlayerToWorld::UpdateInfo(self.player.clone()))?;

        /********************************************************************************
         * 
         * Refresh the map. Outfit changes require a refresh.
         * 
         ********************************************************************************/
        let pos = self.player.position;
        let msg = self.send_map(pos, 18, 14).await?;
        self.enqueue(msg);
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles displaying the list of users in the game (Menu: Info -> Userlist)
     * 
     ********************************************************************************/
    async fn recv_user_list<R: AsyncRead + Unpin>(&mut self, _r: &mut R) -> Result<()> {
        debug_log!("network/connection/receive::recv_user_list -> Received incoming packet for displaying the list of users.");

        let msg = self.send_user_list().await?;
        self.enqueue(msg);
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles requesting information about a player (Menu: Info -> Userlist -> {selectedPlayer} -> Info)
     * 
     ********************************************************************************/
    async fn recv_player_info<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        debug_log!("network/connection/receive::recv_player_info -> Received incoming packet for requesting data about a user.");

        let mut name = String::new();
        r.read_string_to_end(&mut name).await?;

        /********************************************************************************
         * 
         * Find the requested player - check self first, then other players.
         * 
         ********************************************************************************/
        let player = if self.player.name == name {
            self.player.clone()
        } else {
            self.other_players.iter().find(|p| p.name == name).cloned().unwrap_or(self.player.clone())
        };

        let msg = self.send_user_info(&player).await?;
        self.enqueue(msg);
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles auto-walking requests.
     * Client sends a destination position and the server is expected to
     * calculate a path and move the player automatically.
     * 
     ********************************************************************************/
    async fn recv_auto_walk<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        // To do
        debug_log!("network/connection/receive::recv_auto_walk -> Received incoming packet for auto-walking.");

        let _dest = r.read_position().await?;
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles "look at" requests.
     * Sent when the player clicks left+right mouse buttons and examines an object or player.
     * 
     ********************************************************************************/
    async fn recv_look_at<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        debug_log!("network/connection/receive::recv_look_at -> Received incoming packet for looking at something.");

        /********************************************************************************
         * 
         * Get screen position.
         * 
         ********************************************************************************/
        let screen_pos = r.read_position().await?;


        /********************************************************************************
         * 
         * Convert screen coordinates to map coordinates.
         * 
        ********************************************************************************/
        let map_x = self.player.position.x as i32 + (screen_pos.x as i32 - 8);
        let map_y = self.player.position.y as i32 + (screen_pos.y as i32 - 6);
        let map_pos = crate::map::position::Position::new(map_x as u16, map_y as u16, self.player.position.z);


        /********************************************************************************
         * 
         * Format the text based on what the player is looking at.
         * 
         ********************************************************************************/
        let text = if let Some(other) = self.other_players.iter().find(|p| p.position == map_pos) {
            /********************************************************************************
             * 
             * Looking at another player - always takes priority.
             * 
             ********************************************************************************/
            format!(
                "You see {}. {} is {}.\n[Position: {}, {}, {}]",
                other.name,
                match other.gender {
                    crate::player::Gender::Female => "She",
                    _ => "He",
                },
                match other.gender {
                    crate::player::Gender::Female => "female",
                    _ => "male"
                },
                other.position.x,
                other.position.y,
                other.position.z
            )

        } else if map_pos == self.player.position {
            /********************************************************************************
             * 
             * Looking at yourself.
             * 
             ********************************************************************************/
            format!(
                "You see yourself. You are {}.\n[Position: {}, {}, {}]",
                match self.player.gender {
                    crate::player::Gender::Female => "female",
                    _ => "male"
                },
                self.player.position.x,
                self.player.position.y,
                self.player.position.z
            )

        } else if let Some(objects) = crate::map::MAP.get().unwrap().get_tile_objects(map_pos) {
            /********************************************************************************
             * 
             * Getting the topmost object on a tile.
             * 
             ********************************************************************************/
            let top = objects.iter().filter(|o| matches!(o,
                crate::map::TileObject::Item(_)
            )).next()
            .or_else(|| objects.first().filter(|o| matches!(o, crate::map::TileObject::Ground(_))));

            match top {
                /********************************************************************************
                 * 
                 * Looking at items.
                 * 
                 ********************************************************************************/
                Some(crate::map::TileObject::Item(id)) => format!(
                    "You see {}.\n[Item id: {}]\n[Position: {}, {}, {}]",
                    crate::map::items::item_name(*id),
                    id,
                    map_pos.x,
                    map_pos.y,
                    map_pos.z
                ),

                /********************************************************************************
                 * 
                 * Looking at ground tiles.
                 * 
                 ********************************************************************************/
                Some(crate::map::TileObject::Ground(id)) => format!(
                   "You see {}.\n[Item id: {}]\n[Position: {}, {}, {}]",
                    crate::map::items::item_name(*id),
                    id,
                    map_pos.x,
                    map_pos.y,
                    map_pos.z
                ),
                /********************************************************************************
                 * 
                 * Looking at an unknown object.
                 * 
                 ********************************************************************************/
                _ => format!(
                    "You see an unknown object.\n[Position: {}, {}, {}]",
                    map_pos.x,
                    map_pos.y,
                    map_pos.z
                ),
            }
        } else {
            format!(
                "You see nothing.\n[Position: {}, {}, {}]",
                map_pos.x,
                map_pos.y,
                map_pos.z
            )
        };

        let msg = self.send_look_message(&text).await?;
        self.enqueue(msg);
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles item pushing requests.
     * Sent when a player drags an item from one position to another.
     * 
     ********************************************************************************/
    async fn recv_push<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        // To do
        debug_log!("network/connection/receive::recv_push -> Received incoming packet for pushing an object.");

        let _from = r.read_position().await?;
        let _item_id = r.read_u16_le().await?;
        let _stack_pos = r.read_u8().await?;
        let _to = r.read_position().await?;
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles item usage requests.
     * Sent when a player uses an item from inventory, the map, or a container.
     * 
     ********************************************************************************/
    async fn recv_use_item<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        // To do
        debug_log!("network/connection/receive::recv_use_item -> Received incoming packet for using an object.");

        let _item_type = r.read_u8().await?;
        let _pos = r.read_position().await?;
        let _item_id = r.read_u16_le().await?;
        let _stack_pos = r.read_u8().await?;
        let _unknown = r.read_u8().await?;
        let msg = self.send_open_container().await?;
        self.enqueue(msg);
        Ok(())
    }


    /********************************************************************************
     * 
     * Handles container close requests.
     * Sent when a player closes an open container window by left-clicking on it.
     * 
     ********************************************************************************/
    async fn recv_close_container<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        debug_log!("network/connection/receive::recv_close_container -> Received incoming packet for closing a container.");

        let local_id = r.read_u8().await?;
        let msg = self.send_close_container(local_id).await?;
        self.enqueue(msg);
        Ok(())
    }
}
