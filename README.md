# Ripple Transaction Monitor

A real-time monitoring tool for Ripple blockchain transactions built with Rust. This application connects to the Ripple WebSocket API and displays transaction data in a terminal-based UI.

## Features

- Real-time monitoring of Ripple blockchain transactions
- Terminal-based user interface using Ratatui
- Automatic reconnection with exponential backoff
- Support for different transaction types (Payment, OfferCreate, etc.)
- Configurable history size and update interval

## Usage

```bash
# Run with default settings
cargo run

# Specify a custom WebSocket server
cargo run -- --server wss://s1.ripple.com

# Configure history size (number of transactions to keep)
cargo run -- --history-size 200

# Set UI update interval in milliseconds
cargo run -- --update-interval 500
```

## Command Line Arguments

- `--server` or `-s`: WebSocket server URL (default: wss://s1.ripple.com)
- `--history-size` or `-h`: Number of transactions to keep in history (default: 100)
- `--update-interval` or `-u`: UI update interval in milliseconds (default: 250)

## Building

```bash
cargo build --release
```

## License

MIT