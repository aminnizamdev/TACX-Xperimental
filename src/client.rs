use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use backoff::ExponentialBackoffBuilder;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, warn};
use url::Url;

use crate::models::{AppState, ClientMessage, Transaction};

pub struct RippleClient {
    server_url: String,
}

impl RippleClient {
    pub fn new(server_url: String) -> Self {
        Self { server_url }
    }

    pub async fn connect(&self, app_state: Arc<Mutex<AppState>>) -> Result<()> {
        let url = Url::parse(&self.server_url)?;
        debug!("Connecting to {}", url);

        // Configure backoff strategy
        let _backoff = ExponentialBackoffBuilder::new()
            .with_initial_interval(Duration::from_millis(500))
            .with_max_interval(Duration::from_secs(30))
            .with_multiplier(2.0)
            .with_max_elapsed_time(Some(Duration::from_secs(300)))
            .build();

        // Connect to WebSocket with error handling
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                debug!("Connected to Ripple WebSocket server");
                
                // Update connection status
                {
                    let mut state = app_state.lock().unwrap();
                    state.connected = true;
                }
                
                // Subscribe to transactions
                self.handle_connection(ws_stream, app_state).await?
            },
            Err(e) => {
                warn!("Failed to connect to WebSocket server: {}", e);
                return Err(anyhow::anyhow!("WebSocket connection failed: {}", e));
            }
        }

        Ok(())
    }

    async fn handle_connection(
        &self,
        mut ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        app_state: Arc<Mutex<AppState>>,
    ) -> Result<()> {
        // Subscribe to transactions with error handling
        let subscribe_msg = serde_json::to_string(&ClientMessage::subscribe())?;
        if let Err(e) = ws_stream.send(Message::Text(subscribe_msg)).await {
            error!("Failed to send subscription message: {}", e);
            return Err(anyhow::anyhow!("Failed to subscribe: {}", e));
        }
        debug!("Subscribed to transactions");

        // Process incoming messages
        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Parse the message as a JSON value first with error handling
                    match serde_json::from_str::<serde_json::Value>(&text) {
                        Ok(value) => {
                            // Check if this is a transaction message
                            if let Some(tx_obj) = value.get("transaction") {
                            // Extract transaction data
                            if let Some(tx_type) = tx_obj.get("TransactionType").and_then(|v| v.as_str()) {
                                let hash = tx_obj.get("hash")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                
                                let account = tx_obj.get("Account")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                
                                // Extract amount for Payment transactions
                                let amount = if tx_type == "Payment" {
                                    tx_obj.get("Amount")
                                        .and_then(|v| {
                                            if let Some(s) = v.as_str() {
                                                Some(s.to_string())
                                            } else if let Some(n) = v.as_u64() {
                                                Some(n.to_string())
                                            } else {
                                                None
                                            }
                                        })
                                } else {
                                    None
                                };
                                
                                // Extract offer data for OfferCreate transactions
                                let (taker_gets, taker_pays) = if tx_type == "OfferCreate" {
                                    (
                                        tx_obj.get("TakerGets").and_then(|v| {
                                            if let Some(s) = v.as_str() {
                                                Some(s.to_string())
                                            } else if let Some(n) = v.as_u64() {
                                                Some(n.to_string())
                                            } else {
                                                None
                                            }
                                        }),
                                        tx_obj.get("TakerPays").and_then(|v| {
                                            if let Some(s) = v.as_str() {
                                                Some(s.to_string())
                                            } else if let Some(n) = v.as_u64() {
                                                Some(n.to_string())
                                            } else {
                                                None
                                            }
                                        })
                                    )
                                } else {
                                    (None, None)
                                };
                                
                                // Create a Transaction object
                                let tx = Transaction {
                                    hash,
                                    tx_type: tx_type.to_string(),
                                    timestamp: chrono::Utc::now(),
                                    account,
                                    amount,
                                    taker_gets,
                                    taker_pays,
                                };
                                
                                // Use a shorter lock duration to reduce contention
                                {
                                    let mut state = app_state.lock().unwrap();
                                    state.add_transaction(tx);
                                }
                                // Don't log every transaction to reduce console clutter
                                // info!("Added transaction: {}", tx_type);
                            }
                            } else if let Some(engine_result) = value.get("engine_result") {
                                // Only log non-success API responses
                                if engine_result.as_str().map_or(false, |r| r != "tesSUCCESS") {
                                    debug!("Received API response: {}", engine_result);
                                }
                            }
                        },
                        Err(e) => {
                            // Only log parsing errors for non-empty messages
                            if !text.trim().is_empty() {
                                debug!("Failed to parse message: {}", e);
                            }
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    ws_stream.send(Message::Pong(data)).await?;
                }
                Err(e) => {
                    // Use structured error logging with error code if available
                    if let Some(_code) = e.to_string().find("code") {
                        error!("WebSocket error (code): {}", e);
                    } else {
                        error!("WebSocket error: {}", e);
                    }
                    break;
                }
                _ => {}
            }

            // Check if reconnection was requested
            {
                let mut state = app_state.lock().unwrap();
                if state.reconnect_requested {
                    state.reconnect_requested = false;
                    break;
                }
            }
        }

        // Update connection status
        {
            let mut state = app_state.lock().unwrap();
            state.connected = false;
        }

        Ok(())
    }
}