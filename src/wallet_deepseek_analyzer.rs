use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;
use serde_json::Value;
use std::process::Command;

fn main() {
    println!("DeepSeek High-Value Wallet Analyzer\n");
    let mut seen = HashSet::new();
    loop {
        for entry in fs::read_dir(".").unwrap() {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(fname) = path.file_name().and_then(|f| f.to_str()) {
                    if fname.starts_with("deepseek_wallet_") && fname.ends_with(".json") && seen.insert(fname.to_string()) {
                        if let Ok(mut file) = File::open(&path) {
                            let mut contents = String::new();
                            if file.read_to_string(&mut contents).is_ok() {
                                analyze_wallet_with_deepseek(&contents);
                            }
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_secs(60));
    }
}

fn analyze_wallet_with_deepseek(context_json: &str) {
    let parsed: Value = serde_json::from_str(context_json).unwrap_or(Value::Null);
    let wallet = parsed.get("wallet").and_then(|w| w.as_str()).unwrap_or("");
    let account_info = parsed.get("account_info").unwrap_or(&Value::Null);
    let connected_wallets = parsed.get("connected_wallets").unwrap_or(&Value::Null);

    let prompt = format!(
        "You are a blockchain intelligence analyst.\n\
New high value wallet detected!\n\
Wallet: {}\n\
Account info: {}\n\
Connected high-value wallets: {}\n\
Please provide a concise, human-readable report with:\n\
- The wallet's balance and timestamp\n\
- A remark about the wallet's likely role (whale, institutional, etc.)\n\
- Any notable patterns or interconnections with other big wallets\n\
Format your answer as:\n\
Balance: ... (timestamp)\n\
Remarks: ...\n",
        wallet,
        serde_json::to_string_pretty(account_info).unwrap_or_default(),
        serde_json::to_string_pretty(connected_wallets).unwrap_or_default(),
    );

    println!("\n[DeepSeek Analysis for {}]\nPrompt size: {} bytes\n", wallet, prompt.len());
    let output = Command::new("ollama")
        .args(["run", "deepseek-r1:14b", &prompt])
        .output();
    match output {
        Ok(out) => {
            let insight = String::from_utf8_lossy(&out.stdout);
            let report = format!(
                "{}\n{}\n",
                "-".repeat(60),
                insight.trim()
            );
            println!("{}", report);
            // Append to log file
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open("deepseek_wallet_reports.log")
            {
                let _ = writeln!(file, "{}", report);
            }
        }
        Err(e) => {
            println!("Failed to run DeepSeek for wallet {}: {}", wallet, e);
        }
    }
} 