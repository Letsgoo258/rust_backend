use crate::{
    error::AppError,
    modules::auth::backend::AuthSession,
    modules::sis::dto::{CreateStudentRequest, StudentDto},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/students", post(create_student).get(list_students))
        .route("/guardians", post(create_guardian))
        .route("/students/:id", get(get_student))
}

async fn create_student(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<CreateStudentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let student_id = sqlx::query_scalar!(
        r#"
        INSERT INTO students (school_id, admission_number, first_name, last_name, date_of_birth, gender, blood_group, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE')
        RETURNING id
        "#,
        user.school_id,
        payload.admission_number,
        payload.first_name,
        payload.last_name,
        payload.date_of_birth,
        payload.gender,
        payload.blood_group
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "message": "Student created successfully",
            "student_id": student_id
        })),
    ))
}

async fn list_students(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let students = sqlx::query_as!(
        StudentDto,
        r#"
        SELECT id, school_id, user_id, admission_number, first_name, last_name, date_of_birth, gender, blood_group, status as "status!"
        FROM students
        WHERE school_id = $1
        ORDER BY first_name ASC
        "#,
        user.school_id
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Json(students))
}

async fn get_student(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let student = sqlx::query_as!(
        StudentDto,
        r#"
        SELECT id, school_id, user_id, admission_number, first_name, last_name, date_of_birth, gender, blood_group, status as "status!"
        FROM students
        WHERE id = $1 AND school_id = $2
        "#,
        id,
        user.school_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    match student {
        Some(s) => Ok(Json(s)),
        None => Err(AppError::NotFound("Student not found".into())),
    }
}

async fn create_guardian(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<crate::modules::sis::dto::CreateGuardianRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let guardian_id = sqlx::query_scalar!(
        r#"
        INSERT INTO guardians (school_id, first_name, last_name, email, phone)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
        user.school_id,
        payload.first_name,
        payload.last_name,
        payload.email,
        payload.phone
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "message": "Guardian created successfully",
            "guardian_id": guardian_id
        })),
    ))
}
