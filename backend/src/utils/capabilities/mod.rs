pub mod catalogue;
pub mod defaults;
pub mod dto;

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::collections::HashSet;
use uuid::Uuid;

use crate::error::AppError;

#[allow(unused_imports)]
pub use catalogue::catalogue;
#[allow(unused_imports)]
pub use defaults::default_enabled_keys;
pub use dto::{
    CapabilityItem, ConfirmGrantPayload, RequestGrantOtpPayload, RoleCapabilitiesDto,
    RoleCapabilityOverride, UserCapabilityGrantDto, UserSearchItem,
};

pub struct CapabilityController;

impl CapabilityController {
    /// Ensure the role_capabilities and user_capability_grants tables exist dynamically at runtime.
    pub async fn ensure_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS role_capabilities (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                role_code VARCHAR(100) NOT NULL,
                capability_key VARCHAR(100) NOT NULL,
                is_enabled BOOLEAN NOT NULL DEFAULT true,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE(school_id, role_code, capability_key)
            );

            CREATE INDEX IF NOT EXISTS idx_role_capabilities_school_upper_role
                ON role_capabilities (school_id, UPPER(role_code));
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_capability_grants (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                capability_key VARCHAR(100) NOT NULL,
                granted_by UUID REFERENCES users(id) ON DELETE SET NULL,
                reason VARCHAR(255),
                expires_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_user_cap_grants_lookup
                ON user_capability_grants(school_id, user_id, capability_key);
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Master catalogue accessor
    pub fn catalogue() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
        catalogue::catalogue()
    }

