use anyhow::Result;
use config::CONFIG;
use std::path::Path;
use tokio::net::TcpListener;
use tokio_stream::{wrappers::TcpListenerStream, StreamExt};
use world::World;

mod chat;
mod config;
mod constants;
mod io;
mod map;
mod network;
mod player;
mod world;

#[tokio::main]
async fn main() -> Result<()> {
    // Load "server.toml" and initialize the map data.
    config::init(Path::new("server.toml"))?;
    map::init(&CONFIG.get().unwrap().world.map_file)?;
    let config = CONFIG.get().unwrap();

    // Bind the TCP listener and start accepting connections.
    let addr = std::net::SocketAddrV4::new(config.server.ip, config.server.port);
    let listener = TcpListener::bind(addr).await?;
    println!("[{}] Tibia game server is running on: {addr}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));

    // Initialize the game world and grab a sender handle for dispatching events.
    let world = World::new();
    World::start(&world);
    let world_sender = world.read().await.sender();

    // Accept incoming TCP connections and hand each off to its own task.
    let mut incoming = TcpListenerStream::new(listener);

    while let Some(stream) = incoming.next().await {
        match stream {
            Ok(stream) => {
                let sender = world_sender.clone();

                // Spawn a task per connection so the listener stays responsive.
                tokio::spawn(async move {
                    match network::connection::Connection::handle_login(stream, sender).await {
                        // Login succeeded - run the connection's main loop.
                        Ok(Some(mut conn)) => {
                            debug_log!("::main -> Login successfully authenticated.");
                            match conn.run().await {
                                Ok(_) => debug_log!("::main -> Connection closed normally."),
                                Err(e) => debug_log!("::main -> Connection error: {:?}", e),
                            }
                        }

                        // Client disconnected or aborted before login completed.
                        Ok(None) => debug_log!("::main -> Login aborted or client disconnected during handshake."),

                        // Handshake failed (bad credentials, protocol error, etc.).
                        Err(e) => debug_log!("::main -> Login error during handshake: {:?}", e),
                    }
                });
            }
            Err(e) => debug_log!("::main -> Failed to accept incoming TCP connection: {:?}", e),
        }
    }

    Ok(())
}
