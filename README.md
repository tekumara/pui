# Pui - Pueue TUI

A terminal user interface for the [Pueue](https://github.com/nukesor/pueue) task runner, built with Rust and Ratatui.

## Features

- **Real-time Monitoring**: Periodic refresh of task statuses.
- **Task Management**: Start, pause, kill, and remove tasks directly from the TUI.
- **Task Details**: View detailed information about selected tasks.
- **Navigation**: Simple vim-like or arrow key navigation.

## Keybindings

- `j` / `Down`: Move selection down
- `k` / `Up`: Move selection up
- `s`: Start selected task
- `p`: Pause selected task
- `x`: Kill selected task
- `Backspace`: Remove selected task
- `q`: Quit

## Installation

```bash
cargo build --release
```

## Usage

Make sure the `pueued` daemon is running before starting `pui`.

```bash
./target/release/pui
```
