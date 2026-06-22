use super::Connection;
use crate::{
    chat::ChatType,
    debug_log,
    io::ReadExt,
    network::packet::PacketIn,
    player::Direction,
};
use anyhow::Result;
use std::{convert::TryInto, io::Cursor};
use tokio::io::{AsyncRead, AsyncReadExt};

impl Connection {
    /// Reads the packet id from `buf` and dispatches it to the appropriate handler.
    ///
    /// Unknown packet ids are logged and silently ignored.
    pub async fn handle_packet(&mut self, buf: &[u8]) -> Result<()> {
        let mut cursor = Cursor::new(buf);

        match cursor.read_u16_le().await?.try_into() {
            Ok(packet) => {
                // Packet sniffer
                // Uncomment to see decoded packet information and raw packet bytes.
                // Useful when reverse engineering or debugging packet structures.
                debug_log!(
                    "network/connection/receive::handle_packet -> Received incoming packet:\n\
                    \tType: {:?}\n\
                    \tSize: {} bytes\n\
                    \tRaw packet data: {:02X?}",
                    packet,
                    buf.len(),
                    buf
                );

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

    /// Routes a decoded [`PacketIn`] to its handler method.
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
            PacketIn::ChangeDirection   => { /* not observed in Tibia 1.03 */ }
            PacketIn::Comment           => self.recv_comment(r).await?,
            PacketIn::Push              => self.recv_push(r).await?,
            PacketIn::UseItem           => self.recv_use_item(r).await?,
            PacketIn::CloseContainer    => self.recv_close_container(r).await?,
            PacketIn::RequestChangeData => self.recv_request_change_data(r).await?,
            PacketIn::SetData           => self.recv_set_data(r).await?,
            PacketIn::Echo              => { /* no operation */ }
            PacketIn::Logout            => { /* no operation; save player state here in future */ }
        }
        Ok(())
    }

    /// Handles a walk packet - reads the direction, advances the player one tile, and redraws the map.
    ///
    /// In Tibia 1.03 the full map is resent every time a player moves.
    /// After moving, any open container whose source is a map tile that is now more than
    /// 1 sqm away from the player is automatically closed, matching original Tibia behaviour.
    async fn recv_walk<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        let direction: Direction = r.read_u8().await?.try_into()?;
        self.player.direction    = direction;
        self.player.position     = self.player.position + direction;
        let new_pos              = self.player.position;

        // Tell everyone online that the player has walked.
        self.world_sender.send(
            crate::world::message::PlayerToWorld::UpdatePosition(self.player.clone())
        )?;

        // Auto-close any map containers that are now out of range (more than 1 sqm away).
        let mut to_close: Vec<u8> = Vec::new();
        for c in &self.player.containers {
            if let crate::player::ContainerSource::Map(container_pos) = c.source {
                let dx = (container_pos.x as i32 - new_pos.x as i32).abs();
                let dy = (container_pos.y as i32 - new_pos.y as i32).abs();
                if dx > 1 || dy > 1 {
                    to_close.push(c.local_id);
                }
            }
        }
        for local_id in to_close {
            self.player.containers.retain(|c| c.local_id != local_id);
            let msg = self.send_close_container(local_id).await?;
            self.enqueue(msg);
        }

        // Send the map; in Tibia 1.03 the map is redrawn every time a player walks.
        let msg = self.send_map(new_pos, 18, 14).await?;
        self.enqueue(msg);
        Ok(())
    }

    /// Handles an incoming chat packet.
    ///
    /// Messages prefixed with `#` are forwarded to [`recv_qualified_chat`] to determine
    /// the chat type (whisper, yell, broadcast). Messages prefixed with `*Name*` or
    /// `@Name@` are forwarded to [`recv_private_chat`]. Everything else is sent as
    /// [`ChatType::Normal`].
    async fn recv_chat<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        let length = r.read_u16_le().await?;
        let mut raw = vec![0u8; length as usize];
        r.read_exact(&mut raw).await?;
        let msg = unsafe { String::from_utf8_unchecked(raw) };

