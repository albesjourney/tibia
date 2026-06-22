//! Network layer - packet definitions and connection handling.
//!
//! See [`connection`] for the login handshake and per-connection run loop,
//! and [`packet`] for the full set of incoming and outgoing packet types.

pub mod connection;
pub mod packet;
