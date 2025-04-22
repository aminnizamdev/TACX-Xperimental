use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use ratatui::widgets::*;
// Fix unused imports
use tracing::error;

use crate::formatter;
use crate::models::{AppState, Tab};

pub struct UI {
    state: Arc<Mutex<AppState>>,
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    update_interval: Duration,
    last_render_hash: u64,
}

impl UI {
    pub fn new(state: Arc<Mutex<AppState>>, update_interval: Duration) -> Result<Self> {
        // Setup terminal
        enable_raw_mode()?;
        std::io::stdout().execute(EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

        Ok(Self {
            state,
            terminal,
            update_interval,
            last_render_hash: 0,
        })
    }
    
    // Calculate a simple hash of the state to detect changes
    fn calculate_state_hash(&self, state: &AppState) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        
        // Hash key state elements that affect rendering
        state.active_tab.hash(&mut hasher);
        state.connected.hash(&mut hasher);
        state.tx_scroll.hash(&mut hasher);
        state.offer_scroll.hash(&mut hasher);
        state.transactions.len().hash(&mut hasher);
        state.offers.len().hash(&mut hasher);
        
        // Hash the most recent transactions (up to 10)
        let tx_count = state.transactions.len().min(10);
        if tx_count > 0 {
            for i in 0..tx_count {
                let idx = state.transactions.len() - 1 - i;
                state.transactions[idx].hash.hash(&mut hasher);
            }
        }
        
        hasher.finish()
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut last_update = std::time::Instant::now();
        let mut last_flush = std::time::Instant::now();

        loop {
            // Periodically flush pending transactions to ensure they're processed
            if last_flush.elapsed() >= Duration::from_millis(100) {
                let mut state = self.state.lock().unwrap();
                state.flush_pending_transactions();
                last_flush = std::time::Instant::now();
            }
            
            // Check if it's time to update the UI
            if last_update.elapsed() >= self.update_interval {
                // Calculate a simple hash of the state to detect changes
                let render_needed = {
                    let state = self.state.lock().unwrap();
                    let new_hash = self.calculate_state_hash(&state);
                    let changed = new_hash != self.last_render_hash;
                    if changed {
                        self.last_render_hash = new_hash;
                    }
                    changed
                };
                
                // Only redraw if the state has changed
                if render_needed {
                    self.terminal.draw(|frame| {
                        let state = self.state.lock().unwrap();
                        draw_ui(frame, &state);
                    })?;
                }
                
                last_update = std::time::Instant::now();
            }

            // Handle input events
            if event::poll(Duration::from_millis(10))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            break;
                        }
                        KeyCode::Tab => {
                            let mut state = self.state.lock().unwrap();
                            state.active_tab = match state.active_tab {
                                Tab::Transactions => Tab::Offers,
                                Tab::Offers => Tab::Statistics,
                                Tab::Statistics => Tab::Transactions,
                            };
                        }
                        KeyCode::Char('1') => {
                            let mut state = self.state.lock().unwrap();
                            state.active_tab = Tab::Transactions;
                        }
                        KeyCode::Char('2') => {
                            let mut state = self.state.lock().unwrap();
                            state.active_tab = Tab::Offers;
                        }
                        KeyCode::Char('3') => {
                            let mut state = self.state.lock().unwrap();
                            state.active_tab = Tab::Statistics;
                        }
                        KeyCode::Up => {
                            let mut state = self.state.lock().unwrap();
                            match state.active_tab {
                                Tab::Transactions => {
                                    if state.tx_scroll > 0 {
                                        state.tx_scroll -= 1;
                                    }
                                }
                                Tab::Offers => {
                                    if state.offer_scroll > 0 {
                                        state.offer_scroll -= 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                        KeyCode::Down => {
                            let mut state = self.state.lock().unwrap();
                            match state.active_tab {
                                Tab::Transactions => {
                                    if state.tx_scroll < state.transactions.len().saturating_sub(1) {
                                        state.tx_scroll += 1;
                                    }
                                }
                                Tab::Offers => {
                                    if state.offer_scroll < state.offers.len().saturating_sub(1) {
                                        state.offer_scroll += 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                        KeyCode::Char('r') => {
                            // Request reconnection
                            let mut state = self.state.lock().unwrap();
                            state.reconnect_requested = true;
                        }
                        _ => {}
                    }
                }
            }

            // Adaptive sleep to prevent CPU hogging
            // Sleep longer when inactive to reduce resource usage
            let sleep_duration = if event::poll(Duration::from_millis(1))? {
                Duration::from_millis(1) // Short sleep when there's input
            } else {
                Duration::from_millis(10) // Longer sleep when idle
            };
            tokio::time::sleep(sleep_duration).await;
        }

        // Restore terminal
        disable_raw_mode()?;
        std::io::stdout().execute(LeaveAlternateScreen)?;

        Ok(())
    }
}

impl Drop for UI {
    fn drop(&mut self) {
        // Attempt to restore terminal on drop
        if let Err(e) = disable_raw_mode() {
            error!("Failed to disable raw mode: {}", e);
        }
        if let Err(e) = std::io::stdout().execute(LeaveAlternateScreen) {
            error!("Failed to leave alternate screen: {}", e);
        }
    }
}

// Draw the main UI
fn draw_ui(frame: &mut Frame, state: &AppState) {
    // Create layout - optimized to use less vertical space
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // Title - reduced from 3 to 2
            Constraint::Min(0),     // Content
            Constraint::Length(2),  // Status bar - reduced from 3 to 2
        ])
        .split(frame.size());

    // Draw title
    let title = Paragraph::new("Ripple Transaction Monitor")
        .style(Style::default().fg(Color::Cyan).bold())
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    // Draw tabs
    let tabs = Tabs::new(vec![Line::from("Transactions"), Line::from("OfferCreate"), Line::from("Statistics")])
        .select(match state.active_tab {
            Tab::Transactions => 0,
            Tab::Offers => 1,
            Tab::Statistics => 2,
        })
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).bold())
        .divider("|");
    frame.render_widget(tabs, chunks[0]);

