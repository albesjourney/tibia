# Albe's Journey - A game server for Tibia version 1.03

Tibia is a MMORPG developed by CipSoft GmbH. The game officially launched on Jan 07 1997. Just a couple of days later, on Jan 10 1997, the very first player in Tibia entered the game using the character name **Albe** - hence the name of this project; **Albe's Journey**.

On Feb 08 1997, version alpha 1.03 of the game was released. It is the earliest preserved version of the game that can be [found on the Internet Archive](https://web.archive.org/web/19970513122646/http://www-wi.uni-regensburg.de/~vos19618/tibia/e_download.html). And because of that, we have managed to reverse engineer the game client's packet structure and built a functioning server that can communicate with it.

This repository is a collection of tools and information neccessary in order to play that version of the game. Here you will find the game server, game client, sprite images, item data, and a map converter from OTBM format.

---

## Quick Start

### 1. Download the server

Download the compiled server for your operating system from the [release section](https://github.com/albesjourney/tibia/releases/):

- **Linux:** `albesjourney-linux-x86_64.zip`
- **Windows:** `albesjourney-windows-x86_64.zip`

Extract the files and run the executable (`albesjourney`). The server is now running and ready to accept connections.

### 2. Set up Windows 95

Tibia 1.03 requires Windows 3.1 or Windows 95. The easiest way to get that is Felix Rieseberg's [windows95 app](https://github.com/felixrieseberg/windows95), which works on Windows, Linux and macOS. It also sets up a shared folder between Windows 95 and your host OS, making it easy to transfer files.

Alternatively, [winevdm](https://github.com/otya128/winevdm) can run the client on modern Windows without a VM.

### 3. Connect and play

Move the `client` folder from this repository into Windows 95. Launch the game client and go to `File - Preferences`. In the `Tibia-Server Address` field, enter the local IP address of the machine running the server:

```
Windows -> ipconfig /all
Linux   -> ip a
```

Your local IP address will look something like `192.168.1.xxx`. Leave the port as default (`7171`) and click `Save`.

You can now log in using `New Game` to create a character, or `Journey Onward` to enter any name and password.

---

## In-game features

You can find general information about the game on the [old Tibia website from 1997](https://web.archive.org/web/19970513130635/http://www-wi.uni-regensburg.de/~vos19618/tibia/e_anleitung.html). Here are the controls and chat commands:

```
// Movement and actions
Right-click                           -> Use an object
Left-click                            -> Begin auto-walking towards a destination
Left-and-right click at the same time -> Look at a tile/object/player

// Chat commands
#W <message>                          -> Whisper (range: 2 sqm)
#Y <message>                          -> Yell (range: 32 sqm)
#B <message>                          -> Broadcast to all players online
*<name>* <message>                    -> Send a private message
@<name>@ <message>                    -> Send a private message (alternate syntax)
```

---

## Changelog

- ✅ Packet structure.
- ✅ Ability to login, create a character and logout.
- ✅ Change outfit (`Info - Change Data`).
- ✅ See a list of players online (`Info - Userlist`).
- ✅ Access information about a player (`Info - Userlist - {player} - Info`).
- ✅ Send comments (`Info - Comments`).
- ✅ Render the map, objects, players, outfits (colors, and sprites based on direction).
- ✅ Ability to walk around, both using arrow keys and left-click.
- ✅ Looking at tiles, objects and players.
- ✅ Item properties and attributes (moveable, block projectiles, throw range, container).
- ✅ Equipment (player inventory).
- ✅ Chatting, as well as different chat modes (`#W, #Y, #B, private messages`).
- 🛠️ Containers (equipment, and on the map - e.g. barrels, chests).
- 🛠️ Moving objects on the map, as well as to/from containers and equipment.
- ❌ Using objects (using levers, baking bread).
- ❌ Persistent storage of players (and map?) in a database (SQLite?).

Last updated: 2026-06-22

---

## Modifying the server

This section is for those who want to edit the server code, recompile it, or modify the map.

### Running from source (Rust)

The server is written in Rust. Install it from [rust-lang.org](https://rust-lang.org/tools/install/), then open a terminal in the `server` directory and run:

```
cargo check
cargo run
```

Because the server uses `rustdoc` syntax, you can also run `cargo doc` to automatically generate a documentation page for your convenience.

### Editing the map

The map (`map.otbm`) is in OTBM format and can be edited with [Remere's Map Editor](https://github.com/hampusborgos/rme) using [Tibia 8.60](https://otservlist.org/download) assets. You will find the `map.otbm` file inside the `map-converter` directory of this repository. Please make sure to only add items that existed in Tibia 1.03. You will find the full item list inside `server/src/map/items.rs`.

After editing, you need to convert the map from OTBM to JSON format. This repository includes a Node.js conversion script - install [Node.js](https://nodejs.org/en) and run it from the directory where `map.otbm` is located. Note: the script uses `grep` and currently only works on Linux.

```
node convert.mjs
```

It will output the map (`map.json`) which you can then place in your server directory. Please note that the item `trough of water` requires a manual edit in the JSON file. Place regular troughs on the map first, then change the item ID from `1775` to `17751` for any that should appear filled with water.

---

## Credits

This project would not have been possible without the following people and their contributions to the Tibian community over the years.

- **Snyder** - In 2001 he released his [Tibia server emulator](https://web.archive.org/web/20011220184436/http://members.fortunecity.com/snyder8), making it possible to play Tibia on a private server for the first time in history. His reverse engineering later helped develop the first Open Tibia server - and many other servers (including this one).
- **Jopirop** - In 2010 he released [TOS Server](https://sourceforge.net/projects/tosserver/), targeting the Tibia 6.4 protocol - an improvement of Snyder's emulator with more detail on the packet structure of earlier versions.
- **rsribeiro** - In 2022 he released [legbone](https://github.com/rsribeiro/legbone), a modern Tibia server implementation targeting Tibia 3.0, with limited support for 1.03, inspired by Jopirop's [TOS Server](https://sourceforge.net/projects/tosserver/) and [Open Tibia 0.1.0](https://sourceforge.net/projects/opentibia/).
- **jo3bingham** - He made [a tool](https://www.reddit.com/r/TibiaMMO/comments/hwk2cx/tibia_103_sprites/) that extracted the sprite images used in Tibia 1.03.
- **Inconcessus** - He made [a tool](https://github.com/Inconcessus/OTBM2JSON) that converts OTBM map format to JSON.
- **Karr Chaos** - He's been documenting the earliest versions of Tibia [on his website](https://nightmareknights.com/historyframes/framesalpha12.html), preserving valuable information about game which helped during the development of this project.
