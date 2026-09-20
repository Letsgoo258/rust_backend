use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};

#[derive(Deserialize)]
pub struct CreateAdmissionRequest {
    pub student_first_name: String,
    pub student_last_name: String,
    pub date_of_birth: String, // YYYY-MM-DD
    pub guardian_first_name: String,
    pub guardian_last_name: String,
    pub guardian_email: String,
    pub guardian_phone: String,
}

pub async fn create_admission(
    auth_session: AuthSession,
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(payload): Json<CreateAdmissionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session.user.ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;
    
    // Allowed: SCHOOL_ADMIN, RECEPTIONIST, CLERK
    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) 
        && !user.roles.contains(&"RECEPTIONIST".to_string()) 
        && !user.roles.contains(&"CLERK".to_string()) {
        return Err(AppError::Forbidden("Only Admins or Front Office staff can create admissions".into()));
    }
    
    let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]").unwrap();
    let dob = time::Date::parse(&payload.date_of_birth, &format)
        .map_err(|_| AppError::BadRequest("Invalid date format. Use YYYY-MM-DD".into()))?;
        
    let admission_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO admissions 
           (school_id, student_first_name, student_last_name, date_of_birth, 
            guardian_first_name, guardian_last_name, guardian_email, guardian_phone, 
            created_by_user_id, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'PENDING')
           RETURNING id"#
    )
    .bind(user.school_id)
    .bind(&payload.student_first_name)
    .bind(&payload.student_last_name)
    .bind(dob)
    .bind(&payload.guardian_first_name)
    .bind(&payload.guardian_last_name)
    .bind(&payload.guardian_email)
    .bind(&payload.guardian_phone)
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;
    
    crate::utils::audit::log_action(&state.db, Some(user.school_id), Some(user.id), "CREATE_ADMISSION", None, None, None, Some(addr.to_string())).await.ok();
    
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "message": "Admission submitted successfully",
            "admission_id": admission_id
        }))
    ))
}

pub async fn enroll_admission(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(admission_id): Path<Uuid>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session.user.ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;
    
    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        return Err(AppError::Forbidden("Only School Admins can officially enroll students from admissions".into()));
    }
    
    let mut tx = state.db.begin().await?;
    
    // 1. Fetch admission

    let admission = sqlx::query(
        "SELECT student_first_name, student_last_name, date_of_birth, guardian_first_name, guardian_last_name, guardian_email, guardian_phone, status::text 
         FROM admissions WHERE id = $1 AND school_id = $2 FOR UPDATE"
    )
    .bind(admission_id).bind(user.school_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Admission not found".into()))?;
    
    use sqlx::Row;
    let student_first_name: String = admission.try_get("student_first_name").unwrap();
    let student_last_name: String = admission.try_get("student_last_name").unwrap();
    let date_of_birth: time::Date = admission.try_get("date_of_birth").unwrap();
    let guardian_first_name: String = admission.try_get("guardian_first_name").unwrap();
    let guardian_last_name: String = admission.try_get("guardian_last_name").unwrap();
    let guardian_email: String = admission.try_get("guardian_email").unwrap();
    let guardian_phone: String = admission.try_get("guardian_phone").unwrap();
    let status: String = admission.try_get("status").unwrap();

    if status != "APPROVED" && status != "PENDING" {

        return Err(AppError::BadRequest("Admission cannot be enrolled. It is either already enrolled or rejected.".into()));
    }
    
    let default_pass = crate::utils::password::hash_password("Welcome123!").unwrap();
    
    // 2. Create Student User
    let s_username = format!("{}.{}", student_first_name.to_lowercase(), admission_id.to_string().chars().take(5).collect::<String>());
    let s_user_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (school_id, first_name, last_name, username, password_hash, status, roles, updated_by_user_id)
           VALUES ($1, $2, $3, $4, $5, 'ACTIVE', ARRAY['STUDENT']::system_role_type[], $6) RETURNING id"#
    )
    .bind(user.school_id).bind(&student_first_name).bind(&student_last_name)
    .bind(&s_username).bind(&default_pass).bind(user.id)
    .fetch_one(&mut *tx).await?;
    
    // Generate an admission number
    let admission_number = format!("ADM-{}", admission_id.to_string().chars().take(8).collect::<String>().to_uppercase());
    
    let student_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO students (school_id, user_id, admission_number, first_name, last_name, date_of_birth)
           VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#
    )
    .bind(user.school_id).bind(s_user_id).bind(&admission_number)
    .bind(&student_first_name).bind(&student_last_name).bind(date_of_birth)
    .fetch_one(&mut *tx).await?;
    
    // 3. Create Guardian User
    let g_username = format!("{}_{}", guardian_first_name.to_lowercase(), guardian_phone);
    let g_user_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status, roles, updated_by_user_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE', ARRAY['PARENT']::system_role_type[], $8) RETURNING id"#
    )
    .bind(user.school_id).bind(&guardian_first_name).bind(&guardian_last_name)
    .bind(&guardian_email).bind(&guardian_phone)
    .bind(&g_username).bind(&default_pass).bind(user.id)
    .fetch_one(&mut *tx).await?;
    
    let guardian_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO guardians (school_id, user_id, first_name, last_name, email, phone)
           VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#
    )
    .bind(user.school_id).bind(g_user_id)
    .bind(&guardian_first_name).bind(&guardian_last_name)
    .bind(&guardian_email).bind(&guardian_phone)
    .fetch_one(&mut *tx).await?;
    
    // 4. Link Student & Guardian
    sqlx::query(
        r#"INSERT INTO student_guardians (student_id, guardian_id, relationship) VALUES ($1, $2, 'PARENT')"#
    )
    .bind(student_id).bind(guardian_id)
    .execute(&mut *tx).await?;
    
    // 5. Update Admission Status
    sqlx::query(
        "UPDATE admissions SET status = 'ENROLLED', enrolled_student_user_id = $1, updated_at = now() WHERE id = $2"
    )
    .bind(s_user_id).bind(admission_id)
    .execute(&mut *tx).await?;
    
    tx.commit().await?;
    
    crate::utils::audit::log_action(&state.db, Some(user.school_id), Some(user.id), "SYSTEM_ENROLL_ADMISSION", None, None, None, Some(addr.to_string())).await.ok();
    
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "message": "Student successfully enrolled from admission",
            "admission_number": admission_number
        }))
    ))
}
