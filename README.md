# Connect 4

Have you ever wanted to play Connect $ from your terminal? Now you can!

## Features

- Interactive terminal UI built using [Ratatui](https://ratatui.rs/)
- Local and Online Multiplayer
- Keyboard controls

## Quick Start

```bash
# Requires Rust: https://www.rust-lang.org/tools/install
git clone https://github.com/AdamSteinberg1/connect4.git
cd connect4
cargo run -p connect4-client --release
```

## Controls

| Key | Action |
|-----|--------|
| `←` / `→` | Move selection left/right |
| `Enter` | Place disc in selected column |
| `Q` / `Esc` | Quit game |
| `1-7` | Select column |

## Gameplay

Players alternate turns between Red and Yellow. For local multiplayer, the two players are intended to pass the keyboard between each other. The first player to connect four discs horizontally, vertically, or diagonally wins.

## Online Multiplayer

From the main menu, you have the choice to create or join an online game. If you create an online game, then you are the host and will be assigned a join code that you can share with your opponent. If you choose to join an online game, you will be prompted to enter the join code that the host shared with you.

## Project Structure
The project is a Cargo workspace containing three crates. Two binary crates and one library crate.
1. client - binary crate that each player runs on their machine.
2. server - binary crate that runs on the server and all clients connect to.
3 shared - library crate containing common data structures that both the client and server use.


