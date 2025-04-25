<div align="center">

# TACX - Ripple Transaction Monitor

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Ripple](https://img.shields.io/badge/Blockchain-Ripple-blue.svg)](https://xrpl.org/)

**A high-performance, secure, real-time monitoring tool for Ripple blockchain transactions**

</div>

## Overview

TACX is a professional-grade Ripple blockchain transaction monitoring tool built with Rust. It provides real-time visibility into the Ripple network through a secure WebSocket connection, displaying transaction data in an intuitive terminal-based user interface.

## Features

- **Real-time Transaction Monitoring**: Stream and display live Ripple blockchain transactions
- **Multi-tab Interface**: Dedicated views for transactions, offers, and network statistics
- **Transaction Type Filtering**: Focus on specific transaction types (Payments, Offers, etc.)
- **Secure Connection Handling**: TLS encryption with certificate validation
- **Robust Error Handling**: Comprehensive error management with secure logging
- **Rate Limiting Protection**: Built-in safeguards against connection flooding
- **Automatic Reconnection**: Exponential backoff strategy for network resilience
- **Data Sanitization**: Input validation and message sanitization
- **Performance Optimized**: Efficient batch processing of transactions

## Technology Stack

| Component | Technology |
|-----------|------------|
| **Language** | ![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white) |
| **UI Framework** | ![Ratatui](https://img.shields.io/badge/Ratatui-FF5F1F?style=for-the-badge&logo=rust&logoColor=white) |
| **Async Runtime** | ![Tokio](https://img.shields.io/badge/Tokio-7E57C2?style=for-the-badge&logo=rust&logoColor=white) |
| **WebSocket Client** | ![Tungstenite](https://img.shields.io/badge/Tungstenite-2C8EBB?style=for-the-badge&logo=rust&logoColor=white) |
| **Serialization** | ![Serde](https://img.shields.io/badge/Serde-00ADD8?style=for-the-badge&logo=rust&logoColor=white) |
| **Logging** | ![Tracing](https://img.shields.io/badge/Tracing-4B32C3?style=for-the-badge&logo=rust&logoColor=white) |
| **Error Handling** | ![Anyhow](https://img.shields.io/badge/Anyhow-F05033?style=for-the-badge&logo=rust&logoColor=white) |

## Prerequisites

- Rust 1.70 or higher
- Cargo package manager
- Terminal with Unicode support

## Installation

```bash
# Clone the repository
git clone https://github.com/aminnizamdev/tacx.git
cd tacx

# Build the project
cargo build --release

# Run the application
cargo run --release
```

## Usage

```bash
# Connect to the default Ripple WebSocket server
cargo run --release

# Connect to a specific Ripple WebSocket server
cargo run --release -- --server wss://s2.ripple.com

# Configure transaction history size
cargo run --release -- --history-size 200

# Set UI update interval (milliseconds)
cargo run --release -- --update-interval 500
```

### Command Line Arguments

| Argument | Short | Description | Default |
|----------|-------|-------------|--------|
| `--server` | `-s` | WebSocket server URL | `wss://s1.ripple.com` |
| `--history-size` | `-h` | Number of transactions to keep in history | `100` |
| `--update-interval` | `-u` | UI refresh rate in milliseconds | `250` |

## Security Features

TACX implements multiple layers of security to ensure safe and reliable operation:

- **TLS Encryption**: Secure WebSocket connections with certificate validation
- **Input Validation**: Rigorous validation of all incoming data
- **Rate Limiting**: Protection against connection flooding and DoS attempts
- **Message Sanitization**: Prevents injection attacks and malformed data
- **Secure Error Handling**: Redaction of sensitive information in logs
- **Connection Tracking**: Monitoring of connection attempts with backoff enforcement

## Architecture

TACX follows a modular architecture with clear separation of concerns:

```
src/
├── client.rs     # WebSocket client implementation
├── formatter.rs  # Data formatting utilities
├── main.rs       # Application entry point
├── models.rs     # Data structures and state management
├── security.rs   # Security features and validation
└── ui.rs         # Terminal user interface
```

## User Interface

The terminal-based UI provides multiple views:

- **Transactions Tab**: Real-time stream of all transactions
- **Offers Tab**: Market orders and trading activity
- **Statistics Tab**: Network activity metrics and transaction type distribution

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contact

Amin Nizam - [@aminnizamdev](https://github.com/aminnizamdev)

Project Link: [https://github.com/aminnizamdev/tacx](https://github.com/aminnizamdev/tacx)

---

<div align="center">

**TACX** - Bringing transparency to the Ripple blockchain

</div>