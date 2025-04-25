use std::collections::{HashSet, HashMap};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;
use std::thread;
use tungstenite::{connect, Message};
use url::Url;
use serde_json::Value;

fn main() {
    println!("High-Value Wallet Details Monitor\n");
    let mut seen = HashSet::new();
    let wallet_connections = load_wallet_connections();
    loop {
        if let Ok(file) = File::open("high_value_wallets.txt") {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(wallet) = line {
                    if seen.insert(wallet.clone()) {
                        match query_wallet(&wallet) {
                            Ok(details) => {
                                let connections = wallet_connections.get(&wallet).cloned().unwrap_or_default();
                                print_wallet_details(&wallet, &details, &connections);
                                write_deepseek_context(&wallet, &details, &connections);
                            },
                            Err(e) => println!("\nWallet: {}\nError: {}\n", wallet, e),
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_secs(10));
    }
}

fn load_wallet_connections() -> HashMap<String, HashSet<String>> {
    if let Ok(file) = File::open("wallet_connections.json") {
        if let Ok(map) = serde_json::from_reader::<_, HashMap<String, HashSet<String>>>(file) {
            return map;
        }
    }
    HashMap::new()
}

fn query_wallet(wallet: &str) -> Result<String, String> {
    let (mut socket, _response) = connect(Url::parse("wss://s1.ripple.com").unwrap())
        .map_err(|e| format!("WebSocket connect error: {}", e))?;
    let req = format!(
        r#"{{"id":1,"command":"account_info","account":"{}","strict":true}}"#,
        wallet
    );
    socket.send(Message::Text(req)).map_err(|e| format!("Send error: {}", e))?;
    let msg = socket.read().map_err(|e| format!("Read error: {}", e))?;
    Ok(msg.to_string())
}

fn print_wallet_details(wallet: &str, details: &str, connections: &HashSet<String>) {
    let parsed: Value = match serde_json::from_str(details) {
        Ok(val) => val,
        Err(_) => {
            println!("\nWallet: {}\nInvalid JSON response\n", wallet);
            return;
        }
    };

    let account_data = parsed.get("result").and_then(|r| r.get("account_data"));
    let warnings = parsed.get("result").and_then(|r| r.get("warnings"));
    let status = parsed.get("status").and_then(|s| s.as_str()).unwrap_or("");
    let validated = parsed.get("result").and_then(|r| r.get("validated")).and_then(|v| v.as_bool());

    println!("\n==============================");
    println!("Wallet: {}", wallet);
    println!("Status: {}{}", status, if validated == Some(true) { " (validated)" } else { "" });
    if let Some(data) = account_data {
        for (k, v) in data.as_object().unwrap() {
            match k.as_str() {
                "Balance" => {
                    let drops = v.as_str().and_then(|b| b.parse::<u64>().ok()).unwrap_or(0);
                    let xrp = drops as f64 / 1_000_000.0;
                    println!("  Balance: {:>20} drops ({:>20.6} XRP)", format_number(drops), xrp);
                },
                _ => {
                    println!("  {:<20}: {}", k, pretty_json_value(v, 2));
                }
            }
        }
    } else {
        println!("  No account data found.");
    }
    if !connections.is_empty() {
        println!("  Connected high-value wallets:");
        for c in connections {
            println!("    - {}", c);
        }
    }
    if let Some(warns) = warnings {
        println!("  Warnings:");
        for w in warns.as_array().unwrap_or(&vec![]) {
            if let Some(msg) = w.get("message").and_then(|m| m.as_str()) {
                println!("    - {}", msg);
            }
        }
    }
    println!("==============================\n");
}

fn write_deepseek_context(wallet: &str, details: &str, connections: &HashSet<String>) {
    let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(format!("deepseek_wallet_{}.json", wallet)).unwrap();
    let context = serde_json::json!({
        "wallet": wallet,
        "account_info": serde_json::from_str::<Value>(details).unwrap_or(Value::Null),
        "connected_wallets": connections,
        // Optionally, add recent transactions if available
    });
    writeln!(file, "{}", serde_json::to_string_pretty(&context).unwrap()).unwrap();
}

fn pretty_json_value(v: &Value, indent: usize) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(arr) => {
            let mut s = String::new();
            for item in arr {
                s.push_str(&format!("\n{:indent$}- {}", "", pretty_json_value(item, indent + 2), indent = indent));
            }
            s
        },
        Value::Object(obj) => {
            let mut s = String::new();
            for (k, v) in obj {
                s.push_str(&format!("\n{:indent$}{}: {}", "", k, pretty_json_value(v, indent + 2), indent = indent));
            }
            s
        },
        Value::Null => "null".to_string(),
    }
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    let mut count = 0;
    for c in s.chars().rev() {
        if count != 0 && count % 3 == 0 {
            out.push(',');
        }
        out.push(c);
        count += 1;
    }
    out.chars().rev().collect()
} 