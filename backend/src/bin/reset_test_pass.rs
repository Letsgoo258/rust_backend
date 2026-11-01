use eravaya::config::AppConfig;
use eravaya::db;
use eravaya::utils::password::{hash_password, verify_password};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env();
    let pool = db::init_pool(&config).await?;

    let new_pass = "hello123!";
    let new_hash = hash_password(new_pass)?;

    // 1. Reset Super Admin (kabhiram67 / kabhiram67@gmail.com)
    let res_admin = sqlx::query!(
        r#"
        UPDATE users 
        SET password_hash = $1, status = 'ACTIVE', email_verified = true, phone_verified = true 
        WHERE email = 'kabhiram67@gmail.com' OR username = 'kabhiram67'
        RETURNING id, username, email, password_hash
        "#,
        new_hash
    )
    .fetch_all(&pool)
    .await?;

    println!("--- Super Admin Updates ---");
    for user in res_admin {
        println!(
            "User updated: id={}, username={}, email={}",
            user.id,
            user.username,
            user.email.unwrap_or_default()
        );
        let verified = verify_password(new_pass, &user.password_hash)?;
        println!("Password verification for '{}': {}", new_pass, verified);
    }

    // 2. Reset test101
    let res_test = sqlx::query!(
        r#"
        UPDATE users 
        SET password_hash = $1, status = 'ACTIVE', email_verified = true, phone_verified = true 
        WHERE username = 'test101' OR email = 'erayaerp@gmail.com'
        RETURNING id, username, email, password_hash
        "#,
        new_hash
    )
    .fetch_all(&pool)
    .await?;

    println!("\n--- Test101 Updates ---");
    for user in res_test {
        println!(
            "User updated: id={}, username={}, email={}",
            user.id,
            user.username,
            user.email.unwrap_or_default()
        );
        let verified = verify_password(new_pass, &user.password_hash)?;
        println!("Password verification for '{}': {}", new_pass, verified);
    }

    Ok(())
}