    // Draw content based on active tab
    match state.active_tab {
        Tab::Transactions => draw_transactions(frame, state, chunks[1]),
        Tab::Offers => draw_offers(frame, state, chunks[1]),
        Tab::Statistics => draw_statistics(frame, state, chunks[1]),
    }

    // Draw status bar
    draw_stats(frame, state, chunks[2]);
}

// Draw the status bar
fn draw_stats(frame: &mut Frame, state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(50),
        ])
        .split(area);

    // Connection status with compact display
    let status_text = match state.connected {
        true => "✓ Connected",
        false => "✗ Disconnected",
    };
    let status_style = match state.connected {
        true => Style::default().fg(Color::Green),
        false => Style::default().fg(Color::Red),
    };
    let status = Paragraph::new(status_text)
        .style(status_style)
        .alignment(Alignment::Left);
    frame.render_widget(status, chunks[0]);

    // Transaction count with more info
    let tx_count = Paragraph::new(format!("TXs: {} | Types: {}", 
                                         state.tx_count, 
                                         state.tx_type_counts.len()))
        .alignment(Alignment::Center);
    frame.render_widget(tx_count, chunks[1]);

    // Help text with compact keys
    let help = Paragraph::new("q:quit | Tab/1/2/3:switch | r:reconnect | ↑/↓:scroll")
        .alignment(Alignment::Right);
    frame.render_widget(help, chunks[2]);
}

