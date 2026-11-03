use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let db_url = env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await?;

    println!("--- DISTINCT SENDERS AND CHATS ---");
    let rows: Vec<(String, Option<String>, i64, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT sender_jid, payload->>'chat' as chat, count(*) as count, max(received_at) as last_seen
        FROM whatsapp_inbound
        GROUP BY sender_jid, payload->>'chat'
        ORDER BY last_seen DESC
        "#
    ).fetch_all(&pool).await?;

    for (s, chat, count, last_seen) in rows {
        println!(
            "Sender: {} | Chat: {:?} | Count: {} | Last seen: {}",
            s, chat, count, last_seen
        );
    }

    // outbox commented out to see inbounds clearly
    Ok(())
}
