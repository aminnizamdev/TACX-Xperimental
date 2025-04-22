//! Security module for the Ripple transaction monitor
//!
//! This module provides security enhancements including:
//! - Input validation for WebSocket messages
//! - Rate limiting for reconnection attempts
//! - TLS certificate validation
//! - Secure error handling
//! - Message sanitization

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde_json::Value;
use tracing::{debug, error, warn};
use url::Url;

/// Validates a WebSocket URL for security issues
pub fn validate_websocket_url(url_str: &str) -> Result<Url> {
    // Parse the URL
    let url = Url::parse(url_str)
        .context("Invalid WebSocket URL format")?;
    
    // Ensure the URL uses a secure protocol
    if url.scheme() != "wss" {
        warn!("Using insecure WebSocket connection (ws://). Consider using wss:// for encryption");
    }
    
    // Validate the host
    if url.host_str().is_none() {
        return Err(anyhow::anyhow!("Missing host in WebSocket URL"));
    }
    
    // Check for suspicious hosts or paths
    let host = url.host_str().unwrap().to_lowercase();
    if host.contains("localhost") || host.contains("127.0.0.1") || host.contains("0.0.0.0") {
        warn!("Connecting to local WebSocket server. This may be insecure in production");
    }
    
    Ok(url)
}

/// Validates and sanitizes incoming WebSocket messages
pub fn validate_message(msg: &str) -> Result<Value> {
    // Check message size to prevent DoS
    if msg.len() > 1_000_000 { // 1MB limit
        return Err(anyhow::anyhow!("Message too large"));
    }
    
    // Parse JSON with a depth limit to prevent stack overflow attacks
    let parsed: Value = serde_json::from_str(msg)
        .context("Invalid JSON in WebSocket message")?;
    
    // Validate message structure
    if let Some(tx) = parsed.get("transaction") {
        // Validate transaction fields
        if !tx.is_object() {
            return Err(anyhow::anyhow!("Invalid transaction format"));
        }
        
        // Check for required fields
        if tx.get("TransactionType").is_none() {
            debug!("Received transaction without TransactionType field");
        }
    }
    
    Ok(parsed)
}

/// Rate limiter for connection attempts
pub struct RateLimiter {
    attempts: HashMap<String, Vec<Instant>>,
    window: Duration,
    max_attempts: usize,
}

impl RateLimiter {
    pub fn new(window_secs: u64, max_attempts: usize) -> Self {
        Self {
            attempts: HashMap::new(),
            window: Duration::from_secs(window_secs),
            max_attempts,
        }
    }
    
    pub fn check_rate_limit(&mut self, key: &str) -> bool {
        let now = Instant::now();
        let attempts = self.attempts.entry(key.to_string()).or_insert_with(Vec::new);
        
        // Remove attempts outside the time window
        attempts.retain(|time| now.duration_since(*time) < self.window);
        
        // Check if we're over the limit
        if attempts.len() >= self.max_attempts {
            return false;
        }
        
        // Record this attempt
        attempts.push(now);
        true
    }
    
    pub fn get_retry_after(&self, key: &str) -> Option<Duration> {
        if let Some(attempts) = self.attempts.get(key) {
            if !attempts.is_empty() && attempts.len() >= self.max_attempts {
                // Calculate time until oldest attempt expires
                let oldest = attempts[0];
                let elapsed = Instant::now().duration_since(oldest);
                if elapsed < self.window {
                    return Some(self.window - elapsed);
                }
            }
        }
        None
    }
}

/// Secure TLS configuration for WebSocket connections
pub fn create_tls_connector() -> Result<native_tls::TlsConnector> {
    let mut builder = native_tls::TlsConnector::builder();
    
    // Require modern TLS versions
    builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));
    
    // Enable certificate verification
    builder.danger_accept_invalid_certs(false);
    builder.danger_accept_invalid_hostnames(false);
    
    // Build the connector
    let connector = builder.build()
        .context("Failed to create TLS connector")?;
    
    Ok(connector)
}

/// Redacts sensitive information from error messages and logs
pub fn redact_sensitive_data(input: &str) -> String {
    // Redact account addresses
    let account_regex = regex::Regex::new(r"r[a-zA-Z0-9]{24,}").unwrap();
    let redacted = account_regex.replace_all(input, "r...REDACTED...");
    
    // Redact potential private keys (hex strings)
    let key_regex = regex::Regex::new(r"[0-9a-fA-F]{64,}").unwrap();
    let redacted = key_regex.replace_all(&redacted, "...REDACTED_KEY...");
    
    redacted.to_string()
}

/// Safely logs errors without exposing sensitive information
pub fn log_error(context: &str, error: &anyhow::Error) {
    let error_str = error.to_string();
    let redacted_error = redact_sensitive_data(&error_str);
    error!("{}: {}", context, redacted_error);
}

/// Thread-safe connection attempt tracker to prevent DoS
pub struct ConnectionTracker {
    rate_limiter: Arc<Mutex<RateLimiter>>,
}

impl ConnectionTracker {
    pub fn new() -> Self {
        Self {
            rate_limiter: Arc::new(Mutex::new(RateLimiter::new(60, 10))), // 10 attempts per minute
        }
    }
    
    pub fn check_connection_limit(&self, server: &str) -> bool {
        let mut limiter = self.rate_limiter.lock().unwrap();
        limiter.check_rate_limit(server)
    }
    
    pub fn get_backoff_time(&self, server: &str) -> Duration {
        let limiter = self.rate_limiter.lock().unwrap();
        limiter.get_retry_after(server).unwrap_or(Duration::from_secs(5))
    }
}

/// Default implementation of ConnectionTracker
impl Default for ConnectionTracker {
    fn default() -> Self {
        Self::new()
    }
}