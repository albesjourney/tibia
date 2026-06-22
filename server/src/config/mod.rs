//! Server configuration, loaded from `server.toml` at startup.
//!
//! All configurable settings live in a single TOML file. The parsed result is stored in a
//! global [`OnceLock`] so any module can access it via [`CONFIG`] after [`init`] has been
//! called.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::{net::Ipv4Addr, path::Path, sync::OnceLock};

/// Global configuration instance, populated once by [`init`] at startup.
pub static CONFIG: OnceLock<Config> = OnceLock::new();

/// Top-level configuration, corresponding to the sections in `server.toml`.
#[derive(Deserialize, Debug)]
pub struct Config {
    pub server: Server,
    pub world: World,
}

/// Network and gameplay settings for the server process.
#[derive(Deserialize, Debug)]
pub struct Server {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub debug_messages: bool,
    pub status_message: String,
    pub message_of_the_day: String,
}

/// World settings, currently just the path to the map file.
#[derive(Deserialize, Debug)]
pub struct World {
    pub map_file: String,
}

/// Reads and parses the TOML config file at `path`, storing the result in [`CONFIG`].
///
/// Returns an error if the file does not exist, cannot be read, fails to parse,
/// or if `init` has already been called.
pub fn init(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("Config file {:?} not found.", path));
    }

    let contents = std::fs::read_to_string(path)?;
    let config = toml::from_str::<Config>(&contents)?;
    CONFIG.set(config).map_err(|_| anyhow!("Config has already been initialized."))?;

    Ok(())
}

/// Returns `true` if `debug_messages = true` is set in `server.toml`.
///
/// Defaults to `false` if the config has not been loaded yet.
pub fn is_debug() -> bool {
    CONFIG.get().map(|c| c.server.debug_messages).unwrap_or(false)
}

/// Prints a timestamped log message to stdout if `debug_messages = true` in `server.toml`.
///
/// Accepts the same format string syntax as [`println!`]. Does nothing if debug logging is disabled.
///
/// # Examples
/// ```
/// debug_log!("Player {:?} connected to the game.", player_id);
/// ```
#[macro_export]
macro_rules! debug_log {
    () => {
        if $crate::config::is_debug() {
            println!(
                "[{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                ""
            );
        }
    };
    ($($arg:tt)+) => {
        if $crate::config::is_debug() {
            println!(
                "[{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                format!($($arg)*)
            );
        }
    };
}
