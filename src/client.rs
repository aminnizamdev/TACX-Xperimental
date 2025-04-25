use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use backoff::ExponentialBackoffBuilder;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, warn};

use crate::models::{AppState, ClientMessage, Transaction};
use crate::security::{ConnectionTracker, validate_websocket_url, validate_message, create_tls_connector, log_error, redact_sensitive_data};

pub struct RippleClient {
    server_url: String,
    connection_tracker: ConnectionTracker,
}

impl RippleClient {
    pub fn new(server_url: String) -> Self {
        Self { 
            server_url,
            connection_tracker: ConnectionTracker::new(),
        }
    }

    pub async fn connect(&self, app_state: Arc<Mutex<AppState>>) -> Result<()> {
        // Validate the WebSocket URL for security issues
        let url = validate_websocket_url(&self.server_url)
            .context("Invalid WebSocket URL")?;
        debug!("Connecting to {}", url);

        // Apply rate limiting to prevent DoS
        if !self.connection_tracker.check_connection_limit(&self.server_url) {
            let backoff = self.connection_tracker.get_backoff_time(&self.server_url);
            warn!("Connection rate limit exceeded. Backing off for {} seconds", backoff.as_secs());
            tokio::time::sleep(backoff).await;
        }

        // Configure backoff strategy
        let _backoff = ExponentialBackoffBuilder::new()
            .with_initial_interval(Duration::from_millis(500))
            .with_max_interval(Duration::from_secs(30))
            .with_multiplier(2.0)
            .with_max_elapsed_time(Some(Duration::from_secs(300)))
            .build();

        // Create secure TLS connector
        let tls_connector = create_tls_connector()
            .context("Failed to create secure TLS connector")?;
        let connector = tokio_tungstenite::Connector::NativeTls(tls_connector);

        // Connect to WebSocket with error handling and TLS
        let ws_stream = match tokio_tungstenite::connect_async_tls_with_config(
            url,
            None,
            false,
            Some(connector)
        ).await {
            Ok((ws_stream, response)) => {
                // Verify the response status code
                if !response.status().is_informational() && !response.status().is_success() {
                    return Err(anyhow::anyhow!("WebSocket connection failed with status: {}", response.status()));
                }
                debug!("Connected to Ripple WebSocket server");
                
                // Update connection status
                {
                    let mut state = app_state.lock().unwrap();
                    state.connected = true;
                }
                
                ws_stream
            },
            Err(e) => {
                // Securely log the error without exposing sensitive information
                let redacted_error = redact_sensitive_data(&e.to_string());
                warn!("Failed to connect to WebSocket server: {}", redacted_error);
                return Err(anyhow::anyhow!("WebSocket connection failed"));
            }
        };

        // Handle the connection
        self.handle_connection(ws_stream, app_state).await?;
        
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
            log_error("Failed to send subscription message", &e.into());
            return Err(anyhow::anyhow!("Failed to subscribe"));
        }
        debug!("Subscribed to transactions");

        // Process incoming messages
        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Validate and sanitize the message
                    match validate_message(&text) {
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
                                    state.check_and_log_high_value(&tx);
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
                            // Securely log message validation errors
                            debug!("Invalid message received: {}", e);
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    // Respond to ping messages to maintain connection
                    if let Err(e) = ws_stream.send(Message::Pong(data)).await {
                        log_error("Failed to respond to ping", &e.into());
                    }
                }
                Ok(Message::Close(frame)) => {
                    // Handle graceful connection closure
                    if let Some(frame) = frame {
                        debug!("WebSocket closed with code {}: {}", frame.code, frame.reason);
                    } else {
                        debug!("WebSocket closed");
                    }
                    break;
                }
                Err(e) => {
                    // Use structured error logging with error code if available
                    let error_msg = redact_sensitive_data(&e.to_string());
                    if let Some(_code) = error_msg.find("code") {
                        error!("WebSocket error (code): {}", error_msg);
                    } else {
                        error!("WebSocket error: {}", error_msg);
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