    /// Default keys accessor
    pub fn default_enabled_keys(role_code: &str) -> HashSet<&'static str> {
        defaults::default_enabled_keys(role_code)
    }

    /// Efficiently resolve all effective capabilities for a user given their assigned roles and any active temporary grants in a school.
    /// This resolves custom overrides from `role_capabilities`, active `user_capability_grants`, and defaults in fast queries.
    pub async fn resolve_user_capabilities(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Option<Uuid>,
        roles: &[String],
    ) -> Vec<String> {
        let is_admin = roles.is_empty()
            || roles.iter().any(|r| {
                let r_upper = r.to_uppercase();
                r_upper == "SUPER_ADMIN"
                    || r_upper == "SCHOOL_ADMIN"
                    || r_upper == "HEADMASTER"
                    || r_upper == "PRINCIPAL"
                    || r_upper.contains("ADMIN")
            });

        if is_admin && roles.iter().any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN")) {
            return Self::catalogue()
                .into_iter()
                .map(|(k, _, _, _)| k.to_string())
                .collect();
        }

        // 1. Start with defaults for all user's roles
        let mut enabled_set = HashSet::new();
        if roles.is_empty() || is_admin {
            for (k, _, _, _) in Self::catalogue() {
                enabled_set.insert(k.to_string());
            }
        } else {
            for r in roles {
                for k in Self::default_enabled_keys(r) {
                    enabled_set.insert(k.to_string());
                }
            }
        }

        // 2. Fetch all custom role overrides for this school and the user's roles in one single query
        let role_codes_upper: Vec<String> = roles.iter().map(|r| r.to_uppercase()).collect();
        let rows = sqlx::query(
            r#"
            SELECT UPPER(role_code) as role_code, capability_key, is_enabled
            FROM role_capabilities
            WHERE school_id = $1 AND UPPER(role_code) = ANY($2)
            "#,
        )
        .bind(school_id)
        .bind(&role_codes_upper)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for r in rows {
            let cap_key: String = r.try_get("capability_key").unwrap_or_default();
            let is_enabled: bool = r.try_get("is_enabled").unwrap_or(true);

            if is_enabled {
                enabled_set.insert(cap_key);
            } else {
                enabled_set.remove(&cap_key);
            }
        }

        // 3. Fetch active unexpired individual capability grants for this specific user
        if let Some(uid) = user_id {
            let user_grant_rows = sqlx::query(
                r#"
                SELECT capability_key
                FROM user_capability_grants
                WHERE school_id = $1 AND user_id = $2
                  AND (expires_at IS NULL OR expires_at > NOW())
                "#,
            )
            .bind(school_id)
            .bind(uid)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            for r in user_grant_rows {
                let cap_key: String = r.try_get("capability_key").unwrap_or_default();
                if !cap_key.is_empty() {
                    enabled_set.insert(cap_key);
                }
            }
        }

        enabled_set.into_iter().collect()
    }

    /// Check if a specific user/role in a school has a capability key enabled.
    #[allow(dead_code)]
    pub async fn has_capability(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Option<Uuid>,
        roles: &[String],
        capability_key: &str,
    ) -> Result<bool, AppError> {
        let caps = Self::resolve_user_capabilities(pool, school_id, user_id, roles).await;
        Ok(caps.iter().any(|c| c.eq_ignore_ascii_case(capability_key)))
    }

    /// Search users in school for individual capability / role delegation autocomplete.
    pub async fn search_school_users(
        pool: &PgPool,
        school_id: Uuid,
        query: &str,
    ) -> Result<Vec<UserSearchItem>, AppError> {
        let pattern = format!("%{}%", query.trim());
        let rows = sqlx::query(
            r#"
            SELECT
                u.id,
                u.username,
                u.display_name,
                u.email,
                COALESCE(array_agg(r.name) FILTER (WHERE r.name IS NOT NULL), '{}') as roles
            FROM users u
            LEFT JOIN user_roles ur ON ur.user_id = u.id
            LEFT JOIN roles r ON r.id = ur.role_id
            WHERE u.school_id = $1
              AND (
                  u.display_name ILIKE $2 OR
                  u.username ILIKE $2 OR
                  u.email ILIKE $2
              )
            GROUP BY u.id, u.username, u.display_name, u.email
            ORDER BY u.display_name ASC
            LIMIT 25
            "#,
        )
        .bind(school_id)
        .bind(&pattern)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;

        let mut items = Vec::new();
        for r in rows {
            let id: Uuid = r.try_get("id").unwrap_or_default();
            let username: String = r.try_get("username").unwrap_or_default();
            let display_name: String = r.try_get("display_name").unwrap_or_default();
            let email: Option<String> = r.try_get("email").ok();
            let roles: Vec<String> = r.try_get("roles").unwrap_or_default();

            items.push(UserSearchItem {
                id,
                username,
                display_name,
                email,
                roles,
            });
        }

        Ok(items)
    }

    /// Retrieve full capabilities matrix for all roles configured in a school.
    pub async fn get_all_roles_matrix(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<Vec<RoleCapabilitiesDto>, AppError> {
        let _ = Self::ensure_schema(pool).await;

        let mut role_rows = sqlx::query(
            r#"
            SELECT code, name, is_system_role
            FROM roles
            WHERE school_id = $1
            ORDER BY is_system_role DESC, name ASC
            "#,
        )
        .bind(school_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        if role_rows.is_empty() {
            if let Ok(mut conn) = pool.acquire().await {
                let _ = crate::utils::roles::seed_all_school_roles(&mut *conn, school_id).await;
            }
            role_rows = sqlx::query(
                r#"
                SELECT code, name, is_system_role
                FROM roles
                WHERE school_id = $1
                ORDER BY is_system_role DESC, name ASC
                "#,
            )
            .bind(school_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        }

        let custom_rows = sqlx::query(
            r#"
            SELECT role_code, capability_key, is_enabled
            FROM role_capabilities
            WHERE school_id = $1
            "#,
        )
        .bind(school_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let custom_overrides: Vec<RoleCapabilityOverride> = custom_rows
            .into_iter()
            .map(|r| RoleCapabilityOverride {
                role_code: r.try_get("role_code").unwrap_or_default(),
                capability_key: r.try_get("capability_key").unwrap_or_default(),
                is_enabled: r.try_get("is_enabled").unwrap_or(true),
            })
            .collect();

        let mut matrix = Vec::new();
        let cat = Self::catalogue();

        if !role_rows.is_empty() {
            for r in role_rows {
                let role_code: String = r.try_get("code").unwrap_or_default();
                let role_name: String = r.try_get("name").unwrap_or_default();
                let is_system: bool = r.try_get("is_system_role").unwrap_or(true);
                let defaults = Self::default_enabled_keys(&role_code);

                let mut caps = Vec::new();
                for (key, name, category, desc) in &cat {
                    let override_val = custom_overrides.iter().find(|o| {
                        o.role_code.eq_ignore_ascii_case(&role_code) && o.capability_key == *key
                    });

                    let is_enabled = match override_val {
                        Some(o) => o.is_enabled,
                        None => defaults.contains(key),
                    };

                    caps.push(CapabilityItem {
                        key: key.to_string(),
                        name: name.to_string(),
                        category: category.to_string(),
                        description: desc.to_string(),
                        is_enabled,
                    });
                }

                matrix.push(RoleCapabilitiesDto {
                    role_code,
                    role_name,
                    is_system_role: is_system,
                    capabilities: caps,
                });
            }
        }

        Ok(matrix)
    }

    /// Retrieve all active and recent individual user capability grants for a school.
    pub async fn get_school_user_grants(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<Vec<UserCapabilityGrantDto>, AppError> {
        let _ = Self::ensure_schema(pool).await;

        let rows = sqlx::query(
            r#"
            SELECT
                g.id,
                g.school_id,
                g.user_id,
                u.display_name as user_name,
                u.username as user_username,
                COALESCE(array_agg(r.name) FILTER (WHERE r.name IS NOT NULL), '{}') as user_roles,
                g.capability_key,
                admin.display_name as granted_by_name,
                g.reason,
                g.expires_at,
                g.created_at,
                (g.expires_at IS NULL OR g.expires_at > NOW()) as is_active
            FROM user_capability_grants g
            JOIN users u ON u.id = g.user_id
            LEFT JOIN users admin ON admin.id = g.granted_by
            LEFT JOIN user_roles ur ON ur.user_id = u.id
            LEFT JOIN roles r ON r.id = ur.role_id
            WHERE g.school_id = $1
            GROUP BY g.id, g.school_id, g.user_id, u.display_name, u.username, g.capability_key, admin.display_name, g.reason, g.expires_at, g.created_at
            ORDER BY is_active DESC, g.created_at DESC
            LIMIT 100
            "#
        )
        .bind(school_id)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;

        let cat = Self::catalogue();
        let now = Utc::now();

        let mut list = Vec::new();
        for r in rows {
            let id: Uuid = r.try_get("id").unwrap_or_default();
            let school_id: Uuid = r.try_get("school_id").unwrap_or_default();
            let user_id: Uuid = r.try_get("user_id").unwrap_or_default();
            let user_name: String = r.try_get("user_name").unwrap_or_default();
            let user_username: String = r.try_get("user_username").unwrap_or_default();
            let user_roles: Vec<String> = r.try_get("user_roles").unwrap_or_default();
            let capability_key: String = r.try_get("capability_key").unwrap_or_default();
            let granted_by_name: Option<String> = r.try_get("granted_by_name").ok();
            let reason: Option<String> = r.try_get("reason").ok();
            let expires_at_opt: Option<DateTime<Utc>> = r.try_get("expires_at").ok();
            let created_at_dt: DateTime<Utc> =
                r.try_get("created_at").unwrap_or_else(|_| Utc::now());
            let is_active: bool = r.try_get("is_active").unwrap_or(false);

            let (cap_name, cap_cat) = cat
                .iter()
                .find(|(k, _, _, _)| *k == capability_key.as_str())
                .map(|(_, n, c, _)| (n.to_string(), c.to_string()))
                .unwrap_or_else(|| (capability_key.clone(), "Special Permissions".to_string()));

            let duration_label = match expires_at_opt {
                None => "Permanent (Forever)".to_string(),
                Some(exp) => {
                    if exp <= now {
                        "Expired".to_string()
                    } else {
                        let diff = exp - now;
                        let hours = diff.num_hours();
                        let mins = diff.num_minutes() % 60;
                        if hours > 24 {
                            let days = diff.num_days();
                            format!("Expires in {}d {}h", days, hours % 24)
                        } else if hours > 0 {
                            format!("Expires in {}h {}m", hours, mins)
                        } else {
                            format!("Expires in {}m", mins.max(1))
                        }
                    }
                }
            };

            list.push(UserCapabilityGrantDto {
                id,
                school_id,
                user_id,
                user_name,
                user_username,
                user_roles,
                capability_key,
                capability_name: cap_name,
                capability_category: cap_cat,
                granted_by_name,
                reason,
                expires_at: expires_at_opt.map(|dt| dt.to_rfc3339()),
                is_active,
                duration_label,
                created_at: created_at_dt.to_rfc3339(),
            });
        }

        Ok(list)
    }

    /// Grant an individual user a temporary or permanent capability.
    pub async fn grant_user_capability(
        pool: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        capability_key: &str,
        granted_by: Uuid,
        reason: Option<String>,
        duration: &str,
        custom_expires_at: Option<&str>,
    ) -> Result<UserCapabilityGrantDto, AppError> {
        let _ = Self::ensure_schema(pool).await;

        let expires_at: Option<DateTime<Utc>> = match duration.to_lowercase().as_str() {
            "1_day" | "24_hours" | "1day" => Some(Utc::now() + chrono::Duration::hours(24)),
            "2_days" | "48_hours" | "2days" => Some(Utc::now() + chrono::Duration::hours(48)),
            "1_week" | "7_days" | "1week" => Some(Utc::now() + chrono::Duration::days(7)),
            "forever" | "permanent" => None,
            _ => {
                if let Some(custom_str) = custom_expires_at {
                    chrono::DateTime::parse_from_rfc3339(custom_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                } else {
                    None
                }
            }
        };

        let row = sqlx::query(
            r#"
            INSERT INTO user_capability_grants
                (school_id, user_id, capability_key, granted_by, reason, expires_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            RETURNING id, created_at
            "#
        )
        .bind(school_id)
        .bind(user_id)
        .bind(capability_key)
        .bind(granted_by)
        .bind(reason.as_deref())
        .bind(expires_at)
        .fetch_one(pool)
        .await
        .map_err(AppError::Database)?;

        let grant_id: Uuid = row.try_get("id").unwrap_or_default();
        let created_at: DateTime<Utc> = row.try_get("created_at").unwrap_or_else(|_| Utc::now());

        crate::modules::auth::backend::invalidate_user_cache(&user_id);

        // Fetch user & admin metadata for response
        let user_meta = sqlx::query(
            r#"
            SELECT
                u.display_name as user_name,
                u.username as user_username,
                COALESCE(array_agg(r.name) FILTER (WHERE r.name IS NOT NULL), '{}') as user_roles
            FROM users u
            LEFT JOIN user_roles ur ON ur.user_id = u.id
            LEFT JOIN roles r ON r.id = ur.role_id
            WHERE u.id = $2
            GROUP BY u.id, u.display_name, u.username
            "#,
        )
        .bind(school_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)?;

        let (user_name, user_username, user_roles) = match user_meta {
            Some(r) => (
                r.try_get("user_name").unwrap_or_default(),
                r.try_get("user_username").unwrap_or_default(),
                r.try_get("user_roles").unwrap_or_default(),
            ),
            None => ("User".to_string(), "".to_string(), vec![]),
        };

        let admin_name: Option<String> =
            sqlx::query_scalar!("SELECT display_name FROM users WHERE id = $1", granted_by)
                .fetch_optional(pool)
                .await
                .unwrap_or(None)
                .flatten();

        let cat = Self::catalogue();
        let (cap_name, cap_cat) = cat
            .iter()
            .find(|(k, _, _, _)| *k == capability_key)
            .map(|(_, n, c, _)| (n.to_string(), c.to_string()))
            .unwrap_or_else(|| {
                (
                    capability_key.to_string(),
                    "Special Permissions".to_string(),
                )
            });

        let duration_label = match expires_at {
            None => "Permanent (Forever)".to_string(),
            Some(exp) => {
                let diff = exp - Utc::now();
                let hours = diff.num_hours();
                let mins = diff.num_minutes() % 60;
                if hours > 24 {
                    format!("Expires in {}d {}h", diff.num_days(), hours % 24)
                } else if hours > 0 {
                    format!("Expires in {}h {}m", hours, mins)
                } else {
                    format!("Expires in {}m", mins.max(1))
                }
            }
        };

        Ok(UserCapabilityGrantDto {
            id: grant_id,
            school_id,
            user_id,
            user_name,
            user_username,
            user_roles,
            capability_key: capability_key.to_string(),
            capability_name: cap_name,
            capability_category: cap_cat,
            granted_by_name: admin_name,
            reason,
            expires_at: expires_at.map(|dt| dt.to_rfc3339()),
            is_active: true,
            duration_label,
            created_at: created_at.to_rfc3339(),
        })
    }

    /// Revoke an active user capability grant.
    pub async fn revoke_user_capability_grant(
        pool: &PgPool,
        school_id: Uuid,
        grant_id: Uuid,
    ) -> Result<(), AppError> {
        let result =
            sqlx::query("DELETE FROM user_capability_grants WHERE id = $1 AND school_id = $2")
                .bind(grant_id)
                .bind(school_id)
                .execute(pool)
                .await
                .map_err(AppError::Database)?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Capability grant not found".into()));
        }

        Ok(())
    }

    /// Update or save capabilities for a role in a school.
    pub async fn update_role_capabilities(
        pool: &PgPool,
        school_id: Uuid,
        role_code: &str,
        capabilities: Vec<CapabilityItem>,
    ) -> Result<(), AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        for cap in capabilities {
            sqlx::query(
                r#"
                INSERT INTO role_capabilities (school_id, role_code, capability_key, is_enabled, updated_at)
                VALUES ($1, $2, $3, $4, NOW())
                ON CONFLICT (school_id, role_code, capability_key)
                DO UPDATE SET is_enabled = EXCLUDED.is_enabled, updated_at = NOW()
                "#
            )
            .bind(school_id)
            .bind(role_code.to_uppercase())
            .bind(&cap.key)
            .bind(cap.is_enabled)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        }

        tx.commit().await.map_err(AppError::Database)?;
        Ok(())
    }
}
