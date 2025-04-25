use std::time::Duration;
use std::process::Command;
use std::thread;

use anyhow::Result;

mod client;
mod formatter;
mod models;
mod security;
mod ui;

use client::RippleClient;
use models::AppState;
use ui::UI;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Parse command line arguments
    let args = std::env::args().collect::<Vec<String>>();
    let server_url = args.iter().position(|arg| arg == "--server" || arg == "-s")
        .and_then(|pos| args.get(pos + 1))
        .unwrap_or(&String::from("wss://s1.ripple.com"))
        .clone();
    
    let history_size = args.iter().position(|arg| arg == "--history-size" || arg == "-h")
        .and_then(|pos| args.get(pos + 1))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);
    
    let update_interval = args.iter().position(|arg| arg == "--update-interval" || arg == "-u")
        .and_then(|pos| args.get(pos + 1))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(250);
    
    // Initialize application state
    let app_state = AppState::new(history_size);
    
    // Create client
    let client = RippleClient::new(server_url);
    
    // Share state with client thread
    let client_state = app_state.clone();
    
    // Spawn a task to connect to the Ripple WebSocket server
    tokio::spawn(async move {
        loop {
            if let Err(e) = client.connect(client_state.clone()).await {
                tracing::error!("Connection error: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    });
    
    // Spawn DeepSeek status terminal
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd")
            .args(["/C", "start", "cmd", "/K", "cargo run --bin deepseek_status"])
            .spawn();
        let _ = Command::new("cmd")
            .args(["/C", "start", "cmd", "/K", "cargo run --bin wallet_details"])
            .spawn();
        let _ = Command::new("cmd")
            .args(["/C", "start", "cmd", "/K", "cargo run --bin wallet_deepseek_analyzer"])
            .spawn();
    }
    // Initialize UI
    let mut ui = UI::new(app_state.clone(), Duration::from_millis(update_interval))?;
    // Start the UI
    ui.run().await?;
    
    // Periodically export recent transactions for DeepSeek
    {
        let app_state = app_state.clone();
        thread::spawn(move || {
            loop {
                {
                    let state = app_state.lock().unwrap();
                    let _ = state.export_recent_transactions_to_json(100, "recent_transactions.json");
                }
                std::thread::sleep(std::time::Duration::from_secs(10));
            }
        });
    }
    
    Ok(())
}