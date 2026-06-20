use super::Connection;
use crate::{
    chat::{encoding, ChatType},
    config::CONFIG,
    io::WriteExt,
    map::{TileObject, MAP},
    network::packet::{PacketOut, PacketOutAux},
    player::{Direction, InventorySlot, OutfitColors, Player},
};
use anyhow::Result;
use std::io::Cursor;
use tokio::io::AsyncWriteExt;

impl Connection {
    /********************************************************************************
     * 
     * Stores outgoing packets in a queue.
     * flush() will later add the length prefix and send everything over the socket.
     * 
     ********************************************************************************/
    pub fn enqueue(&self, msg: Vec<u8>) {
        if !msg.is_empty() {
            self.message_queue.push(msg);
        }
    }


    /********************************************************************************
     * 
     * Sends all queued messages.
     * Each queued packet is sent as:
     * - u16 little-endian length (packet_size + 2)
     * - raw packet bytes
     * 
     ********************************************************************************/
    pub async fn flush(&mut self) -> Result<()> {
        let mut out = Cursor::new(Vec::new());

        while let Some(msg) = self.message_queue.pop() {
            out.write_u16_le(msg.len() as u16 + 2).await?;
            out.write_all(&msg).await?;
        }

        let bytes = out.into_inner();
        if !bytes.is_empty() {
            self.stream.write_all(&bytes).await?;
            self.stream.flush().await?;
        }

        Ok(())
    }


    /********************************************************************************
     * 
     * Sends the initial set of packets after the client connects.
     * This establishes the player's identity and gives the client the first map view.
     * 
     ********************************************************************************/
    pub async fn send_login_sequence(&mut self) -> Result<()> {
        let config = CONFIG.get().unwrap();
        let pos = self.player.position;

        /********************************************************************************
         * 
         * Login.
         * 
         ********************************************************************************/
        let login = self.send_login().await?;
        self.enqueue(login);


        /********************************************************************************
         * 
         * Set equipment (for all players).
         * This will be replaced later when each player has their own inventory saved in a database.
         * 
         ********************************************************************************/
        // let helmet =        self.send_equipped_item(InventorySlot::Helmet, 0x013D).await?;
        // let necklace =      self.send_equipped_item(InventorySlot::Necklace, 0x013D).await?;
        let bag =           self.send_equipped_item(InventorySlot::Bag, 0x013D).await?;         // bag
        // let armor =         self.send_equipped_item(InventorySlot::Armor, 0x013D).await?;
        let right_hand =    self.send_equipped_item(InventorySlot::RightHand, 0x005A).await?;   // sword
        let left_hand =     self.send_equipped_item(InventorySlot::LeftHand, 0x0086).await?;    // bread
        // let legs =          self.send_equipped_item(InventorySlot::Legs, 0x013D).await?;
        // let boots =         self.send_equipped_item(InventorySlot::Boots, 0x013D).await?;
        //self.enqueue(helmet);
        //self.enqueue(necklace);
        self.enqueue(bag);
        //self.enqueue(armor);
        self.enqueue(right_hand);
        self.enqueue(left_hand);
        //self.enqueue(legs);
        //self.enqueue(boots);

        
        /********************************************************************************
         * 
         * Load the map view.
         * 
         ********************************************************************************/
        let map = self.send_map(pos, 18, 14).await?;
        self.enqueue(map);


        /********************************************************************************
         * 
         * Send status message and Message of the Day.
         * 
         ********************************************************************************/
        let status = self.send_status_message(&config.server.status_message).await?;
        let motd = self.send_message_of_the_day(&config.server.message_of_the_day).await?;
        self.enqueue(status);
        self.enqueue(motd);

        Ok(())
    }