// Draw the transactions tab
fn draw_transactions(frame: &mut Frame, state: &AppState, area: Rect) {
    let transactions = state.transactions.iter().map(|tx| {
        let time = formatter::format_timestamp(&tx.timestamp);
        let tx_type = formatter::get_tx_type_description(&tx.tx_type);
        // Truncate hash to save space
        let hash = if tx.hash.len() > 10 {
            format!("{}...", &tx.hash[0..10])
        } else {
            tx.hash.clone()
        };
        let account = tx.account.as_ref().map(|a| formatter::format_account(a)).unwrap_or_default();
        
        // Format amount or create a summary based on transaction type
        let value_display = match tx.tx_type.as_str() {
            "Payment" => tx.amount.as_ref().map(|a| formatter::format_currency(a)).unwrap_or_default(),
            "OfferCreate" => {
                if let (Some(gets), Some(pays)) = (&tx.taker_gets, &tx.taker_pays) {
                    formatter::format_offer(gets, pays)
                } else {
                    "Unknown offer".to_string()
                }
            },
            _ => formatter::get_tx_summary(&tx.tx_type, 
                                         tx.amount.as_ref().map(|s| s.as_str()), 
                                         tx.taker_gets.as_ref().map(|s| s.as_str()), 
                                         tx.taker_pays.as_ref().map(|s| s.as_str()))
        };
        
        // Apply color based on transaction type
        let tx_type_style = Style::default().fg(formatter::get_tx_type_color(&tx.tx_type));
        
        // Create cells with individual styling
        let cells = vec![
            Cell::from(time),
            Cell::from(tx_type.to_string()).style(tx_type_style),
            Cell::from(hash),
            Cell::from(account),
            Cell::from(value_display)
        ];
        
        Row::new(cells)
    }).collect::<Vec<_>>();

    let header = Row::new(vec!["Time", "Type", "Hash", "Account", "Description"])
        .style(Style::default().fg(Color::Yellow))
        .bottom_margin(0); // Reduced from 1 to 0 to save space

    let table = Table::new(transactions)
        .header(header)
        .block(Block::default().title("Transactions").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .widths(&[
            Constraint::Length(19),  // Time - expanded for full timestamp
            Constraint::Length(16),  // Type - expanded for descriptive names
            Constraint::Length(12),  // Hash - reduced to save space
            Constraint::Length(10),  // Account - reduced to save space
            Constraint::Min(20),     // Description - expanded for readable summaries
        ]);

    let mut table_state = TableState::default();
    table_state.select(Some(state.tx_scroll));
    frame.render_stateful_widget(
        table,
        area,
        &mut table_state,
    );
}

// Draw the offers tab
fn draw_offers(frame: &mut Frame, state: &AppState, area: Rect) {
    let offers = state.offers.iter().map(|offer| {
        let time = formatter::format_timestamp(&offer.timestamp);
        // Format account
        let account = formatter::format_account(&offer.account);
        
        // Format currency values
        let gets = formatter::format_currency(&offer.taker_gets);
        let pays = formatter::format_currency(&offer.taker_pays);
        
        // Extract market pair
        let market_pair = formatter::format_market_pair(&offer.taker_gets, &offer.taker_pays);
        
        // Calculate price if possible
        let price = formatter::calculate_price(&offer.taker_gets, &offer.taker_pays)
            .map_or("N/A".to_string(), |p| format!("{:.5}", p));
        
        // Create a human-readable summary using the enhanced format_offer
        let summary = formatter::format_offer(&offer.taker_gets, &offer.taker_pays);
        
        Row::new(vec![time, account, gets, pays, market_pair, price, summary])
            .style(Style::default())
    }).collect::<Vec<_>>();

    let header = Row::new(vec!["Time", "Account", "Selling", "Buying", "Market Pair", "Price", "Summary"])
        .style(Style::default().fg(Color::Yellow))
        .bottom_margin(0); // Reduced from 1 to 0 to save space

    let table = Table::new(offers)
        .header(header)
        .block(Block::default().title("Market Orders (OfferCreate)").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .widths(&[
            Constraint::Length(19),  // Time - expanded for full timestamp
            Constraint::Length(10),  // Account - reduced
            Constraint::Length(15),  // Selling (Taker Gets)
            Constraint::Length(15),  // Buying (Taker Pays)
            Constraint::Length(10),  // Market Pair
            Constraint::Length(10),  // Price
            Constraint::Min(20),     // Summary - human-readable description
        ]);

    let mut table_state = TableState::default();
    table_state.select(Some(state.offer_scroll));
    frame.render_stateful_widget(
        table,
        area,
        &mut table_state,
    );
}

// Draw the statistics tab
fn draw_statistics(frame: &mut Frame, state: &AppState, area: Rect) {
    // Use vertical layout for better organization
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    // Upper section with transaction types and rates
    let upper_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(main_chunks[0]);

    // Transaction type distribution
    let tx_types = state.tx_type_counts.iter()
        .map(|(tx_type, count)| (formatter::get_tx_type_description(tx_type), *count as u64))
        .collect::<Vec<_>>();

    let tx_type_chart = BarChart::default()
        .block(Block::default().title("Transaction Types").borders(Borders::ALL))
        .bar_width(5)
        .bar_gap(3)
        .bar_style(Style::default().fg(Color::Blue))
        .value_style(Style::default().fg(Color::Black).bg(Color::Blue))
        .data(&tx_types)
        .max(tx_types.iter().map(|(_, count)| *count).max().unwrap_or(1));

    frame.render_widget(tx_type_chart, upper_chunks[0]);

    // Transaction rate over time
    let tx_rate_data = state.tx_rate_history.iter()
        .enumerate()
        .map(|(i, rate)| (i as f64, *rate as f64))
        .collect::<Vec<_>>();

    let tx_rate_dataset = Dataset::default()
        .name("Transactions per second")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Cyan))
        .data(&tx_rate_data);

    let tx_rate_chart = Chart::new(vec![tx_rate_dataset])
        .block(Block::default().title("Transaction Rate").borders(Borders::ALL))
        .x_axis(
            Axis::default()
                .title("Time (seconds)")
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, tx_rate_data.len() as f64])
                .labels(vec!["60s ago".into(), "30s ago".into(), "now".into()]),
        )
        .y_axis(
            Axis::default()
                .title("TPS")
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, state.tx_rate_history.iter().copied().max().unwrap_or(10) as f64 * 1.1])
                .labels(vec!["0".into(), "max".into()]),
        );

    frame.render_widget(tx_rate_chart, upper_chunks[1]);

    // Lower section with market data
    let lower_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(main_chunks[1]);

    // Popular trading pairs
    let mut market_pairs = std::collections::HashMap::new();
    for offer in &state.offers {
        let pair = formatter::format_market_pair(&offer.taker_gets, &offer.taker_pays);
        *market_pairs.entry(pair).or_insert(0) += 1;
    }

    let mut pairs: Vec<_> = market_pairs.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count, descending
    
    // Convert to a format compatible with BarChart (using string slices instead of owned strings)
    let pairs_data: Vec<(&str, u64)> = pairs.iter()
        .take(10) // Top 10 pairs
        .map(|(pair, count)| (pair.as_str(), *count as u64))
        .collect();

    let pairs_chart = BarChart::default()
        .block(Block::default().title("Popular Trading Pairs").borders(Borders::ALL))
        .bar_width(7)
        .bar_gap(1)
        .bar_style(Style::default().fg(Color::Green))
        .value_style(Style::default().fg(Color::Black).bg(Color::Green))
        .data(&pairs_data)
        .max(pairs_data.iter().map(|(_, count)| *count).max().unwrap_or(1));

    frame.render_widget(pairs_chart, lower_chunks[0]);

    // Transaction volume summary
    let mut summary_text = Vec::new();
    
    // Total transactions
    let total_txs: usize = state.tx_type_counts.values().sum();
    summary_text.push(Line::from(vec![
        Span::styled("Total Transactions: ", Style::default().fg(Color::Yellow)),
        Span::raw(format!("{}", total_txs))
    ]));
    
    // Payment volume
    let payment_count = state.tx_type_counts.get("Payment").unwrap_or(&0);
    summary_text.push(Line::from(vec![
        Span::styled("Payment Transactions: ", Style::default().fg(Color::Green)),
        Span::raw(format!("{} ({:.1}%)", payment_count, if total_txs > 0 { (*payment_count as f64 / total_txs as f64) * 100.0 } else { 0.0 }))
    ]));
    
    // OfferCreate volume
    let offer_count = state.tx_type_counts.get("OfferCreate").unwrap_or(&0);
    summary_text.push(Line::from(vec![
        Span::styled("Market Orders: ", Style::default().fg(Color::Blue)),
        Span::raw(format!("{} ({:.1}%)", offer_count, if total_txs > 0 { (*offer_count as f64 / total_txs as f64) * 100.0 } else { 0.0 }))
    ]));
    
    // Current TPS
    let current_tps = state.tx_rate_history.last().unwrap_or(&0);
    summary_text.push(Line::from(vec![
        Span::styled("Current TPS: ", Style::default().fg(Color::Cyan)),
        Span::raw(format!("{}", current_tps))
    ]));
    
    // Peak TPS
    let peak_tps = state.tx_rate_history.iter().max().unwrap_or(&0);
    summary_text.push(Line::from(vec![
        Span::styled("Peak TPS: ", Style::default().fg(Color::Magenta)),
        Span::raw(format!("{}", peak_tps))
    ]));
    
    // Add empty line as separator
    summary_text.push(Line::from(""));
    
    // Network activity summary
    summary_text.push(Line::from(vec![Span::styled("Network Activity Summary", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]));
    
    // Add activity level description
    let activity_level = if *current_tps < 5 {
        ("Low", Color::Green)
    } else if *current_tps < 20 {
        ("Moderate", Color::Yellow)
    } else {
        ("High", Color::Red)
    };
    
    summary_text.push(Line::from(vec![
        Span::raw("Activity Level: "),
        Span::styled(activity_level.0, Style::default().fg(activity_level.1).add_modifier(Modifier::BOLD))
    ]));
    
    // Add network health indicator
    let health_indicator = if state.connected {
        ("Healthy", Color::Green)
    } else {
        ("Disconnected", Color::Red)
    };
    
    summary_text.push(Line::from(vec![
        Span::raw("Network Status: "),
        Span::styled(health_indicator.0, Style::default().fg(health_indicator.1).add_modifier(Modifier::BOLD))
    ]));

    let summary = Paragraph::new(summary_text)
        .block(Block::default().title("Transaction Metrics").borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    frame.render_widget(summary, lower_chunks[1]);
}