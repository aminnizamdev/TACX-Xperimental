use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Tab {
    Transactions,
    Offers,
    Statistics,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Transaction {
    pub hash: String,
    pub tx_type: String,
    pub timestamp: DateTime<Utc>,
    pub account: Option<String>,
    pub amount: Option<String>,
    pub taker_gets: Option<String>,
    pub taker_pays: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Offer {
    pub hash: String,
    pub account: String,
    pub timestamp: DateTime<Utc>,
    pub taker_gets: String,
    pub taker_pays: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientMessage {
    pub command: String,
    pub id: Option<String>,
    pub streams: Option<Vec<String>>,
}

impl ClientMessage {
    pub fn subscribe() -> Self {
        Self {
            command: "subscribe".to_string(),
            id: Some("monitor".to_string()),
            streams: Some(vec!["transactions_proposed".to_string(), "transactions".to_string()]),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub connected: bool,
    pub active_tab: Tab,
    pub transactions: Vec<Transaction>,
    pub offers: Vec<Offer>,
    pub tx_count: usize,
    pub tx_scroll: usize,
    pub offer_scroll: usize,
    pub tx_type_counts: HashMap<String, usize>,
    pub tx_rate_history: Vec<usize>,
    pub last_tx_time: SystemTime,
    pub reconnect_requested: bool,
    pub history_size: usize,
    pub pending_transactions: Vec<Transaction>,
    pub batch_processing: bool,
    pub last_ui_update: SystemTime,
}

impl AppState {
    pub fn new(history_size: usize) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            connected: false,
            active_tab: Tab::Transactions,
            transactions: Vec::with_capacity(history_size),
            offers: Vec::with_capacity(history_size),
            tx_count: 0,
            tx_scroll: 0,
            offer_scroll: 0,
            tx_type_counts: HashMap::new(),
            tx_rate_history: vec![0; 60],
            last_tx_time: SystemTime::now(),
            reconnect_requested: false,
            history_size,
            pending_transactions: Vec::with_capacity(100),
            batch_processing: true,
            last_ui_update: SystemTime::now(),
        }))
    }

    pub fn add_transaction(&mut self, tx: Transaction) {
        // Update transaction count
        self.tx_count += 1;

        // Update transaction type counts
        *self.tx_type_counts.entry(tx.tx_type.clone()).or_insert(0) += 1;

        // Update transaction rate
        let now = SystemTime::now();
        let elapsed = now.duration_since(self.last_tx_time).unwrap_or(Duration::from_secs(0));
        if elapsed >= Duration::from_secs(1) {
            // Shift history using more efficient slice operations
            if self.tx_rate_history.len() > 1 {
                self.tx_rate_history.copy_within(1.., 0);
            }
            // Add new rate
            let last_idx = self.tx_rate_history.len() - 1;
            self.tx_rate_history[last_idx] = self.tx_count;
            // Reset count and update time
            self.tx_count = 0;
            self.last_tx_time = now;
        }

        // If batch processing is enabled, add to pending transactions
        if self.batch_processing {
            self.pending_transactions.push(tx.clone());
            
            // Only process batch if enough time has passed since last UI update
            // or if we have accumulated too many pending transactions
            let ui_elapsed = now.duration_since(self.last_ui_update).unwrap_or(Duration::from_secs(0));
            if ui_elapsed >= Duration::from_millis(100) || self.pending_transactions.len() >= 50 {
                self.process_pending_transactions();
                self.last_ui_update = now;
            }
        } else {
            // Add directly to transactions list with bounds checking
            self.add_transaction_to_list(tx);
        }
    }
    
    fn add_transaction_to_list(&mut self, tx: Transaction) {
        // Add to transactions list with capacity check
        if self.transactions.len() >= self.history_size {
            // More efficient to remove from the front when at capacity
            self.transactions.remove(0);
        }
        self.transactions.push(tx.clone());

        // If it's an OfferCreate, add to offers list with more lenient field requirements
        if tx.tx_type == "OfferCreate" {
            // Create offer with more professional placeholders for missing fields
            let offer = Offer {
                hash: tx.hash,
                account: tx.account.unwrap_or_else(|| "—".to_string()),
                timestamp: tx.timestamp,
                taker_gets: tx.taker_gets.unwrap_or_else(|| "N/A".to_string()),
                taker_pays: tx.taker_pays.unwrap_or_else(|| "N/A".to_string()),
            };
            
            // Add to offers list with capacity check
            if self.offers.len() >= self.history_size {
                self.offers.remove(0);
            }
            self.offers.push(offer);
        }
    }
    
    fn process_pending_transactions(&mut self) {
        // Skip if no pending transactions
        if self.pending_transactions.is_empty() {
            return;
        }
        
        // Process all pending transactions in batch
        // Collect transactions first to avoid multiple mutable borrows
        let transactions = std::mem::take(&mut self.pending_transactions);
        for tx in transactions {
            self.add_transaction_to_list(tx);
        }
    }
    
    // Call this method periodically to ensure pending transactions are processed
    pub fn flush_pending_transactions(&mut self) {
        self.process_pending_transactions();
    }
}