use crate::{
    error::AppError,
    modules::auth::backend::AuthSession,
    modules::auth::{
        backend::Credentials,
        dto::{LoginRequest, SetupRequest, SetupResponse},
    },
    state::AppState,
    utils::password::hash_password,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/setup", post(setup_system))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
}

/// Idempotent setup endpoint to create the very first School and Super Admin.
async fn setup_system(
    State(state): State<AppState>,
    Json(payload): Json<SetupRequest>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Security Check: Only allow if no schools exist
    let school_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM schools")
        .fetch_one(&state.db)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .unwrap_or(0);

    if school_count > 0 {
        return Err(AppError::Forbidden(
            "System is already initialized. Cannot run setup again.".into(),
        ));
    }

    // 2. Hash the admin password securely
    let hashed_password = hash_password(&payload.admin_password)?;

    // 3. Start a database transaction to ensure atomic setup
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    // Create the School
    let school_id = sqlx::query_scalar!(
        "INSERT INTO schools (name, code, status) VALUES ($1, $2, 'ACTIVE') RETURNING id",
        payload.school_name,
        payload.school_code
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Create the Super Admin Role
    let role_id = sqlx::query_scalar!(
        "INSERT INTO roles (school_id, name, code, description, is_system_role) 
         VALUES ($1, 'Super Admin', 'SUPER_ADMIN', 'Full system access', true) RETURNING id",
        school_id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Create the Admin User
    let user_id = sqlx::query_scalar!(
        "INSERT INTO users (school_id, username, email, password_hash, first_name, last_name, status, user_type) 
         VALUES ($1, $2, $3, $4, $5, $6, 'ACTIVE', 'SUPER_ADMIN') RETURNING id",
        school_id,
        payload.admin_username,
        payload.admin_email,
        hashed_password,
        payload.first_name,
        payload.last_name
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Assign Role to User
    sqlx::query!(
        "INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)",
        user_id,
        role_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    tracing::info!(
        "System successfully initialized with school '{}' and admin user '{}'",
        payload.school_name,
        payload.admin_username
    );

    Ok((
        StatusCode::CREATED,
        Json(SetupResponse {
            message: "System successfully initialized.".to_string(),
            school_id,
            user_id,
        }),
    ))
}

async fn login(
    mut auth_session: AuthSession,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let creds = Credentials {
        school_code: payload.school_code,
        username: payload.username,
        password: payload.password,
    };

    let user = match auth_session.authenticate(creds).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err(AppError::Unauthorized(
                "Invalid username or password".into(),
            ))
        }
        Err(_) => {
            return Err(AppError::InternalServerError(
                "Authentication failed".into(),
            ))
        }
    };

    if auth_session.login(&user).await.is_err() {
        return Err(AppError::InternalServerError(
            "Failed to create session".into(),
        ));
    }

    Ok(Json(json!({ "message": "Login successful", "user": user })))
}

async fn logout(mut auth_session: AuthSession) -> Result<impl IntoResponse, AppError> {
    if auth_session.logout().await.is_err() {
        return Err(AppError::InternalServerError("Failed to logout".into()));
    }
    Ok(Json(json!({ "message": "Logged out successfully" })))
}

async fn me(auth_session: AuthSession) -> Result<impl IntoResponse, AppError> {
    match auth_session.user {
        Some(user) => Ok(Json(user)),
        None => Err(AppError::Unauthorized("Not logged in".into())),
    }
}
