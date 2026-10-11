use eravaya::config::AppConfig;
use eravaya::db;
use eravaya::utils::password::{hash_password, verify_password};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env();
    let pool = db::init_pool(&config).await?;

    let email = "kabhiram67@gmail.com";
    let new_pass = "password123";
    let new_hash = hash_password(new_pass)?;

    // Update password
    let res = sqlx::query!(
        "UPDATE users SET password_hash = $1, status = 'ACTIVE', email_verified = true, phone_verified = true WHERE email = $2 RETURNING id, username, password_hash",
        new_hash,
        email
    )
    .fetch_optional(&pool)
    .await?;

    match res {
        Some(user) => {
            println!("User found and password updated!");
            println!("User ID: {}", user.id);
            println!("Username: {}", user.username);
            let verified = verify_password(new_pass, &user.password_hash)?;
            println!(
                "Password verification test for '{}': {}",
                new_pass, verified
            );
        }
        None => {
            println!("No user found with email: {}", email);
        }
    }

    Ok(())
}
