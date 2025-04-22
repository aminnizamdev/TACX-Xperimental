use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;

// Global regex patterns to avoid recompilation
lazy_static! {
    static ref CURRENCY_REGEX: Regex = Regex::new(r#"\{"currency":"([A-Z0-9]{3,})","issuer":"([a-zA-Z0-9]+)","value":"([0-9.]+)"\}"#).unwrap();
}

/// Formats a raw currency value to a human-readable format with 5 decimal places
pub fn format_currency(value: &str) -> String {
    // Try to parse as a number first
    if let Ok(num) = value.parse::<f64>() {
        // XRP is represented as drops (1 XRP = 1,000,000 drops)
        let xrp_value = num / 1_000_000.0;
        return format!("XRP {:.5}", xrp_value);
    }
    
    // Check if it's a currency object in JSON format using the globally cached regex
    
    if let Some(caps) = CURRENCY_REGEX.captures(value) {
        let currency = caps.get(1).map_or("", |m| m.as_str());
        let issuer = caps.get(2).map_or("", |m| m.as_str());
        let value_str = caps.get(3).map_or("", |m| m.as_str());
        if let Ok(value_num) = value_str.parse::<f64>() {
            // Format with exactly 5 decimal places and add currency code
            return format!("{:.5} {} ({}...)", value_num, currency, &issuer[0..6]);
        }
    }
    
    // If we can't parse it, return the original with a note
    format!("{}", value)
}

/// Formats a timestamp to a human-readable format
pub fn format_timestamp(timestamp: &DateTime<Utc>) -> String {
    // Format with date and time in a compact but readable format
    timestamp.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Returns a human-readable description of a transaction type
pub fn get_tx_type_description(tx_type: &str) -> &str {
    match tx_type {
        "Payment" => "Money Transfer",
        "OfferCreate" => "New Market Order",
        "OfferCancel" => "Cancelled Order",
        "TrustSet" => "Trust Line Setup",
        "AccountSet" => "Account Settings",
        "SetRegularKey" => "Security Key Change",
        "SignerListSet" => "Signers Change",
        "EscrowCreate" => "Escrow Creation",
        "EscrowFinish" => "Escrow Completion",
        "EscrowCancel" => "Escrow Cancellation",
        "PaymentChannelCreate" => "Payment Channel Open",
        "PaymentChannelFund" => "Channel Funding",
        "PaymentChannelClaim" => "Channel Claim",
        "DepositPreauth" => "Deposit Preapproval",
        "CheckCreate" => "Check Issuance",
        "CheckCash" => "Check Redemption",
        "CheckCancel" => "Check Cancellation",
        "TicketCreate" => "Ticket Creation",
        "NFTokenMint" => "NFT Minting",
        "NFTokenBurn" => "NFT Burning",
        _ => tx_type,
    }
}

/// Returns a color for a transaction type
pub fn get_tx_type_color(tx_type: &str) -> ratatui::style::Color {
    use ratatui::style::Color;
    
    match tx_type {
        "Payment" => Color::Green,
        "OfferCreate" => Color::Blue,
        "OfferCancel" => Color::Red,
        "TrustSet" => Color::Yellow,
        "AccountSet" => Color::Cyan,
        "EscrowCreate" | "EscrowFinish" | "EscrowCancel" => Color::Magenta,
        "PaymentChannelCreate" | "PaymentChannelFund" | "PaymentChannelClaim" => Color::LightBlue,
        "CheckCreate" | "CheckCash" | "CheckCancel" => Color::LightGreen,
        "NFTokenMint" | "NFTokenBurn" => Color::LightMagenta,
        _ => Color::White,
    }
}

/// Formats an offer in a human-readable way with price calculation
pub fn format_offer(taker_gets: &str, taker_pays: &str) -> String {
    // Check for placeholder values first
    if taker_gets == "N/A" || taker_pays == "N/A" {
        return "Market order with incomplete data".to_string();
    }
    
    let gets = format_currency(taker_gets);
    let pays = format_currency(taker_pays);
    
    // Calculate and include the price ratio if possible
    if let (Ok(gets_num), Ok(pays_num)) = (taker_gets.parse::<f64>(), taker_pays.parse::<f64>()) {
        // For XRP values (represented as drops)
        let gets_value = gets_num / 1_000_000.0;
        let pays_value = pays_num / 1_000_000.0;
        let price = pays_value / gets_value;
        return format!("Sell {} for {} (Price: {:.5} XRP)", gets, pays, price);
    }
    
    // Try to extract values from currency objects using the globally cached regex
    
    if let (Some(gets_caps), Some(pays_caps)) = (CURRENCY_REGEX.captures(taker_gets), CURRENCY_REGEX.captures(taker_pays)) {
        let gets_currency = gets_caps.get(1).map_or("", |m| m.as_str());
        let pays_currency = pays_caps.get(1).map_or("", |m| m.as_str());
        let gets_value_str = gets_caps.get(3).map_or("", |m| m.as_str());
        let pays_value_str = pays_caps.get(3).map_or("", |m| m.as_str());
        
        if let (Ok(gets_value), Ok(pays_value)) = (gets_value_str.parse::<f64>(), pays_value_str.parse::<f64>()) {
            let price = pays_value / gets_value;
            let market_pair = format!("{}/{}", gets_currency, pays_currency);
            return format!("Sell {} for {} (Pair: {}, Price: {:.5})", gets, pays, market_pair, price);
        }
    }
    
    // Default format if we can't calculate price
    format!("Sell {} for {}", gets, pays)
}

/// Returns a human-readable summary of a transaction
pub fn get_tx_summary(tx_type: &str, amount: Option<&str>, taker_gets: Option<&str>, taker_pays: Option<&str>) -> String {
    match tx_type {
        "Payment" => {
            if let Some(amt) = amount {
                format!("Transferred {}", format_currency(amt))
            } else {
                "Payment with unknown amount".to_string()
            }
        },
        "OfferCreate" => {
            if let (Some(gets), Some(pays)) = (taker_gets, taker_pays) {
                // Use the enhanced format_offer function for better readability
                format!("Market order: {}", format_offer(gets, pays))
            } else {
                "Created offer with unknown details".to_string()
            }
        },
        "OfferCancel" => "Cancelled an existing market order".to_string(),
        "TrustSet" => "Established a trust line with another account".to_string(),
        "AccountSet" => "Changed account settings".to_string(),
        "EscrowCreate" => "Created a time-locked payment".to_string(),
        "EscrowFinish" => "Released funds from escrow".to_string(),
        "EscrowCancel" => "Cancelled an escrow payment".to_string(),
        "PaymentChannelCreate" => "Opened a payment channel".to_string(),
        "PaymentChannelFund" => "Added funds to a payment channel".to_string(),
        "PaymentChannelClaim" => "Claimed funds from a payment channel".to_string(),
        "CheckCreate" => "Issued a check for later redemption".to_string(),
        "CheckCash" => "Redeemed a check payment".to_string(),
        "CheckCancel" => "Cancelled an outstanding check".to_string(),
        "NFTokenMint" => "Created a new NFT".to_string(),
        "NFTokenBurn" => "Destroyed an NFT".to_string(),
        _ => format!("Executed a {} transaction", tx_type),
    }
}

/// Formats an account address to be more readable
pub fn format_account(account: &str) -> String {
    if account.len() > 12 {
        // Show first 6 and last 4 characters with ellipsis in between for better recognition
        format!("{}.{}", &account[0..6], &account[account.len()-4..])
    } else {
        account.to_string()
    }
}

/// Extracts currency code from a currency string or object
pub fn extract_currency_code(currency_str: &str) -> String {
    // Handle placeholder values professionally
    if currency_str == "N/A" || currency_str == "—" {
        return "—".to_string();
    }
    
    // Check if it's a currency object in JSON format using the globally cached regex
    
    if let Some(caps) = CURRENCY_REGEX.captures(currency_str) {
        return caps.get(1).map_or("—".to_string(), |m| m.as_str().to_string());
    }
    
    // If it's a number, it's XRP
    if currency_str.parse::<f64>().is_ok() {
        return "XRP".to_string();
    }
    
    "—".to_string()
}

/// Calculates price from taker_gets and taker_pays values
pub fn calculate_price(taker_gets: &str, taker_pays: &str) -> Option<f64> {
    // Handle placeholder values
    if taker_gets == "N/A" || taker_pays == "N/A" || taker_gets == "—" || taker_pays == "—" {
        return None;
    }
    
    // Try direct number parsing first (XRP values)
    if let (Ok(gets_num), Ok(pays_num)) = (taker_gets.parse::<f64>(), taker_pays.parse::<f64>()) {
        let gets_value = gets_num / 1_000_000.0; // Convert from drops
        let pays_value = pays_num / 1_000_000.0;
        return Some(pays_value / gets_value);
    }
    
    // Try to extract values from currency objects using the globally cached regex
    
    if let (Some(gets_caps), Some(pays_caps)) = (CURRENCY_REGEX.captures(taker_gets), CURRENCY_REGEX.captures(taker_pays)) {
        let gets_value_str = gets_caps.get(3).map_or("", |m| m.as_str());
        let pays_value_str = pays_caps.get(3).map_or("", |m| m.as_str());
        
        if let (Ok(gets_value), Ok(pays_value)) = (gets_value_str.parse::<f64>(), pays_value_str.parse::<f64>()) {
            return Some(pays_value / gets_value);
        }
    }
    
    None
}

/// Creates a market pair string from taker_gets and taker_pays
pub fn format_market_pair(taker_gets: &str, taker_pays: &str) -> String {
    // Handle placeholder values
    if taker_gets == "N/A" || taker_pays == "N/A" || taker_gets == "—" || taker_pays == "—" {
        return "—".to_string();
    }
    
    let base = extract_currency_code(taker_gets);
    let quote = extract_currency_code(taker_pays);
    
    // If either currency is a placeholder, return a placeholder for the pair
    if base == "—" || quote == "—" {
        return "—".to_string();
    }
    
    format!("{}/{}", base, quote)
}