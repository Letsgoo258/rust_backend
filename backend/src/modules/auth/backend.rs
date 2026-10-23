use async_trait::async_trait;
use axum_login::{AuthUser, AuthnBackend, UserId};
use dashmap::DashMap;
use sqlx::PgPool;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use uuid::Uuid;

static USER_CACHE: LazyLock<DashMap<Uuid, (User, Instant)>> = LazyLock::new(DashMap::new);
const USER_CACHE_TTL: Duration = Duration::from_secs(60);

pub fn invalidate_user_cache(user_id: &Uuid) {
    USER_CACHE.remove(user_id);
}

#[allow(dead_code)]
pub fn clear_user_cache() {
    USER_CACHE.clear();
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub id: Uuid,
    pub school_id: Uuid,
    pub username: String,
    #[serde(skip_serializing, default)]
    pub password_hash: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub email_verified: bool,
    pub phone_verified: bool,
    pub school_status: String,
    pub school_operable: bool,
    pub school_onboarding_step: i32,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct UserMeResponse {
    pub id: Uuid,
    pub school_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub roles: Vec<String>,
    pub capabilities: Vec<String>,
    pub navigation: crate::config::navigation::NavigationTree,
    pub is_platform_admin: bool,
    pub is_tenant_admin: bool,
    pub email_verified: bool,
    pub phone_verified: bool,
    pub school_status: String,
    pub school_operable: bool,
    pub school_onboarding_step: i32,
    pub url_signature: Option<crate::utils::url_signer::DashboardSignature>,
}

impl User {
    /// Instant O(1) in-memory check without any database queries or compute overhead.
    pub fn has_capability(&self, key: &str) -> bool {
        if self
            .roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN"))
        {
            return true;
        }
        self.capabilities
            .iter()
            .any(|c| c.eq_ignore_ascii_case(key))
    }

    /// Check if user has any of the requested capabilities.
    #[allow(dead_code)]
    pub fn has_any_capability(&self, keys: &[&str]) -> bool {
        if self
            .roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN"))
        {
            return true;
        }
        keys.iter().any(|k| self.has_capability(k))
    }

    /// Fast in-memory check if user has school management privileges.
    pub fn is_school_admin(&self) -> bool {
        self.roles.iter().any(|r| {
            let r_upper = r.to_uppercase();
            r_upper == "SCHOOL_ADMIN" || r_upper == "HEADMASTER" || r_upper == "PRINCIPAL"
        })
    }

    pub fn to_me_response(&self) -> UserMeResponse {
        let nav = crate::config::navigation::NavigationConfig::build_user_navigation(self);

        UserMeResponse {
            id: self.id,
            school_id: self.school_id,
            username: self.username.clone(),
            display_name: self.display_name.clone(),
            roles: self.roles.clone(),
            capabilities: self.capabilities.clone(),
            navigation: nav,
            is_platform_admin: self
                .roles
                .iter()
                .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN")),
            is_tenant_admin: self.is_school_admin(),
            email_verified: self.email_verified,
            phone_verified: self.phone_verified,
            school_status: self.school_status.clone(),
            school_operable: self.school_operable,
            school_onboarding_step: self.school_onboarding_step,
            url_signature: Some(crate::utils::url_signer::UrlSigner::sign_dashboard_session(
                self.id,
                self.school_id,
            )),
        }
    }
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

    pub async fn get_user_by_id(pool: &PgPool, user_id: Uuid) -> Option<User> {
        // Fast in-memory cache lookup
        if let Some(entry) = USER_CACHE.get(&user_id) {
            let (ref user, ref cached_at) = *entry;
            if cached_at.elapsed() < USER_CACHE_TTL {
                return Some(user.clone());
            }
        }
        USER_CACHE.remove(&user_id);

        let row = sqlx::query_as::<_, DbUserRow>(
            r#"
            SELECT
                u.id,
                u.school_id,
                u.username,
                u.password_hash,
                u.display_name,
                COALESCE(array_agg(DISTINCT r.code::TEXT) FILTER (WHERE r.code IS NOT NULL), ARRAY[]::TEXT[]) as roles,
                u.email_verified,
                u.phone_verified,
                s.status::TEXT as school_status,
                COALESCE(s.operable, false) as school_operable,
                COALESCE(s.onboarding_step, 1) as school_onboarding_step
            FROM users u
            JOIN schools s ON u.school_id = s.id
            LEFT JOIN user_roles ur ON ur.user_id = u.id
            LEFT JOIN roles r ON ur.role_id = r.id
            WHERE u.id = $1 AND u.status = 'ACTIVE'
            GROUP BY u.id, u.school_id, u.username, u.password_hash, u.display_name, u.email_verified, u.phone_verified, s.status, s.operable, s.onboarding_step
            "#
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if let Some(row) = row {
            let caps = crate::utils::capabilities::CapabilityController::resolve_user_capabilities(
                pool,
                row.school_id,
                Some(row.id),
                &row.roles,
            )
            .await;

            let user = User {
                id: row.id,
                school_id: row.school_id,
                username: row.username,
                password_hash: row.password_hash,
                display_name: row.display_name,
                roles: row.roles,
                capabilities: caps,
                email_verified: row.email_verified,
                phone_verified: row.phone_verified,
                school_status: row.school_status,
                school_operable: row.school_operable,
                school_onboarding_step: row.school_onboarding_step,
            };

            USER_CACHE.insert(user_id, (user.clone(), Instant::now()));
            return Some(user);
        }

        None
    }
}

pub type AuthSession = axum_login::AuthSession<Backend>;
pub type SessionLayer =
    axum_login::AuthManagerLayer<Backend, tower_sessions_sqlx_store::PostgresStore>;

#[derive(Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(sqlx::FromRow)]
struct DbUserRow {
    id: Uuid,
    school_id: Uuid,
    username: String,
    password_hash: String,
    display_name: Option<String>,
    roles: Vec<String>,
    email_verified: bool,
    phone_verified: bool,
    school_status: String,
    school_operable: bool,
    school_onboarding_step: i32,
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
        let row = sqlx::query_as::<_, DbUserRow>(
            r#"
            SELECT
                u.id,
                u.school_id,
                u.username,
                u.password_hash,
                u.display_name,
                COALESCE(array_agg(DISTINCT r.code::TEXT) FILTER (WHERE r.code IS NOT NULL), ARRAY[]::TEXT[]) as roles,
                u.email_verified,
                u.phone_verified,
                s.status::TEXT as school_status,
                COALESCE(s.operable, false) as school_operable,
                COALESCE(s.onboarding_step, 1) as school_onboarding_step
            FROM users u
            JOIN schools s ON u.school_id = s.id
            LEFT JOIN user_roles ur ON ur.user_id = u.id
            LEFT JOIN roles r ON ur.role_id = r.id
            WHERE (u.username = $1 OR u.email = $1) AND u.status = 'ACTIVE' AND s.status IN ('ACTIVE', 'PENDING_SETUP')
            GROUP BY u.id, u.school_id, u.username, u.password_hash, u.display_name, u.email_verified, u.phone_verified, s.status, s.operable, s.onboarding_step
            LIMIT 1
            "#
        )
        .bind(&creds.username)
        .fetch_optional(&self.db)
        .await
        .unwrap_or(None);

        if let Some(row) = row {
            if crate::utils::password::verify_password(&creds.password, &row.password_hash)
                .unwrap_or(false)
            {
                let caps =
                    crate::utils::capabilities::CapabilityController::resolve_user_capabilities(
                        &self.db,
                        row.school_id,
                        Some(row.id),
                        &row.roles,
                    )
                    .await;

                let user = User {
                    id: row.id,
                    school_id: row.school_id,
                    username: row.username,
                    password_hash: row.password_hash,
                    display_name: row.display_name,
                    roles: row.roles,
                    capabilities: caps,
                    email_verified: row.email_verified,
                    phone_verified: row.phone_verified,
                    school_status: row.school_status,
                    school_operable: row.school_operable,
                    school_onboarding_step: row.school_onboarding_step,
                };

                USER_CACHE.insert(user.id, (user.clone(), Instant::now()));
                return Ok(Some(user));
            }
        }

        Ok(None)
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        Ok(Self::get_user_by_id(&self.db, *user_id).await)
    }
}
