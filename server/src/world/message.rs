use crate::chat::ChatType;
use crate::map::position::Position;
use crate::player::Player;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone, Debug)]
pub enum PlayerToWorld {
    Login(Player, UnboundedSender<WorldToPlayer>),
    Logout(Player),
    UpdateInfo(Player),
    UpdatePosition(Player),
    Chat(Position, ChatType, Vec<u8>, String),
}

#[derive(Clone, Debug)]
pub enum WorldToPlayer {
    PlayerList(Vec<Player>),
    Chat(Position, ChatType, Vec<u8>, String),
}
