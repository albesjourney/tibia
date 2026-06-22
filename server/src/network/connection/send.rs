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
    /// Pushes a packet payload onto the outgoing queue.
    ///
    /// Empty payloads are silently dropped. [`flush`](Connection::flush) drains the
    /// queue, prepends the length header, and writes everything to the socket.
    pub fn enqueue(&self, msg: Vec<u8>) {
        if !msg.is_empty() {
            self.message_queue.push(msg);
        }
    }

    /// Drains the outgoing queue and writes all pending packets to the socket.
    ///
    /// Each packet is framed as:
    /// - `u16` little-endian length (`payload_len + 2`)
    /// - raw payload bytes
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

    /// Sends the sequence of packets that establish the player's session after login.
    ///
    /// Order: login confirmation -> equipped items -> initial map view -> status message -> MotD.
    /// The map is sent here without other players; they are added in `handle_login` once the
    /// world responds with the initial [`PlayerList`](crate::world::message::WorldToPlayer::PlayerList).
    pub async fn send_login_sequence(&mut self) -> Result<()> {
        let config = CONFIG.get().unwrap();
        let pos    = self.player.position;

        // Login.
        let login = self.send_login().await?;
        self.enqueue(login);

        // Send each equipped item.
        // TODO: replace with inventory stored in a database.
        for (slot, item_id) in &self.player.equipment {
            let slot_enum: InventorySlot = match (*slot).try_into() {
                Ok(s)  => s,
                Err(_) => continue,
            };
            let msg = self.send_equipped_item(slot_enum, *item_id).await?;
            self.enqueue(msg);
        }

        // Draw the map, send status message and Message of the Day.
        let map = self.send_map(pos, 18, 14).await?;
        let status = self.send_status_message(&config.server.status_message).await?;
        let motd = self.send_message_of_the_day(&config.server.message_of_the_day).await?;
        self.enqueue(map);
        self.enqueue(status);
        self.enqueue(motd);

        Ok(())
    }

    /// Builds a login confirmation packet.
    async fn send_login(&self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::Login).await?;
        Ok(buf.into_inner())
    }

    /// Builds a status bar message packet (displayed in the bottom-left of the client).
    pub async fn send_status_message(&self, message: &str) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::StatusMessage).await?;
        buf.write_null_terminated_string(message).await?;
        Ok(buf.into_inner())
    }

    /// Builds a Message of the Day packet (displayed upon login).
    pub async fn send_message_of_the_day(&self, message: &str) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::MessageOfTheDay).await?;
        buf.write_null_terminated_string(message).await?;
        Ok(buf.into_inner())
    }

    /// Builds a packet that places `item_id` into the given inventory `slot`.
    pub async fn send_equipped_item(&self, slot: InventorySlot, item_id: u16) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::EquippedItem).await?;
        buf.write_u16_le(item_id).await?;
        buf.write_u8(slot as u8).await?;
        Ok(buf.into_inner())
    }

    /// Builds a packet that clears the given inventory slot on the client.
    pub async fn send_remove_equipped(&self, slot: u8) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::RemoveEquipped).await?;
        buf.write_u8(slot).await?;
        Ok(buf.into_inner())
    }

    /// Builds the "Change Data" window packet, pre-filled with the player's current profile.
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

    /// Builds the user list packet (Menu: Info -> Userlist).
    ///
    /// The requesting player's own name is written first, followed by all other online players,
    /// each separated by a newline and terminated by a null byte.
    pub async fn send_user_list(&self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::UserList).await?;
        buf.write_u16_le(0x1010).await?;

        buf.write_all(self.player.name.as_bytes()).await?;
        buf.write_u8(b'\n').await?;

        for other in &self.other_players {
            buf.write_all(other.name.as_bytes()).await?;
            buf.write_u8(b'\n').await?;
        }

        buf.write_u8(0).await?;
        Ok(buf.into_inner())
    }

    /// Builds the player info popup packet (Menu: Info -> Userlist -> {player} -> Info).
    pub async fn send_user_info(&self, player: &Player) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());

        buf.write_packet(PacketOut::UserInfo).await?;
        buf.write_u16_le(0x1010).await?;

        buf.write_null_terminated_string(&format!(
            "Name: {}\nReal name: {}\nLocation: {}\nEmail: {}\nSex: {:?}",
            player.name,
            player.real_name,
            player.location,
            player.email,
            player.gender,
        )).await?;
        Ok(buf.into_inner())
    }

    /// Builds an open-container packet for the given container window.
    ///
    /// `c.local_id` is the window slot on the client (0-based). Sending the same
    /// `local_id` again replaces the existing window in-place, which is used both
    /// when adding items and when navigating into a sub-container. The icon always
    /// comes from `c.container_id`, never from the items placed inside it.
    pub async fn send_open_container(&self, c: &crate::player::OpenContainer) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::OpenContainer).await?;
        buf.write_u8(c.local_id).await?;
        buf.write_u16_le(c.container_id).await?;

        for item_id in &c.items {
            buf.write_u16_le(*item_id).await?;
        }

        buf.write_u16_le(0xFFFF).await?;
        Ok(buf.into_inner())
    }

    /// Builds a close-container packet.
    ///
    /// `local_id` must match the id used when the container was opened.
    pub async fn send_close_container(&self, local_id: u8) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::CloseContainer).await?;
        buf.write_u8(local_id).await?;
        Ok(buf.into_inner())
    }

    /// Builds a chat packet anchored to the sender's screen position.
    ///
    /// The sender's map position is converted to a screen-space coordinate relative to
    /// this player's viewport and clamped to the visible area. The chat bubble will
    /// appear at that screen position on the client.
    ///
    /// TODO: chat bubble position currently snaps to the clamped screen coordinate and does
    /// not track the sender if they walk away mid-message.
    pub async fn build_chat_packet(
        &self,
        chat_type: ChatType,
        encoded: &[u8],
        sender_pos: crate::map::position::Position,
        sender_name: &str,
    ) -> Result<Vec<u8>> {
        // Sender's position on the map
        let dx = sender_pos.x as i32 - self.player.position.x as i32;
        let dy = sender_pos.y as i32 - self.player.position.y as i32;

        // Sender's position on screen
        let screen_x = (8 + dx).clamp(0, 14) as u8;
        let screen_y = (6 + dy).clamp(0, 11) as u8;

        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::Chat).await?;
        buf.write_u8(screen_x).await?;
        buf.write_u8(screen_y).await?;
        buf.write_u8(chat_type as u8).await?;

        if !sender_name.is_empty() {
            match chat_type {
                // For broadcasts ("#B") the string "Broadcast from" is prepended to the message.
                ChatType::Broadcast => {
                    buf.write_all(b"Broadcast from ").await?;
                    buf.write_all(sender_name.as_bytes()).await?;
                    buf.write_u8(0x0009).await?; // TAB separator between name and message
                }

                // For private messages the string "Message from" is prepended to the message.
                ChatType::Private => {
                    buf.write_all(b"Message from ").await?;
                    buf.write_all(sender_name.as_bytes()).await?;
                    buf.write_u8(0x0009).await?; // TAB separator between name and message
                }

                _ => {
                    buf.write_all(sender_name.as_bytes()).await?;
                    buf.write_u8(0x0009).await?; // TAB separator between name and message
                }
            }
        }

        // Write the message
        buf.write_all(encoded).await?;
        buf.write_u8(0x0000).await?;
        Ok(buf.into_inner())
    }

    /// Builds a look message packet, displayed at the centre of the player's screen.
    pub async fn send_look_message(&self, text: &str) -> Result<Vec<u8>> {
        let encoded = crate::chat::encoding::translate(text);
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::Chat).await?;
        buf.write_u8(8).await?;
        buf.write_u8(7).await?;
        buf.write_u8(ChatType::Look as u8).await?;
        buf.write_all(&encoded).await?;
        buf.write_u8(0x0000).await?;
        Ok(buf.into_inner())
    }

    /// Builds the full map packet for a `width * height` viewport centered on `center`.
    pub async fn send_map(
        &self,
        center: crate::map::position::Position,
        width:  u16,
        height: u16,
    ) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());
        buf.write_packet(PacketOut::Map).await?;
        buf.write_position(center).await?;
        buf.write_all(&self.build_map_data(center, width, height).await?).await?;
        Ok(buf.into_inner())
    }

    /// Builds the raw tile payload for a `width * height` viewport centered on `center`.
    ///
    /// After all tiles are serialized, the last byte is replaced with `0x00FE` and
    /// a `0x0000` map terminator is appended, as required by the Tibia 1.03 protocol.
    async fn build_map_data(
        &self,
        center: crate::map::position::Position,
        width:  u16,
        height: u16,
    ) -> Result<Vec<u8>> {
        use crate::map::position::Position;

        let corner = Position::new(
            center.x.saturating_sub((width  - 1) / 2),
            center.y.saturating_sub((height - 1) / 2),
            center.z,
        );

        // Write all tiles on the map (ground, items, etc)
        let mut buf = Cursor::new(Vec::new());
        for x in 0..width {
            for y in 0..height {
                let pos = Position::new(corner.x + x, corner.y + y, corner.z);
                buf.write_all(&self.build_tile(pos).await?).await?;
            }
        }

        let mut data = buf.into_inner();
        if let Some(last) = data.last_mut() {
            *last = 0x00FE;
        }

        data.push(0x0000);
        Ok(data)
    }

    /// Serializes a single tile into its wire format.
    ///
    /// Write order within a tile:
    /// 1. Ground object (`u16` id), if present.
    /// 2. This player's creature data, if standing on this tile.
    /// 3. Other players' creature data, for any standing on this tile.
    /// 4. Remaining stack objects (items and non-player creatures).
    /// 5. Two `0x00FF` terminator bytes.
    async fn build_tile(&self, pos: crate::map::position::Position) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());

        if let Some(objects) = MAP.get().unwrap().read().await.get_tile_objects(pos) {
            let mut iter = objects.iter();

            // Ground is always the first element in the stack.
            if let Some(TileObject::Ground(id)) = iter.next() {
                buf.write_u16_le(*id).await?;
            }

            // Player receives the next stack position (on top of ground, but under nothing).
            if pos == self.player.position {
                buf.write_all(&self.build_player_creature().await?).await?;
            }

            // Do the same as above, but for other players as well.
            for other in &self.other_players {
                if other.position == pos {
                    buf.write_all(&self.build_other_player_creature(other).await?).await?;
                }
            }

            // Write remaining items (stack position 1+).
            for obj in iter {
                match obj {
                    TileObject::Ground(id) | TileObject::Item(id) => {
                        buf.write_u16_le(*id).await?;
                    }
                }
            }
        } else if pos == self.player.position {
            // No map data for this tile, but the player is here - render them anyway.
            buf.write_all(&self.build_player_creature().await?).await?;
        } else {
            // Draw other players on the tiles.
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

    /// Builds the creature bytes for this player.
    ///
    /// The sprite id encodes the facing direction; outfit colors follow immediately after.
    async fn build_player_creature(&self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());

        // Write the correct outfit sprite based on direction.
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

    /// Builds the creature bytes for other players.
    async fn build_other_player_creature(&self, other: &Player) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::new());

        // Write the correct outfit sprite based on direction.
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
}