    /********************************************************************************
     * 
     * Outgoing packet builder for login.
     * Produces only the payload; flush() adds the length framing.
     * 
     ********************************************************************************/
    async fn send_login(&self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::Login).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Sends the server status text show by the client (bottom-left corner).
     * Payload is a packet header plus a null-terminated string.
     * 
     ********************************************************************************/
    pub async fn send_status_message(&self, message: &str) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::StatusMessage).await?;
        buf.write_null_terminated_string(message).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Sends the Message of the Day (MotD).
     * Payload is a packet header plus a null-terminated string.
     * 
     ********************************************************************************/
    pub async fn send_message_of_the_day(&self, message: &str) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::MessageOfTheDay).await?;
        buf.write_null_terminated_string(message).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Tells the client which item ID is equipped in a specific inventory slot.
     * 
     ********************************************************************************/
    pub async fn send_equipped_item(&self, slot: InventorySlot, item_id: u16) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::EquippedItem).await?;
        buf.write_u16_le(item_id).await?;
        buf.write_u8(slot as u8).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Sends the player's editable character fields.
     * This is the data window the client uses in "Info -> Change Data".
     * It populates all input fields with the player's data.
     * 
     ********************************************************************************/
    pub async fn send_data_window(&self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::DataWindow).await?;

        buf.write_fixed_string(&self.player.name, 30).await?;
        buf.write_fixed_string(&self.player.password, 30).await?;

        buf.write_gender(self.player.gender).await?;
        buf.write_outfit_colors(self.player.outfit).await?;

        buf.write_fixed_string(&self.player.real_name, 50).await?;
        buf.write_fixed_string(&self.player.location, 50).await?;
        buf.write_fixed_string(&self.player.email, 50).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Sends the list of users (Info -> Userlist).
     * This displays a list of all players online.
     * 
     ********************************************************************************/
    pub async fn send_user_list(&self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::UserList).await?;
        buf.write_u16_le(0x1010).await?;

        // Write self first.
        buf.write_all(self.player.name.as_bytes()).await?;
        buf.write_u8(b'\n').await?;

        // Write all other online players.
        for other in &self.other_players {
            buf.write_all(other.name.as_bytes()).await?;
            buf.write_u8(b'\n').await?;
        }

        buf.write_u8(0).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Sends the pop-up window for a selected user (Info -> Userlist -> {selectedPlayer} -> Info).
     * 
     ********************************************************************************/
    pub async fn send_user_info(&self, player: &Player) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());

        buf.write_packet(PacketOut::UserInfo).await?;
        buf.write_u16_le(0x1010).await?;

        buf.write_null_terminated_string(&format!(
            "Name: {}\n\
            Real name: {}\n\
            Location: {}\n\
            Email: {}\n\
            Sex: {:?}",
            player.name,
            player.real_name,
            player.location,
            player.email,
            player.gender
        )).await?;

        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Opens the container UI on the client.
     * For now it always opens a bag and pre-fills it with some items.
     * 
     ********************************************************************************/
    pub async fn send_open_container(&self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::OpenContainer).await?;


        /********************************************************************************
         * 
         * Local ID used to reference the UI container, and the bag item ID.
         * 
         ********************************************************************************/
        buf.write_u8(1).await?;
        buf.write_u16_le(0x013D).await?;


        /********************************************************************************
         * 
         * Add 4 meat to the container.
         * 
         ********************************************************************************/
        for _ in 0..4 {
            buf.write_u16_le(0x0282).await?;
        }

        buf.write_u16_le(0xFFFF).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Closes a container UI on the client.
     * "local_id" must match the one used when opening it.
     * 
     ********************************************************************************/
    pub async fn send_close_container(&self, local_id: u8) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::CloseContainer).await?;
        buf.write_u8(local_id).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Sends a chat message.
     * It encodes the text based on chat type (yelling will become uppercase).
     * If a sender is present, it also includes the sender name and a tab separator.
     * 
     * Note: the "chat bubble position" is currently hardcoded (grid_x, grid_y) and currently
     * not using map-based coordinates. This needs to be looked at, to ensure chat bubbles
     * are created relative to where the player is - and stays there when they walk away.
     * 
     ********************************************************************************/
    pub async fn send_chat(
        &self,
        chat_type: ChatType,
        msg: &str,
        sender: Option<&Player>,
    ) -> Result<Vec<u8>> {
        let encoded = match chat_type {
            ChatType::Yell => encoding::translate_upper(&msg.to_uppercase()),
            _              => encoding::translate(msg),
        };

        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::Chat).await?;


        /********************************************************************************
         * 
         * Print chat bubble in the middle of the screen grid.
         * 
         ********************************************************************************/
        buf.write_packet(PacketOut::Chat).await?;
        let grid_x: u8 = 6;
        let grid_y: u8 = 8;
        buf.write_u8(grid_y).await?;
        buf.write_u8(grid_x).await?;
        
        buf.write_u8(chat_type as u8).await?;

        if let Some(player) = sender {
            buf.write_all(player.name.as_bytes()).await?;
            buf.write_u8(0x0009).await?; // TAB separator
        }

        buf.write_all(&encoded).await?;
        buf.write_u8(0x0000).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Chat packet to enable multiplayer communication in the game.
     * 
     ********************************************************************************/
    pub async fn build_chat_packet(
        &self,
        chat_type: ChatType,
        encoded: &[u8],
        sender_pos: crate::map::position::Position,
        sender_name: &str,
    ) -> Result<Vec<u8>> {
        /********************************************************************************
         * 
         * Get the sender's position on the map.
         * 
         ********************************************************************************/
        let dx = sender_pos.x as i32 - self.player.position.x as i32;
        let dy = sender_pos.y as i32 - self.player.position.y as i32;


        /********************************************************************************
         * 
         * Get the sender's position on screen.
         * 
         ********************************************************************************/
        let screen_x = (8 + dx).clamp(0, 14) as u8;
        let screen_y = (6 + dy).clamp(0, 11) as u8;


        /********************************************************************************
         * 
         * Print the message on the screen.
         * 
         ********************************************************************************/
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::Chat).await?;
        buf.write_u8(screen_x).await?;
        buf.write_u8(screen_y).await?;
        buf.write_u8(chat_type as u8).await?;

        if !sender_name.is_empty() {
            buf.write_all(sender_name.as_bytes()).await?;
            buf.write_u8(0x0009).await?;
        }

        buf.write_all(encoded).await?;
        buf.write_u8(0x0000).await?;

        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Renders the map that's visible to the player's viewport.
     * 
     ********************************************************************************/
    pub async fn send_map(
        &self,
        center: crate::map::position::Position,
        width: u16,
        height: u16,
    ) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::Map).await?;
        buf.write_position(center).await?;
        buf.write_all(&self.build_map_data(center, width, height).await?).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Handles "look" messages.
     * This type of message is displayed at the center of screen.
     * 
     ********************************************************************************/
    pub async fn send_look_message(&self, text: &str) -> Result<Vec<u8>> {
        let encoded = crate::chat::encoding::translate(text);
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::Chat).await?;
        buf.write_u8(8).await?;
        buf.write_u8(7).await?;
        buf.write_u8(ChatType::Look as u8).await?;
        buf.write_all(&encoded).await?;
        buf.write_u8(0x00).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Builds the raw tile payload for the requested map view.
     * 
     ********************************************************************************/
    async fn build_map_data(
        &self,
        center: crate::map::position::Position,
        width: u16,
        height: u16,
    ) -> Result<Vec<u8>> {
        use crate::map::position::Position;
        let corner = Position::new(
            center.x.saturating_sub((width - 1) / 2),
            center.y.saturating_sub((height - 1) / 2),
            center.z,
        );


        /********************************************************************************
         * 
         * Populate the map view with all tiles and items.
         * 
         ********************************************************************************/
        let mut buf = Cursor::new(Vec::new());
        for x in 0..width {
            for y in 0..height {
                let pos = Position::new(corner.x + x, corner.y + y, corner.z);
                buf.write_all(&self.build_tile(pos).await?).await?;
            }
        }


        /********************************************************************************
         * 
         * Replace last byte with 0x00FE, then append 0x000 (map terminator).
         * 
         ********************************************************************************/
        let mut data = buf.into_inner();
        if let Some(last) = data.last_mut() {
            *last = 0x00FE;
        }

        data.push(0x0000);
        Ok(data)
    }


    /********************************************************************************
     * 
     * Serializes a single tile on the map.
     * It writes:
     * - optional ground (u16) first
     * - optional player creature if this tile is the player's position
     * - then remaining objects (items/creatures)
     * - then two 0x00FF terminator bytes
     * 
     ********************************************************************************/
    async fn build_tile(&self, pos: crate::map::position::Position) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());

        if let Some(objects) = MAP.get().unwrap().get_tile_objects(pos) {
            let mut iter = objects.iter();

            /********************************************************************************
             * 
             * Write ground first (if present as the first stack element).
             * 
             ********************************************************************************/
            if let Some(first) = iter.next() {
                match first {
                    TileObject::Ground(id) => buf.write_u16_le(*id).await?,
                    _ => {}
                }
            }


            /********************************************************************************
             * 
             * Player receives the next stack position (on top of ground, but under nothing).
             * 
             ********************************************************************************/
            if pos == self.player.position {
                buf.write_all(&self.build_player_creature().await?).await?;
            }


            /********************************************************************************
             * 
             * Do the same as above, but for other players as well.
             * 
             ********************************************************************************/
            for other in &self.other_players {
                if other.position == pos {
                    buf.write_all(&self.build_other_player_creature(other).await?).await?;
                }
            }


            /********************************************************************************
             * 
             * Write remaining items (stack position 1+).
             * 
             ********************************************************************************/
            for obj in iter {
                match obj {
                    TileObject::Ground(id) | TileObject::Item(id) => {
                        buf.write_u16_le(*id).await?;
                    }
                    TileObject::Creature(id, name, outfit) => {
                        buf.write_all(&self.build_creature(*id, name, *outfit).await?).await?;
                    }
                }
            }

        } else if pos == self.player.position {
            /********************************************************************************
             * 
             * No map data exists for this tile, but the player is standing there.
             * 
             ********************************************************************************/
            buf.write_all(&self.build_player_creature().await?).await?;

        } else {
            /********************************************************************************
             * 
             * Draw other players on the tiles.
             * 
             ********************************************************************************/
            for other in &self.other_players {
                if other.position == pos {
                    buf.write_all(&self.build_other_player_creature(other).await?).await?;
                }
            }
        }

        buf.write_u8(0x00FF).await?;
        buf.write_u8(0x00FF).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Draws the player on the map.
     * The outfit sprite ID depends on the facing direction.
     * 
     ********************************************************************************/
    async fn build_player_creature(&self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());

        /********************************************************************************
         * 
         * Draw the correct outfit sprite based on direction.
         * 
         ********************************************************************************/
        let sprite = match self.player.direction {
            Direction::North => 0x00FA,
            Direction::East  => 0x00FB,
            Direction::South => 0x00FC,
            Direction::West  => 0x00FD,
        };

        buf.write_u8(sprite).await?;
        buf.write_outfit_colors(self.player.outfit).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Draws other players on the map.
     * 
     ********************************************************************************/
    async fn build_other_player_creature(&self, other: &Player) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());

        /********************************************************************************
         * 
         * Draw the correct outfit sprite based on direction.
         * 
         ********************************************************************************/
        let sprite = match other.direction {
            Direction::North => 0x00FA,
            Direction::East  => 0x00FB,
            Direction::South => 0x00FC,
            Direction::West  => 0x00FD,
        };

        buf.write_u8(sprite).await?;
        buf.write_outfit_colors(other.outfit).await?;
        Ok(buf.into_inner())
    }


    /********************************************************************************
     * 
     * Builds the creature encoding for other creatures on the map.
     * Currently it uses a simplified format:
     * - writes an aux "character" marker
     * - writes outfit colors
     * 
     * Not sure if this is used in Tibia 1.03, need to check.
     * "_id" and "_name" is accepted, but not in use.
     * 
     ********************************************************************************/
    async fn build_creature(&self, _id: u32, _name: &str, outfit: OutfitColors) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_u8(PacketOutAux::Character as u8).await?;
        buf.write_outfit_colors(outfit).await?;
        Ok(buf.into_inner())
    }
}
