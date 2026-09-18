use async_trait::async_trait;
use axum_login::{AuthUser, AuthnBackend, UserId};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow, serde::Serialize)]
pub struct User {
    pub id: Uuid,
    pub school_id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub display_name: Option<String>,
}

impl AuthUser for User {
    type Id = Uuid;
    fn id(&self) -> Self::Id {
        self.id
    }
    fn session_auth_hash(&self) -> &[u8] {
        self.password_hash.as_bytes()
    }
}

#[derive(Clone)]
pub struct Backend {
    pub db: PgPool,
}

impl Backend {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

pub type AuthSession = axum_login::AuthSession<Backend>;
pub type SessionLayer =
    axum_login::AuthManagerLayer<Backend, tower_sessions_sqlx_store::PostgresStore>;

#[derive(Clone)]
pub struct Credentials {
    pub school_code: String,
    pub username: String,
    pub password: String,
}

#[async_trait]
impl AuthnBackend for Backend {
    type User = User;
    type Credentials = Credentials;
    type Error = std::convert::Infallible;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT u.id, u.school_id, u.username, u.password_hash, u.display_name
            FROM users u
            JOIN schools s ON u.school_id = s.id
            WHERE s.code = $1 AND u.username = $2 AND u.status = 'ACTIVE' AND s.status = 'ACTIVE'
            "#,
            creds.school_code,
            creds.username
        )
        .fetch_optional(&self.db)
        .await
        .unwrap_or(None);

        if let Some(user) = user {
            if crate::utils::password::verify_password(&creds.password, &user.password_hash)
                .unwrap_or(false)
            {
                return Ok(Some(user));
            }
        }

        Ok(None)
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT id, school_id, username, password_hash, display_name
            FROM users
            WHERE id = $1 AND status = 'ACTIVE'
            "#,
            user_id
        )
        .fetch_optional(&self.db)
        .await
        .unwrap_or(None);

        Ok(user)
    }
}
