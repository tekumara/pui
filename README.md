# Pui

A live, interactive terminal dashboard for tasks (ie: background processes) managed by [Pueue](https://github.com/nukesor/pueue). Watch tasks run, stream their logs, filter and sort the task list, and run custom commands — all without leaving your terminal.

![demo](docs/demo.gif)

## Features

- **Real-time monitoring** — task statuses refresh automatically
- **Task management** — start, pause, kill, restart, and remove tasks
- **Streaming log viewer** — real-time log streaming with autoscroll
- **Task filtering** — interactive text filter across all columns (id, status, command, path)
- **Column sorting** — sort tasks by id, status, command, or path
- **Multi-select** — toggle selection on multiple tasks then apply actions to all selected tasks at once
- **Custom commands** — define key-bound commands that run in the selected task's working directory (e.g., launch lazygit, an editor, or shell)

## Installation

```bash
cargo build --release
```

## Usage

Make sure the `pueued` daemon is running before starting `pui`.

Use pueue to create new tasks, and then pui to manage them.

To start pui:

```bash
./target/release/pui
```
