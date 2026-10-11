use chrono::Utc;
use eravaya::utils::email::send_email;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let admin_email =
        std::env::var("ALERT_ADMIN_EMAIL").unwrap_or_else(|_| "kabhiram67@gmail.com".to_string());

    let trace_id = Uuid::new_v4().simple().to_string();
    let start_time = Utc::now().to_rfc3339();

    println!("============================================================");
    println!("🧪 RUNNING RUST TEST SUITE (Trace ID: {})", trace_id);
    println!("⏰ Started At: {}", start_time);
    println!("============================================================");

    let mut child = match Command::new("cargo")
        .args(["test", "--", "--nocapture"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            eprintln!("💥 Failed to spawn cargo test: {}", err);
            let subject = format!(
                "🚨 [CRITICAL ALERT] Failed to Spawn Tests | Trace ID: {}",
                trace_id
            );
            let html_body = format!(
                "<h3>Failed to spawn cargo test</h3><p><strong>Trace ID:</strong> {}</p><pre>{}</pre>",
                trace_id, err
            );
            let _ = send_email(&admin_email, &subject, &html_body).await;
            std::process::exit(1);
        }
    };

    let stdout = child.stdout.take().expect("Child stdout missing");
    let stderr = child.stderr.take().expect("Child stderr missing");

    let stdout_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        let mut full_stdout = String::new();
        while let Ok(Some(line)) = reader.next_line().await {
            println!("{}", line);
            full_stdout.push_str(&line);
            full_stdout.push('\n');
        }
        full_stdout
    });

    let stderr_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut full_stderr = String::new();
        while let Ok(Some(line)) = reader.next_line().await {
            eprintln!("{}", line);
            full_stderr.push_str(&line);
            full_stderr.push('\n');
        }
        full_stderr
    });

    let status = match child.wait().await {
        Ok(status) => status,
        Err(err) => {
            eprintln!("💥 Error waiting for cargo test: {}", err);
            std::process::exit(1);
        }
    };

    let stdout_log = stdout_handle.await.unwrap_or_default();
    let stderr_log = stderr_handle.await.unwrap_or_default();

    if status.success() {
        println!("============================================================");
        println!(
            "✅ ALL TEST CASES PASSED SUCCESSFULLY! (Trace ID: {})",
            trace_id
        );
        println!("============================================================");
        std::process::exit(0);
    } else {
        println!("============================================================");
        eprintln!("❌ TEST SUITE FAILED OR DROPPED! (Trace ID: {})", trace_id);
        println!("============================================================");

        let exit_code = status.code().unwrap_or(-1);
        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
  <style>
    body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background-color: #f8fafc; margin: 0; padding: 20px; }}
    .card {{ background: #ffffff; border: 1px solid #e2e8f0; border-radius: 12px; padding: 24px; max-width: 700px; margin: 0 auto; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1); }}
    .badge {{ background: #fee2e2; color: #991b1b; padding: 4px 12px; border-radius: 9999px; font-weight: bold; font-size: 12px; display: inline-block; }}
    .trace {{ background: #1e293b; color: #38bdf8; padding: 8px 12px; border-radius: 6px; font-family: monospace; font-size: 14px; margin: 12px 0; word-break: break-all; }}
    .log-box {{ background: #0f172a; color: #f1f5f9; padding: 16px; border-radius: 8px; font-family: monospace; font-size: 12px; overflow-x: auto; white-space: pre-wrap; max-height: 400px; }}
    .meta {{ font-size: 13px; color: #64748b; margin-bottom: 16px; }}
  </style>
</head>
<body>
  <div class="card">
    <div class="badge">CRITICAL: Test Suite Failure</div>
    <h2 style="color: #0f172a; margin-top: 12px;">Eravaya Backend Automated Test Alert</h2>

    <div class="meta">
      <strong>Timestamp:</strong> {}<br/>
      <strong>Exit Code:</strong> {}<br/>
      <strong>Service:</strong> eravaya-rust-backend
    </div>

    <div><strong>Trace ID:</strong></div>
    <div class="trace">{}</div>

    <h3>Standard Error Output:</h3>
    <div class="log-box">{}</div>

    <h3>Standard Output:</h3>
    <div class="log-box">{}</div>

    <p style="color: #64748b; font-size: 12px; margin-top: 24px; border-top: 1px solid #e2e8f0; padding-top: 12px;">
      This is an automated alert dispatched from the Docker test runner.
    </p>
  </div>
</body>
</html>"#,
            start_time, exit_code, trace_id, stderr_log, stdout_log
        );

        let subject = format!(
            "🚨 [CRITICAL ALERT] Rust Test Failed | Trace ID: {}",
            trace_id
        );

        println!(
            "📧 Sending error email with Trace ID to admin: {}",
            admin_email
        );
        match send_email(&admin_email, &subject, &html_body).await {
            Ok(_) => {
                println!(
                    "✅ Failure notification email successfully dispatched to {}",
                    admin_email
                );
            }
            Err(e) => {
                eprintln!("⚠️ Failed to send error alert email: {}", e);
            }
        }

        std::process::exit(1);
    }
}
