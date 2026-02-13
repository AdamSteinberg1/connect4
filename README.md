# Connect 4

A terminal-based Connect 4 game implementation in Rust using the Ratatui TUI framework.

## Overview

Connect 4 is a two-player connection game where players alternate dropping colored discs into a vertical grid. This implementation provides an interactive terminal user interface with full game logic.

## Features

- Interactive terminal UI using [Ratatui](https://ratatui.rs/)
- Color-coded players (Red and Yellow)
- Keyboard controls

## Quick Start

```bash
# Requires Rust: https://www.rust-lang.org/tools/install
git clone https://github.com/AdamSteinberg1/connect4.git
cd connect4
cargo run --release
```

## Controls

| Key | Action |
|-----|--------|
| `←` / `→` | Move selection left/right |
| `Enter` | Place disc in selected column |
| `Q` / `Esc` | Quit game |
| `1-7` | Select column (quick select) |

## Gameplay

Players alternate turns between Red and Yellow. The two players are intended to pass the kayboard between each other. The first player to connect four discs horizontally, vertically, or diagonally wins. If the board fills without a winner, the game is a draw.

## Project Structure

```
connect4/
├── src/
│   ├── main.rs      # Main application logic and TUI rendering
│   └── board.rs     # Game board state and win detection logic
├── Cargo.toml       # Project manifest with dependencies
├── Cargo.lock       # Dependency lock file
└── .gitignore       # Git ignore rules
```

## Dependencies

- **ratatui** - Terminal UI framework for rendering
- **crossterm** - Cross-platform terminal manipulation and event handling
- **anyhow** - Simplified error handling
- **itertools** - Extended iterator functionality
