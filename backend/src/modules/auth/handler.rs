use super::backend::{AuthSession, Credentials};
use super::dto::{LoginRequest, LoginResponse, UserDto};
use crate::error::AppError;
use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

pub fn router() -> Router<crate::state::AppState> {
    Router::new()
        .route("/login", post(login_handler))
        .route("/logout", post(logout_handler))
        .route("/me", get(me_handler))
}

async fn login_handler(
    mut auth_session: AuthSession,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let creds = Credentials {
        school_code: payload.school_code,
        username: payload.username,
        password: payload.password,
    };

    let user = auth_session
        .authenticate(creds)
        .await
        .map_err(|_| AppError::Internal("Authentication failed".into()))?;

    if let Some(user) = user {
        auth_session
            .login(&user)
            .await
            .map_err(|_| AppError::Internal("Login failed".into()))?;

        Ok(Json(LoginResponse {
            success: true,
            data: UserDto {
                id: user.id,
                school_id: user.school_id,
                username: user.username,
                display_name: user.display_name,
            },
        }))
    } else {
        Err(AppError::Internal("Invalid credentials".into())) // Will create specific auth error later
    }
}

async fn logout_handler(mut auth_session: AuthSession) -> Result<StatusCode, AppError> {
    auth_session
        .logout()
        .await
        .map_err(|_| AppError::Internal("Logout failed".into()))?;
    Ok(StatusCode::OK)
}

async fn me_handler(auth_session: AuthSession) -> Result<Json<UserDto>, AppError> {
    if let Some(user) = auth_session.user {
        Ok(Json(UserDto {
            id: user.id,
            school_id: user.school_id,
            username: user.username,
            display_name: user.display_name,
        }))
    } else {
        Err(AppError::Internal("Not authenticated".into()))
    }
}
