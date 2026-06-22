# Albe's Journey - A game server for Tibia version 1.03
Tibia is a MMORPG developed by CipSoft GmbH. The game officially launched on Jan 07 1997. Just a couple of days later, on Jan 10 1997, the very first player in Tibia entered the game using the character name **Albe** - hence the name of this project; **Albe's Journey**.

On Feb 08 1997, version alpha 1.03 of the game was released. It is the earliest preserved version of the game that can be [found on the Internet Archive](https://web.archive.org/web/19970513122646/http://www-wi.uni-regensburg.de/~vos19618/tibia/e_download.html). And because of that, we have managed to reverse engineer the game client's packet structure and built a functioning server that can communicate with it.

This repository is a collection of tools and information neccessary in order to play that version of the game. Here you will not only find the game server, but also the game client, sprite images, item data, and a map converter from OTBM format (most commonly used within the Open Tibia community).

**Scroll down to see all information you might need, including the changelog of the server.**

## Credits
- **Snyder** - In 2001 he released his [Tibia server emulator](https://web.archive.org/web/20011220184436/http://members.fortunecity.com/snyder8). It made it possible to play Tibia on a private server for the first time in history. His reverse engineering of the game later helped develop the first Open Tibia server. His server emulator has since inspired many people, and one can still learn from his efforts until this day.
- **Jopirop** - In 2010 he released [TOS Server](https://sourceforge.net/projects/tosserver/), a Tibia Old School server targeting the Tibia 6.4 protocol. It was an improvement of Snyder's server emulator and included even more information about the packet structure in the earlier versions of Tibia.
- **rsribeiro** - In 2022 he released [legbone](https://github.com/rsribeiro/legbone), a Tibia game server targeting protocol 3.0 onwards, with limited support for Tibia 1.03. It was a modern implementation inspired by Jopirop's release, as well as [Open Tibia version 0.1.0](https://sourceforge.net/projects/opentibia/), and also included his own reverse engineering.
- **jo3bingham** - He released [a tool](https://www.reddit.com/r/TibiaMMO/comments/hwk2cx/tibia_103_sprites/) that extracted the sprite images used in Tibia 1.03.
- **Inconcessus** - He made [a converter](https://github.com/Inconcessus/OTBM2JSON) for the Open Tibia map format OTBM to JSON - which this server uses.
- **Karr Chaos** - He's been documenting the earliest versions of the game [on his website](https://nightmareknights.com/historyframes/framesalpha12.html) and has preserved a lot of useful information about the game's features, which helped during the reverse engineering process.

This project would not have been possible without the efforts of those mentioned above. So I would like to personally thank them for their tremendous efforts in bringing back life to the earliest versions of Tibia.

## Prerequisites
Listed below are the things you need to setup on your computer before you're able to play on this server.

### Windows 95
In order to play Tibia 1.03, you will need a supported operating system such as Windows 3.1 or Windows 95. The easiest way to get that is to use Felix Rieseberg's [windows95 app](https://github.com/felixrieseberg/windows95) which works on Windows, Linux and macOS. Within minutes you can download Windows 95 and run it as an app. I've personally used it on Linux Mint and it works flawlessly. It also sets up a shared folder between Windows 95 and your main operating system, which makes it easy to move the game client files over.

You can also use something like [winevdm](https://github.com/otya128/winevdm) to run the Tibia 1.03 game client on modern Windows. I've not personally tested it, but I've heard good things about it.

### Rust
The game server is written in the Rust programming language. In order to run it and modify to your liking, you will need to [install Rust](https://rust-lang.org/tools/install/) on your computer. Once you have done that, it's a straightforward process to start the game server. See instructions below.

### Remere's Map Editor
In case you want to change the map on the server, I recommend using [Remere's Map Editor](https://github.com/opentibiabr/remeres-map-editor) (also known as "RME"). This server's map was built using it, with [Tibia 8.60](https://mega.nz/#!WfA1kKwT!oH9hLUQEafAtWtzJJrd3gnn2TN383qpqQfrp7qqLbC0) graphics. 

#### Node.js
If you used RME to build the map file in OTBM format ("**map.otbm**"), you will need to convert the map to JSON ("**map.json**") because that's the format this server uses. This repository includes a Node.js script which converts the map for you - which means you will have to install [Node.js](https://nodejs.org/en) if you plan on using the map converter.
**Note:** The script uses the `grep` command after converting the map, in order to clean up the formatting. That means it will currently only work on Linux, unless you modify the map conversion script.

## Running the server
Once you have installed the neccessary softwares (*as mentioned above*) you can get the server up and running within a few seconds.
If you haven't already, start by downloading this repository so that you get all the neccessary files. You can download the repository by clickin the green button that says "**Code**" and select "**Download ZIP**". Extract it on your desktop so that you have the `tibia-main` directory.

Then launch a new terminal/command prompt window inside the `tibia-main/server` directory and run the following commands:
```
cargo check
cargo run
```

The game server is now running and is ready to accept connections from players.

Because this game server is made in Rust using the `rustdoc` syntax, you can also run the following command to automatically generate a documentation page for you.

```
cargo doc
```

## Connecting to the server
Now you will need to launch Windows 95 and move the game client folder (`tibia-main/client`) over there. Then launch the game client and click on `File - Preferences`. In the input field labeled `Tibia-Server Address`, enter the IP address of the computer that's running the server. If you don't know your local IP address, you can use the following commands in a terminal/command prompt on your machine:

```
Windows -> ipconfig /all
Linux   -> ip a
```
Your local IP address looks something like `192.168.1.xxx`. Leave the port number in the client as default (`7171`) and click `Save`. Now you're ready to login. You can either login using `New Game` and setup your character the way you want, or use `Journey Onward` and enter any character name and password.

## In-game features
Tibia 1.03 is very limited in terms of features, but there were some. You can find information about it on the [old Tibia website from 1997](https://web.archive.org/web/19970513130635/http://www-wi.uni-regensburg.de/~vos19618/tibia/e_anleitung.html). Spend a minute to familiarize yourself with the game client's built-in menus. And below are some features you can use in the game:
```
// Movements and actions
Right-click                           -> Use an object
Left-click                            -> Begin auto-walking towards a destination
Left-and-right click on something     -> Look at an object

// Chat commands
#W <message>                          -> Whisper a message to nearby players (range: 2 sqm)
#Y <message>                          -> Yell a message to nearby players (range: 32 sqm)
#B <message>                          -> Broadcast a message to all players online.
*<name>* <message>                    -> Send a private message to someone
@<name>@ <message>                    -> Send a private message to someone
```

## Changelog
Listed below you will find the status of the game server.
- ✅ Packet structure.
- ✅ Ability to login, create a character and logout.
- ✅ Change outfit (`Info - Change Data`).
- ✅ See a list of players online (`Info - Userlist`).
- ✅ Access information about a player (`Info - Userlist - {player} - Info`).
- ✅ Send comments (`Info - Comments`).
- ✅ Render the map, objects, players, outfits (colors, and sprites based on direction),
- ✅ Ability to walk around, both using arrow keys and left-click.
- ✅ Looking at players, ground and objects.
- ✅ Item properties and attributes (moveable, block projectiles, throw range, container).
- ✅ Equipment (player inventory).
- ✅ Chatting, as well as different chat modes (`#W, #Y, #B, private messages`).
- 🛠️ Containers (equipment, and on the map).
- 🛠️ Moving objects on the map, as well as to/from containers and equipment.
- ❌ Using objects (using levers, baking bread).
- ❌ Persistent storage of players (and map?) in a database (SQLite?).
___
Last updated: 2026-06-22
