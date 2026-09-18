use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub port: u16,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            port: env::var("APP_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("APP_PORT must be a valid port number"),
        }
    }
}
