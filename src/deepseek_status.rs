use std::fs;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    let json_path = "recent_transactions.json";
    println!("DeepSeek Brain: Running\n");
    loop {
        // Read the recent transactions JSON
        let tx_data = match fs::read_to_string(json_path) {
            Ok(data) => data,
            Err(_) => {
                println!("No transaction data available yet.");
                thread::sleep(Duration::from_secs(10));
                continue;
            }
        };
        // Compose the prompt for DeepSeek
        let prompt = format!(
            "Study the following blockchain transactions and generate insights: {}",
            tx_data
        );
        // Call DeepSeek via Ollama
        let output = Command::new("ollama")
            .args(["run", "deepseek-r1:14b", &prompt])
            .output();
        match output {
            Ok(out) => {
                let insight = String::from_utf8_lossy(&out.stdout);
                println!("\n[DeepSeek Insights @ {:?}]:\n{}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), insight);
            }
            Err(e) => {
                println!("Failed to run DeepSeek: {}", e);
            }
        }
        thread::sleep(Duration::from_secs(10));
    }
} 