        if msg.starts_with('#') {
            self.recv_qualified_chat(&msg).await?;
        } else if msg.starts_with('*') || msg.starts_with('@') {
            self.recv_private_chat(&msg).await?;
        } else {
            let encoded = crate::chat::encoding::translate(&msg);
            self.world_sender.send(crate::world::message::PlayerToWorld::Chat(
                self.player.position,
                ChatType::Normal,
                encoded,
                self.player.name.clone(),
                None,
            ))?;
        }
        Ok(())
    }

    /// Parses a private chat string and forwards it to the world.
    ///
    /// The receiver's name is delimited by a matching pair of `*` or `@` characters
    /// (e.g. `*Gandalf*hello there` or `@Gandalf@hello there`); the client distinguishes
    /// the two only in how it pre-fills the chat box for the next message - the server
    /// treats both identically. Names may contain spaces. A single space directly after
    /// the closing delimiter is skipped if present, but is not required. If the closing
    /// delimiter is missing entirely, the message is discarded.
    async fn recv_private_chat(&mut self, msg: &str) -> Result<()> {
        let delimiter = msg.chars().next().unwrap();
        let rest = &msg[1..];

        let Some(end) = rest.find(delimiter) else {
            debug_log!(
                "network/connection/receive::recv_private_chat -> Missing closing '{}' in: {:?}",
                delimiter,
                msg
            );
            return Ok(());
        };

        let receiver_name = rest[..end].to_string();
        let mut text = &rest[end + delimiter.len_utf8()..];
        text = text.strip_prefix(' ').unwrap_or(text);
        let encoded = crate::chat::encoding::translate(text);

        self.world_sender.send(crate::world::message::PlayerToWorld::Chat(
            self.player.position,
            ChatType::Private,
            encoded,
            self.player.name.clone(),
            Some(receiver_name),
        ))?;
        Ok(())
    }

    /// Parses a qualified chat string and forwards it to the world.
    ///
    /// The character at index 1 determines the [`ChatType`] (see `src/chat/mod.rs`).
    /// Yell and broadcast are uppercased before encoding. Unknown qualifiers are logged
    /// and discarded.
    async fn recv_qualified_chat(&mut self, msg: &str) -> Result<()> {
        use std::convert::TryInto;

        match msg.chars().nth(1).try_into() {
            Ok(chat_type) => {
                let encoded = match chat_type {
                    ChatType::Yell | ChatType::Broadcast => {
                        crate::chat::encoding::translate_upper(&msg[3..].to_uppercase())
                    }
                    _ => crate::chat::encoding::translate(&msg[3..]),
                };
                
                self.world_sender.send(crate::world::message::PlayerToWorld::Chat(
                    self.player.position,
                    chat_type,
                    encoded,
                    self.player.name.clone(),
                    None,
                ))?;
            }
            Err(_) => {
                debug_log!(
                    "network/connection/receive::recv_qualified_chat -> Unknown chat qualifier: {:?}",
                    msg.chars().nth(1)
                );
            }
        }
        Ok(())
    }

    /// Handles a player comment - logged server-side only, not displayed in game.
    ///
    /// TODO: store the comment in a database.
    async fn recv_comment<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        let mut msg = String::new();
        r.read_string_to_end(&mut msg).await?;
        debug_log!("network/connection/receive::recv_comment -> Player {} submitted a comment:\n\t{}", self.player.name, msg);
        Ok(())
    }

    /// Handles a request to open the "Change Data" window.
    ///
    /// Responds with the player's current editable fields pre-filled.
    async fn recv_request_change_data<R: AsyncRead + Unpin>(&mut self, _r: &mut R) -> Result<()> {
        let msg = self.send_data_window().await?;
        self.enqueue(msg);
        Ok(())
    }

    /// Handles a "Change Data" form submission - updates the player's profile fields.
    ///
    /// After updating, the new data is broadcast to the world and the map is redrawn
    /// so outfit changes are visible to other players immediately.
    ///
    /// TODO: persist these fields to a database.
    async fn recv_set_data<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        let mut password = String::new();
        r.read_string(&mut password, 30).await?;
        let outfit = r.read_outfit_colors().await?;

        let mut real_name = String::new();
        r.read_string(&mut real_name, 50).await?;

        let mut location = String::new();
        r.read_string(&mut location, 50).await?;

        let mut email = String::new();
        r.read_string(&mut email, 50).await?;

        self.player.password  = password;
        self.player.outfit    = outfit;
        self.player.real_name = real_name;
        self.player.location  = location;
        self.player.email     = email;

        self.world_sender.send(
            crate::world::message::PlayerToWorld::UpdateInfo(self.player.clone())
        )?;

        // Outfit changes require a full map redraw.
        let pos = self.player.position;
        let msg = self.send_map(pos, 18, 14).await?;
        self.enqueue(msg);
        Ok(())
    }

    /// Handles a request for displaying the online list (Menu: Info -> Userlist).
    async fn recv_user_list<R: AsyncRead + Unpin>(&mut self, _r: &mut R) -> Result<()> {
        let msg = self.send_user_list().await?;
        self.enqueue(msg);
        Ok(())
    }

    /// Handles a request for displaying a specific player's info (Menu: Info -> Userlist -> {player} -> Info).
    ///
    /// Checks `self` first, then falls back to `other_players`. If the name is not found,
    /// the requesting player's own data is returned.
    async fn recv_player_info<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        let mut name = String::new();
        r.read_string_to_end(&mut name).await?;

        let player = if self.player.name == name {
            self.player.clone()
        } else {
            self.other_players.iter().find(|p| p.name == name).cloned().unwrap_or(self.player.clone())
        };

        let msg = self.send_user_info(&player).await?;
        self.enqueue(msg);
        Ok(())
    }

    /// Handles an auto-walk request.
    ///
    /// The client sends the clicked screen tile; this is converted to a map position and
    /// a BFS path is computed from the player's current position. The resulting step list
    /// is stored in `pending_path` and consumed one step per run-loop tick.
    ///
    /// Walkability rules:
    /// - Tiles with blocking objects (walls, water, etc.) are impassable.
    /// - Tiles occupied by other players are impassable.
    /// - Empty in-bounds tiles are passable.
    /// - Out-of-bounds tiles are impassable.
    /// - The destination tile itself skips the walkability check so players can click
    ///   toward a wall and still path as close as possible.
    ///
    /// The BFS is capped at 400 visited tiles to avoid hanging on unreachable destinations.
    async fn recv_auto_walk<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        let screen_pos = r.read_position().await?;

        // The player sits at screen position (8, 6). Valid click area is x: 2–14, y: 2–10.
        if screen_pos.x < 2 || screen_pos.x > 14 || screen_pos.y < 2 || screen_pos.y > 10 {
            return Ok(());
        }

        let dest_x = self.player.position.x as i32 + (screen_pos.x as i32 - 8);
        let dest_y = self.player.position.y as i32 + (screen_pos.y as i32 - 6);
        let dest = crate::map::position::Position::new(
            dest_x as u16,
            dest_y as u16,
            self.player.position.z,
        );

        let map = crate::map::MAP.get().unwrap().read().await;

        use std::collections::{HashMap, VecDeque};

        // BFS (Breadth-First Search) pathfinding.
        //
        // We explore tiles outward from the player's position, tracking which tile we came from
        // and which direction we moved to get there. This guarantees the shortest walkable path.
        let start = self.player.position;
        let mut queue: VecDeque<crate::map::position::Position> = VecDeque::new();
        let mut came_from: HashMap<crate::map::position::Position, (crate::map::position::Position, Direction)> = HashMap::new();

        queue.push_back(start);
        came_from.insert(start, (start, Direction::South));

        'bfs: while let Some(current) = queue.pop_front() {
            if current == dest {
                break 'bfs;
            }

            for (dir, dx, dy) in &[
                (Direction::North,  0i32, -1i32),
                (Direction::East,   1,     0),
                (Direction::South,  0,     1),
                (Direction::West,  -1,     0),
            ] {
                let nx = current.x as i32 + dx;
                let ny = current.y as i32 + dy;
                if nx < 0 || ny < 0 { continue; }

                let next = crate::map::position::Position::new(nx as u16, ny as u16, current.z);

                // Skip tiles we have already visited.
                if came_from.contains_key(&next) { continue; }

                if next != dest {
                    // Block tiles occupied by other players.
                    if self.other_players.iter().any(|p| p.position == next) {
                        continue;
                    }

                    let walkable = match map.get_tile_objects(next) {
                        Some(objects) if !objects.is_empty() => objects.iter().all(|obj| {
                            let id = match obj {
                                crate::map::TileObject::Ground(id)      => *id,
                                crate::map::TileObject::Item(id)        => *id,
                            };
                            crate::map::items::is_walkable(id)
                        }),
                        Some(_) => true,  // empty tile within bounds = walkable
                        None    => false, // out of bounds = not walkable
                    };
                    if !walkable { continue; }
                }

                came_from.insert(next, (current, *dir));
                queue.push_back(next);

                // Safety limit to avoid hanging if the destination is unreachable.
                // If we have explored 400 tiles without finding the destination,
                // we stop and the player will not move.
                if came_from.len() > 400 { break 'bfs; }
            }
        }

        // Reconstruct the path by walking backwards from the destination to the start,
        // following the came_from map. Then reverse it so it goes start -> destination.
        let mut path = Vec::new();
        if came_from.contains_key(&dest) {
            let mut current = dest;
            while current != start {
                let (prev, dir) = came_from[&current];
                path.push(dir);
                current = prev;
            }
            path.reverse();
        }

        self.pending_path = path;

        if self.pending_path.is_empty() {
            let msg = self.send_status_message("Sorry, not possible.").await?;
            self.enqueue(msg);
        }

        Ok(())
    }

    /// Handles a "look at" request - triggered when the player examines a tile or creature.
    ///
    /// Priority order: other players -> self -> items -> ground tiles -> nothing.
    /// Screen coordinates are converted to map coordinates before the lookup.
    async fn recv_look_at<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        let screen_pos = r.read_position().await?;
        let map_x = self.player.position.x as i32 + (screen_pos.x as i32 - 8);
        let map_y = self.player.position.y as i32 + (screen_pos.y as i32 - 6);
        let map_pos = crate::map::position::Position::new(
            map_x as u16,
            map_y as u16,
            self.player.position.z,
        );

        let text = if let Some(other) = self.other_players.iter().find(|p| p.position == map_pos) {
            // Looking at another player.
            format!(
                "You see {}. {} is {}.\n[Position: {}, {}, {}]",
                other.name,
                match other.gender { crate::player::Gender::Female => "She", _ => "He" },
                match other.gender { crate::player::Gender::Female => "female", _ => "male" },
                other.position.x,
                other.position.y,
                other.position.z,
            )
        } else if map_pos == self.player.position {
            // Looking at yourself.
            format!(
                "You see yourself. You are {}.\n[Position: {}, {}, {}]",
                match self.player.gender { crate::player::Gender::Female => "female", _ => "male" },
                self.player.position.x,
                self.player.position.y,
                self.player.position.z,
            )
        } else if let Some(objects) = crate::map::MAP.get().unwrap().read().await.get_tile_objects(map_pos) {
            // Prefer the topmost item; fall back to the ground tile.
            let top = objects
                .iter()
                .find(|o| matches!(o, crate::map::TileObject::Item(_)))
                .or_else(|| objects.first().filter(|o| matches!(o, crate::map::TileObject::Ground(_))));

            match top {
                // Looking at items and ground tiles.
                Some(crate::map::TileObject::Item(id)) | Some(crate::map::TileObject::Ground(id)) => format!(
                    "You see {}.\n[Item id: {}]\n[Position: {}, {}, {}]",
                    crate::map::items::item_name(*id),
                    id,
                    map_pos.x,
                    map_pos.y,
                    map_pos.z,
                ),
                _ => format!(
                    "You see an unknown object.\n[Position: {}, {}, {}]",
                    map_pos.x, map_pos.y, map_pos.z,
                ),
            }
        } else {
            format!(
                "You see nothing.\n[Position: {}, {}, {}]",
                map_pos.x, map_pos.y, map_pos.z,
            )
        };

        let msg = self.send_look_message(&text).await?;
        self.enqueue(msg);
        Ok(())
    }

    /// Handles an item push (drag) request - moves an item between map tiles,
    /// equipment slots, and container slots in any combination.
    ///
    /// Coordinate encoding in the packet:
    /// - `sx = 0x00FF`, `sy = 1–8`   -> equipment slot (slot number = `sy`)
    /// - `sx = 0x00FF`, `sy ≥ 0x14`  -> container window (local_id = `sy - 0x14`)
    /// - any other `sx`              -> map tile (relative to player screen center at 8,6)
    ///
    /// Rules enforced:
    /// - Only items with `props.movable == true` may be moved from the map at all.
    /// - Picking up from the map into a container or equipment slot additionally requires
    ///   `props.range == 8` (long-range items only). Short-range items such as barrels
    ///   and troughs can only be pushed between map tiles.
    /// - For map->map moves: range, line-of-sight, walkability, and player-occupancy
    ///   checks are applied.
    /// - The source tile is verified to actually contain the expected item before removal.
    ///   If the item is not there (e.g. the player moved since mouse-down), the push is
    ///   rejected with no side-effects.
    /// - Dragging an item onto an equipment slot that holds a container puts the item
    ///   inside that container rather than replacing the equipped container.
    /// - When an item is picked up from the map into a container, the map is redrawn
    ///   first so the item disappears before the container window updates.
    /// - Moving an item into a container updates all open windows that show the same
    ///   container (identified by container_id and source), so duplicate windows stay
    ///   in sync.
    async fn recv_push<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        let from_screen = r.read_position().await?;
        let item_id     = r.read_u16_le().await?;
        let _stack_pos  = r.read_u8().await?;
        let to_screen   = r.read_position().await?;

        #[derive(Debug)]
        enum Location {
            Map(crate::map::position::Position),
            Equipment(u8), // equipment slot number (1–8)
            Container(u8), // local_id of the target container window
        }

        let decode = |sx: u16, sy: u16| -> Location {
            if sx == 0x00FF {
                if sy >= 0x0014 {
                    Location::Container((sy - 0x0014) as u8)
                } else {
                    Location::Equipment(sy as u8)
                }
            } else {
                let mx = self.player.position.x as i32 + (sx as i32 - 8);
                let my = self.player.position.y as i32 + (sy as i32 - 6);

                Location::Map(crate::map::position::Position::new(
                    mx as u16,
                    my as u16,
                    self.player.position.z,
                ))
            }
        };

        let from  = decode(from_screen.x, from_screen.y);
        let to    = decode(to_screen.x,   to_screen.y);
        let props = crate::map::items::item_properties(item_id);

        // If the source is a map tile, verify it is within the item's
        // throw range from the player. If the player walked during the drag, the
        // screen coordinate in from_screen now resolves to a shifted tile. Because the
        // range check here is strict (≤ props.range), an accidentally shifted tile
        // for a short-range item (range=2) would be at distance 3+ and is rejected,
        // preventing the wrong item from being moved.
        if let Location::Map(from_pos) = &from {
            let player_pos = self.player.position;
            let dx = (from_pos.x as i32 - player_pos.x as i32).abs();
            let dy = (from_pos.y as i32 - player_pos.y as i32).abs();
            
            // The player must be standing on or adjacent (≤1 sqm) to the source tile
            // for any map item - regardless of the item's throw range. Throw range only
            // affects how far the item can travel, not how far away the player can be.
            if dx > 1 || dy > 1 {
                let msg = self.send_status_message("Sorry, not possible.").await?;
                self.enqueue(msg);
                return Ok(());
            }
        }

        // Remove the item from its source location.
        let removed = match &from {
            Location::Map(from_pos) => {
                // Any move from the map requires the item to be movable.
                if !props.movable {
                    let msg = self.send_status_message("Sorry, not possible.").await?;
                    self.enqueue(msg);
                    return Ok(());
                }

                // Picking up into a container or equipment slot requires long range (8).
                // Short-range items (barrels, troughs, etc.) can only be pushed on the map.
                let picking_up = matches!(to, Location::Container(_) | Location::Equipment(_));
                if picking_up && props.range < 8 {
                    let msg = self.send_status_message("Sorry, not possible.").await?;
                    self.enqueue(msg);
                    return Ok(());
                }

                // Verify the item is actually on the exact tile encoded in from_screen.
                // If the player moved since mouse-down, the screen coordinate now resolves
                // to a different map tile; reject rather than moving the wrong item.
                let map = crate::map::MAP.get().unwrap().read().await;
                let item_present = map
                    .get_tile_objects(*from_pos)
                    .map(|objs| objs.iter().any(|o| {
                        matches!(o, crate::map::TileObject::Item(id) if *id == item_id)
                    }))
                    .unwrap_or(false);
                drop(map);

                if !item_present {
                    let msg = self.send_status_message("Sorry, not possible.").await?;
                    self.enqueue(msg);
                    return Ok(());
                }

                // Remove from the map now. For map->container moves we redraw the map
                // immediately after removal (before sending the container update) so the
                // item disappears from the ground before it appears in the container.
                let did_remove = crate::map::MAP.get().unwrap().write().await.remove_item(*from_pos, item_id);
                if did_remove && matches!(to, Location::Container(_) | Location::Equipment(_)) {
                    let pos = self.player.position;
                    let map_pkt = self.send_map(pos, 18, 14).await?;
                    self.enqueue(map_pkt);
                    self.world_sender.send(crate::world::message::PlayerToWorld::MapUpdate(pos))?;
                }
                did_remove
            }

            // Pushing items to the equipment.
            Location::Equipment(slot) => {
                if self.player.equipment.get(slot) == Some(&item_id) {
                    self.player.equipment.remove(slot);
                    let msg = self.send_remove_equipped(*slot).await?;
                    self.enqueue(msg);

                    let moving_to_equip = matches!(&to, Location::Equipment(_));

                    let mut to_close: Vec<u8> = Vec::new();
                    for c in &self.player.containers {
                        if c.source == crate::player::ContainerSource::Equipment(*slot) {
                            // Always persist the live window's contents.
                            self.player.equipment_container_contents.insert(*slot, c.items.clone());
                            if !moving_to_equip {
                                to_close.push(c.local_id);
                            }
                        }
                    }

                    // If no window was open the equipment_container_contents entry from
                    // the last time the bag was closed is already the authoritative copy.
                    for local_id in to_close {
                        self.player.containers.retain(|c| c.local_id != local_id);
                        let msg = self.send_close_container(local_id).await?;
                        self.enqueue(msg);
                    }
                    true
                } else {
                    false
                }
            }

            Location::Container(local_id) => {
                // Find the window, remove the first matching item, and refresh the window.
                let mut did_remove = false;
                if let Some(c) = self.player.containers.iter_mut().find(|c| c.local_id == *local_id) {
                    if let Some(idx) = c.items.iter().position(|&id| id == item_id) {
                        c.items.remove(idx);
                        did_remove = true;
                    }
                }

                if did_remove {
                    // Refresh the source container window so the client sees the item removed.
                    if let Some(c) = self.player.containers.iter().find(|c| c.local_id == *local_id) {
                        let c_clone = c.clone();
                        let msg = self.send_open_container(&c_clone).await?;
                        self.enqueue(msg);
                    }
                }
                did_remove
            }
        };

        // If we just removed an item from a map-sourced container, persist the new contents.
        if removed {
            if let Location::Container(local_id) = &from {
                if let Some(c) = self.player.containers.iter().find(|c| c.local_id == *local_id) {
                    match c.source {
                        crate::player::ContainerSource::Map(map_pos) => {
                            crate::map::MAP.get().unwrap().write().await
                                .set_container_contents(map_pos, c.items.clone());
                            self.world_sender.send(crate::world::message::PlayerToWorld::ContainerUpdate(
                                map_pos,
                                c.container_id,
                                c.items.clone(),
                            ))?;
                        }

                        crate::player::ContainerSource::Equipment(slot) => {
                            self.player.equipment_container_contents.insert(slot, c.items.clone());
                        }
                    }
                }
            }
        }

        if !removed {
            return Ok(());
        }

        // If the destination is a map tile that holds a container item but no open
        // window for it exists, reject the move entirely.
        if let Location::Map(to_pos) = &to {
            let map = crate::map::MAP.get().unwrap().read().await;
            let to_has_container = map
                .get_tile_objects(*to_pos)
                .map(|objs| objs.iter().any(|o| {
                    let id = match o {
                        crate::map::TileObject::Ground(id) => *id,
                        crate::map::TileObject::Item(id)   => *id,
                    };
                    crate::map::items::is_container(id)
                }))
                .unwrap_or(false);
            drop(map);

            if to_has_container {
                // The destination tile contains a container that is not open.
                // Put the item back where it came from and reject the move.
                match &from {
                    Location::Equipment(slot) => {
                        self.player.equipment.insert(*slot, item_id);
                        let slot_enum: crate::player::InventorySlot = match (*slot).try_into() {
                            Ok(s)  => s,
                            Err(_) => return Ok(()),
                        };
                        let msg = self.send_equipped_item(slot_enum, item_id).await?;
                        self.enqueue(msg);
                    }

                    Location::Map(from_pos) => {
                        crate::map::MAP.get().unwrap().write().await.add_item_to_tile(*from_pos, item_id);
                    }

                    Location::Container(local_id) => {
                        if let Some(c) = self.player.containers.iter_mut().find(|c| c.local_id == *local_id) {
                            c.items.push(item_id);
                            let c_clone = c.clone();
                            let msg = self.send_open_container(&c_clone).await?;
                            self.enqueue(msg);
                        }
                    }
                }

                let msg = self.send_status_message("Sorry, not possible.").await?;
                self.enqueue(msg);
                return Ok(());
            }
        }

        // Resolve the effective destination.
        //
        // If the player drags an equipment item onto an equipment slot that
        // currently holds a container, treat it as dropping into it.
        let effective_to = match &to {
            Location::Equipment(slot) => {
                if let Some(&held_id) = self.player.equipment.get(slot) {
                    if crate::map::items::is_container(held_id) {
                        // Drag onto a container in equipment -> drop item into that container.
                        Location::Container(*slot)
                    } else {
                        // Slot is occupied by a non-container item - reject the move.
                        // Restore item to source before returning.
                        match &from {
                            Location::Map(fp) => {
                                crate::map::MAP.get().unwrap().write().await.add_item_to_tile(*fp, item_id);
                            }

                            Location::Equipment(old_slot) => {
                                self.player.equipment.insert(*old_slot, item_id);
                                let slot_enum: crate::player::InventorySlot = match (*old_slot).try_into() {
                                    Ok(s)  => s,
                                    Err(_) => return Ok(()),
                                };
                                let msg = self.send_equipped_item(slot_enum, item_id).await?;
                                self.enqueue(msg);
                            }

                            Location::Container(local_id) => {
                                if let Some(c) = self.player.containers.iter_mut().find(|c| c.local_id == *local_id) {
                                    c.items.push(item_id);
                                    let c_clone = c.clone();
                                    let msg = self.send_open_container(&c_clone).await?;
                                    self.enqueue(msg);
                                }
                            }
                        }

                        let msg = self.send_status_message("Sorry, not possible.").await?;
                        self.enqueue(msg);
                        return Ok(());
                    }
                } else {
                    Location::Equipment(*slot)
                }
            }

            Location::Container(local_id) => Location::Container(*local_id),
            Location::Map(pos) => Location::Map(*pos),
        };

        // Place the item at its destination.
        match &effective_to {
            // Pushing items to the map.
            Location::Map(to_pos) => {
                if let Location::Map(from_pos) = &from {
                    let player_pos = self.player.position;

                    // Range check: destination must be within range of the *item*, not the player.
                    // The source-tile guard above already ensures the item is near the player.
                    let dx = (to_pos.x as i32 - from_pos.x as i32).abs();
                    let dy = (to_pos.y as i32 - from_pos.y as i32).abs();

                    // For short-range items (e.g. troughs, range == 2): the destination must
                    // be within props.range of the *player*, not the item. This matches the
                    // original Tibia behaviour where a trough at x+1 can be pushed to x-1
                    // (distance 2 from player, which equals props.range).
                    // For long-range throwables (range == 8): destination is measured from
                    // the item tile, as before.
                    let (ref_pos, max_to_dist) = if props.range < 8 {
                        (player_pos, props.range as i32)
                    } else {
                        (*from_pos, props.range as i32)
                    };

                    let dx = (to_pos.x as i32 - ref_pos.x as i32).abs();
                    let dy = (to_pos.y as i32 - ref_pos.y as i32).abs();

                    if dx > max_to_dist || dy > max_to_dist {
                        crate::map::MAP.get().unwrap().write().await.add_item_to_tile(*from_pos, item_id);
                        let msg = self.send_status_message("Sorry, not possible.").await?;
                        self.enqueue(msg);
                        return Ok(());
                    }

                    // Line-of-sight check: walk from the item's tile to the destination,
                    // not from the player. A wall between the item and its target blocks
                    // the throw regardless of where the player is standing.
                    let steps = dx.max(dy);
                    let mut blocked = false;
                    for i in 1..steps {
                        let cx = (from_pos.x as i32 + (to_pos.x as i32 - from_pos.x as i32) * i / steps) as u16;
                        let cy = (from_pos.y as i32 + (to_pos.y as i32 - from_pos.y as i32) * i / steps) as u16;
                        let cp = crate::map::position::Position::new(cx, cy, player_pos.z);

                        if let Some(objects) = crate::map::MAP.get().unwrap().read().await.get_tile_objects(cp) {
                            for obj in objects {
                                let id = match obj {
                                    crate::map::TileObject::Ground(id) => *id,
                                    crate::map::TileObject::Item(id)   => *id,
                                };

                                if crate::map::items::item_properties(id).blocks_projectiles {
                                    blocked = true;
                                    break;
                                }
                            }
                        }

                        if blocked { break; }
                    }

                    if blocked {
                        crate::map::MAP.get().unwrap().write().await.add_item_to_tile(*from_pos, item_id);
                        let msg = self.send_status_message("Sorry, not possible.").await?;
                        self.enqueue(msg);
                        return Ok(());
                    }
                }

                // Make sure the destination is walkable before throwing an item.
                let dest_walkable = match crate::map::MAP.get().unwrap().read().await.get_tile_objects(*to_pos) {
                    Some(objects) if !objects.is_empty() => objects.iter().all(|obj| {
                        let id = match obj {
                            crate::map::TileObject::Ground(id) => *id,
                            crate::map::TileObject::Item(id) => *id,
                        };

                        crate::map::items::is_walkable(id)
                    }),
                    Some(_) => true,
                    None => false,
                };

                if !dest_walkable {
                    // Restore the item to wherever it came from.
                    match &from {
                        Location::Map(fp) => {
                            crate::map::MAP.get().unwrap().write().await.add_item_to_tile(*fp, item_id);
                        }

                        Location::Equipment(slot) => {
                            self.player.equipment.insert(*slot, item_id);
                            let slot_enum: crate::player::InventorySlot = match (*slot).try_into() {
                                Ok(s)  => s,
                                Err(_) => return Ok(()),
                            };

                            let msg = self.send_equipped_item(slot_enum, item_id).await?;
                            self.enqueue(msg);
                        }

                        Location::Container(local_id) => {
                            if let Some(c) = self.player.containers.iter_mut().find(|c| c.local_id == *local_id) {
                                c.items.push(item_id);
                                let c_clone = c.clone();
                                let msg = self.send_open_container(&c_clone).await?;
                                self.enqueue(msg);
                            }
                        }
                    }

                    let msg = self.send_status_message("Sorry, not possible.").await?;
                    self.enqueue(msg);
                    return Ok(());
                }

                // Check if the destination tile is occupied (i.e. a player is standing there).
                // For example, you cannot throw a trough to a tile a player is standing on.
                let occupied = self.player.position == *to_pos
                    || self.other_players.iter().any(|p| p.position == *to_pos);
                if occupied && !crate::map::items::is_walkable(item_id) {
                    match &from {
                        Location::Map(fp) => {
                            crate::map::MAP.get().unwrap().write().await.add_item_to_tile(*fp, item_id);
                        }

                        Location::Equipment(slot) => {
                            self.player.equipment.insert(*slot, item_id);
                            let slot_enum: crate::player::InventorySlot = match (*slot).try_into() {
                                Ok(s)  => s,
                                Err(_) => return Ok(()),
                            };
                            let msg = self.send_equipped_item(slot_enum, item_id).await?;
                            self.enqueue(msg);
                        }

                        Location::Container(local_id) => {
                            if let Some(c) = self.player.containers.iter_mut().find(|c| c.local_id == *local_id) {
                                c.items.push(item_id);
                                let c_clone = c.clone();
                                let msg = self.send_open_container(&c_clone).await?;
                                self.enqueue(msg);
                            }
                        }
                    }

                    let msg = self.send_status_message("Sorry, not possible.").await?;
                    self.enqueue(msg);
                    return Ok(());
                }

                crate::map::MAP.get().unwrap().write().await.add_item_to_tile(*to_pos, item_id);

                // If we just moved a container item on the map, transfer its persisted
                // contents from the old tile position to the new one so they survive the move.
                if crate::map::items::is_container(item_id) {
                    let contents_to_transfer: Option<Vec<u16>> = match &from {
                        Location::Map(from_pos) => {
                            crate::map::MAP.get().unwrap().write().await
                                .take_container_contents(*from_pos)
                        }

                        Location::Equipment(slot) => {
                            // Use the persisted contents for this equipment slot.
                            // The removal step above already saved the live window's
                            // contents before closing it, so this is always up to date.
                            self.player.equipment_container_contents.remove(slot)
                        }

                        Location::Container(local_id) => {
                            // A container nested inside another container being moved to the map.
                            // Its items are inline in the OpenContainer struct; retrieve them.
                            self.player.containers.iter().find(|c| c.local_id == *local_id).map(|c| c.items.clone())
                        }
                    };

                    if let Some(contents) = contents_to_transfer {
                        crate::map::MAP.get().unwrap().write().await.set_container_contents(*to_pos, contents);
                    }
                }

                // If a container sitting on the map was just thrown out of range, close its window.
                let player_pos = self.player.position;
                let mut to_close: Vec<u8> = Vec::new();
                for c in &self.player.containers {
                    if let crate::player::ContainerSource::Map(container_pos) = c.source {
                        let dx = (container_pos.x as i32 - player_pos.x as i32).abs();
                        let dy = (container_pos.y as i32 - player_pos.y as i32).abs();
                        if dx > 1 || dy > 1 {
                            to_close.push(c.local_id);
                        }
                    }
                }
                for local_id in to_close {
                    self.player.containers.retain(|c| c.local_id != local_id);
                    let msg = self.send_close_container(local_id).await?;
                    self.enqueue(msg);
                }

                let pos = self.player.position;
                let map = self.send_map(pos, 18, 14).await?;
                self.enqueue(map);
                self.world_sender.send(crate::world::message::PlayerToWorld::MapUpdate(pos))?;
            }

            // Pushing items to the equipment.
            Location::Equipment(slot) => {
                self.player.equipment.insert(*slot, item_id);
                let slot_enum: crate::player::InventorySlot = match (*slot).try_into() {
                    Ok(s)  => s,
                    Err(_) => return Ok(()),
                };
                let msg = self.send_equipped_item(slot_enum, item_id).await?;
                self.enqueue(msg);

                // If a container moved between equipment slots, update the open window's
                // source to the new slot and migrate the persistent contents key.
                if crate::map::items::is_container(item_id) {
                    if let Location::Equipment(old_slot) = &from {
                        for c in self.player.containers.iter_mut() {
                            if c.source == crate::player::ContainerSource::Equipment(*old_slot)
                                && c.container_id == item_id
                            {
                                c.source = crate::player::ContainerSource::Equipment(*slot);
                            }
                        }

                        if let Some(contents) = self.player.equipment_container_contents.remove(old_slot) {
                            self.player.equipment_container_contents.insert(*slot, contents);
                        }
                    }
                }

                let pos = self.player.position;
                let map = self.send_map(pos, 18, 14).await?;
                self.enqueue(map);
                self.world_sender.send(crate::world::message::PlayerToWorld::MapUpdate(pos))?;
            }

            // Pushing items to a container.
            Location::Container(local_id) => {
                // When the destination was rewritten from an equipment slot (drag-onto-bag),
                // find the open window by the container_id held in that equipment slot.
                // Otherwise find by local_id directly.
                let (target_container_id, target_source) = if matches!(&to, Location::Equipment(_)) {
                    let slot = match &to { Location::Equipment(s) => *s, _ => 0 };
                    let held_id = self.player.equipment.get(&slot).copied().unwrap_or(0);

                    // Find the source of the open window for this container.
                    let source = self.player.containers.iter()
                        .find(|c| c.container_id == held_id)
                        .map(|c| c.source.clone())
                        .unwrap_or(crate::player::ContainerSource::Equipment(slot));
                    (held_id, source)
                } else {
                    let c = self.player.containers.iter().find(|c| c.local_id == *local_id);
                    match c {
                        Some(c) => (c.container_id, c.source.clone()),
                        None    => return Ok(()),
                    }
                };

                // Add the item to every open window that shows the same container
                // (same container_id and same source location), so duplicate windows stay in sync.
                let mut updated_locals: Vec<u8> = Vec::new();
                for c in self.player.containers.iter_mut() {
                    if c.container_id == target_container_id && c.source == target_source {
                        c.items.push(item_id);
                        updated_locals.push(c.local_id);
                    }
                }

                // Send a refresh packet for each updated window.
                for lid in &updated_locals {
                    if let Some(c) = self.player.containers.iter().find(|c| c.local_id == *lid) {
                        let c_clone = c.clone();
                        let msg = self.send_open_container(&c_clone).await?;
                        self.enqueue(msg);
                    }
                }

                // Persist updated contents for all updated container windows.
                for lid in &updated_locals {
                    if let Some(c) = self.player.containers.iter().find(|c| c.local_id == *lid) {
                        match c.source {
                            crate::player::ContainerSource::Map(map_pos) => {
                                crate::map::MAP.get().unwrap().write().await
                                    .set_container_contents(map_pos, c.items.clone());
                                self.world_sender.send(crate::world::message::PlayerToWorld::ContainerUpdate(
                                    map_pos,
                                    target_container_id,
                                    c.items.clone(),
                                ))?;
                            }

                            crate::player::ContainerSource::Equipment(slot) => {
                                self.player.equipment_container_contents.insert(slot, c.items.clone());
                            }
                        }
                    }
                }

                // If no open window matched (i.e. bag is closed), persist the item directly
                // into the container's backing store so it isn't lost.
                if updated_locals.is_empty() {
                    match &target_source {
                        crate::player::ContainerSource::Equipment(slot) => {
                            self.player.equipment_container_contents
                                .entry(*slot)
                                .or_insert_with(Vec::new)
                                .push(item_id);
                        }

                        crate::player::ContainerSource::Map(map_pos) => {
                            let mut contents = crate::map::MAP.get().unwrap().read().await
                                .get_container_contents(*map_pos)
                                .cloned()
                                .unwrap_or_default();
                            contents.push(item_id);
                            crate::map::MAP.get().unwrap().write().await
                                .set_container_contents(*map_pos, contents.clone());
                            self.world_sender.send(crate::world::message::PlayerToWorld::ContainerUpdate(
                                *map_pos,
                                target_container_id,
                                contents,
                            ))?;
                        }
                    }
                }

                // Map redraw was already sent above for map->container moves.
                // For equipment->container or container->container, send it now.
                if !matches!(&from, Location::Map(_)) {
                    let pos = self.player.position;
                    let map = self.send_map(pos, 18, 14).await?;
                    self.enqueue(map);
                    self.world_sender.send(crate::world::message::PlayerToWorld::MapUpdate(pos))?;
                }
            }
        }

        Ok(())
    }

    /// Handles an item use request.
    ///
    /// For containers, tracks which window slot to open them in:
    /// - If used from inside an open container window (`sx = 0xFF`, `sy ≥ 0x14`), the
    ///   child container replaces that same window (folder-navigation behaviour).
    /// - Otherwise, the container opens in the next free window slot (0 or 1, max 2).
    ///   If both slots are occupied the oldest one (slot 0) is evicted.
    ///
    /// The container's source (equipment slot or map position) is recorded so it can be
    /// auto-closed when it moves out of range or is unequipped.
    ///
    /// Opening the same container (same container_id + source) a second time re-uses the
    /// same item list so the contents are consistent across duplicate windows.
    async fn recv_use_item<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        let _item_type = r.read_u8().await?;
        let use_pos    = r.read_position().await?;
        let item_id    = r.read_u16_le().await?;
        let _stack_pos = r.read_u8().await?;
        let _unknown   = r.read_u8().await?;

        if !crate::map::items::is_container(item_id) {
            let msg = self.send_status_message("Sorry, not possible.").await?;
            self.enqueue(msg);
            return Ok(());
        }

        // Determine where this container lives.
        let source = if use_pos.x == 0x00FF && use_pos.y < 0x0014 {
            // Opened from an equipment slot.
            crate::player::ContainerSource::Equipment(use_pos.y as u8)
        } else if use_pos.x == 0x00FF && use_pos.y >= 0x0014 {
            // Opened from inside another container window - source is the same as the
            // parent window's source.
            let parent_local_id = (use_pos.y - 0x0014) as u8;
            self.player.containers.iter()
                .find(|c| c.local_id == parent_local_id)
                .map(|c| c.source.clone())
                .unwrap_or(crate::player::ContainerSource::Equipment(0))
        } else {
            // Opened from the map.
            let mx = self.player.position.x as i32 + (use_pos.x as i32 - 8);
            let my = self.player.position.y as i32 + (use_pos.y as i32 - 6);
            crate::player::ContainerSource::Map(crate::map::position::Position::new(
                mx as u16, my as u16, self.player.position.z,
            ))
        };

        // Range check: map containers must be on the player's tile or adjacent (within 1 sqm).
        // Containers opened from equipment or from inside another container window are exempt.
        if let crate::player::ContainerSource::Map(container_pos) = &source {
            let dx = (container_pos.x as i32 - self.player.position.x as i32).abs();
            let dy = (container_pos.y as i32 - self.player.position.y as i32).abs();
            if dx > 1 || dy > 1 {
                let msg = self.send_status_message("Sorry, not possible.").await?;
                self.enqueue(msg);
                return Ok(());
            }
        }

        // If a window for this exact container (same id + source) is already open,
        // reuse that window's live item list (duplicate window).
        // Otherwise, for map containers, load the persisted contents from the map.
        // For equipment containers, there is no map persistence - use what the player
        // already has recorded (handled below via existing_items falling through to default).
        let existing_items: Option<Vec<u16>> = if let Some(open) = self.player.containers.iter()
            .find(|c| c.container_id == item_id && c.source == source)
        {
            // Already open in another window - share its live contents.
            Some(open.items.clone())
        } else if let crate::player::ContainerSource::Map(map_pos) = &source {
            // Map container not currently open - load from persistent map storage.
            crate::map::MAP.get().unwrap().read().await
                .get_container_contents(*map_pos)
                .cloned()
        } else if let crate::player::ContainerSource::Equipment(slot) = &source {
            // Equipment container - load from the player's own persistent storage.
            // This survives the bag being closed, moved to another slot, etc.
            self.player.equipment_container_contents.get(slot).cloned()
                .or_else(|| {
                    // Fall back to any open window with the same container_id
                    // (the bag may have moved slots since it was last opened).
                    self.player.containers.iter()
                        .find(|c| c.container_id == item_id)
                        .map(|c| c.items.clone())
                })
        } else {
            None
        };

        // Determine the local_id for the new window.
        let from_inside_container = use_pos.x == 0x00FF && use_pos.y >= 0x0014;
        let local_id = if from_inside_container {
            // Replace the parent window (folder-navigation).
            let parent_local_id = (use_pos.y - 0x0014) as u8;
            
            // Remove the parent window from tracking before replacing it.
            self.player.containers.retain(|c| c.local_id != parent_local_id);
            parent_local_id
        } else {
            // Open in the next free slot (0 or 1).
            let used: std::collections::HashSet<u8> =
                self.player.containers.iter().map(|c| c.local_id).collect();
            if let Some(free) = (0u8..2).find(|id| !used.contains(id)) {
                free
            } else {
                // Both slots occupied - evict slot 0.
                self.player.containers.retain(|c| c.local_id != 0);
                0
            }
        };

        let items = existing_items.unwrap_or_default();

        self.player.containers.push(crate::player::OpenContainer {
            container_id: item_id,
            items:        items.clone(),
            local_id,
            source,
        });

        let c = self.player.containers.last().unwrap().clone();
        let msg = self.send_open_container(&c).await?;
        self.enqueue(msg);

        Ok(())
    }

    /// Handles a container close request - sent when the player closes a container window.
    ///
    /// Removes the matching window from `player.containers` so its slot is free for re-use.
    async fn recv_close_container<R: AsyncRead + Unpin>(&mut self, r: &mut R) -> Result<()> {
        let local_id = r.read_u8().await?;
        self.player.containers.retain(|c| c.local_id != local_id);
        let msg = self.send_close_container(local_id).await?;
        self.enqueue(msg);
        Ok(())
    }
}
