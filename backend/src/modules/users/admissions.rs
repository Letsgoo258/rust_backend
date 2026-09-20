use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;
use std::collections::HashMap;
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};

#[derive(Deserialize)]
pub struct CreateAdmissionRequest {
    pub applicant_first_name: String,
    pub applicant_middle_name: Option<String>,
    pub applicant_last_name: String,
    pub date_of_birth: String, // YYYY-MM-DD
    pub applicant_gender: Option<String>,
    pub applicant_phone: Option<String>,
    pub applicant_email: Option<String>,
    
    pub guardian_name: String,
    pub guardian_relationship: String,
    pub guardian_phone: String,
    pub guardian_email: Option<String>,
    pub guardian_occupation: Option<String>,
    
    // Custom Fields: field_key -> value
    #[serde(default)]
    pub custom_fields: HashMap<String, String>,
}

pub async fn create_admission(
    auth_session: AuthSession,
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(payload): Json<CreateAdmissionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session.user.ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;
    
    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) 
        && !user.roles.contains(&"RECEPTIONIST".to_string()) 
        && !user.roles.contains(&"CLERK".to_string()) {
        return Err(AppError::Forbidden("Only Admins or Front Office staff can create admissions".into()));
    }
    
    let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]").unwrap();
    let dob = time::Date::parse(&payload.date_of_birth, &format)
        .map_err(|_| AppError::BadRequest("Invalid date format. Use YYYY-MM-DD".into()))?;
        
    let mut tx = state.db.begin().await?;
    
    // 1. Create Base Admission
    let admission_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO admissions (school_id, status, submitted_at) VALUES ($1, 'PENDING', now()) RETURNING id"#
    )
    .bind(user.school_id)
    .fetch_one(&mut *tx).await?;
    
    // 2. Insert Applicant
    sqlx::query(
        r#"INSERT INTO admission_applicants 
           (admission_id, first_name, middle_name, last_name, date_of_birth, gender, phone, email)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#
    )
    .bind(admission_id).bind(&payload.applicant_first_name).bind(&payload.applicant_middle_name)
    .bind(&payload.applicant_last_name).bind(dob).bind(&payload.applicant_gender)
    .bind(&payload.applicant_phone).bind(&payload.applicant_email)
    .execute(&mut *tx).await?;
    
    // 3. Insert Guardian
    sqlx::query(
        r#"INSERT INTO admission_guardians 
           (admission_id, name, relationship, phone, email, occupation)
           VALUES ($1, $2, $3, $4, $5, $6)"#
    )
    .bind(admission_id).bind(&payload.guardian_name).bind(&payload.guardian_relationship)
    .bind(&payload.guardian_phone).bind(&payload.guardian_email).bind(&payload.guardian_occupation)
    .execute(&mut *tx).await?;
    
    // 4. Record Status History
    sqlx::query(
        r#"INSERT INTO admission_status_history (admission_id, to_status, changed_by, reason) 
           VALUES ($1, 'PENDING', $2, 'Initial Submission')"#
    )
    .bind(admission_id).bind(user.id)
    .execute(&mut *tx).await?;
    
    // 5. Handle Custom Fields
    for (key, value) in &payload.custom_fields {
        let field_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM custom_fields WHERE school_id = $1 AND entity_type = 'ADMISSION' AND field_key = $2"
        )
        .bind(user.school_id).bind(key)
        .fetch_optional(&mut *tx).await?;
        
        if let Some(fid) = field_id {
            sqlx::query("INSERT INTO admission_custom_values (admission_id, field_id, value) VALUES ($1, $2, $3)")
                .bind(admission_id).bind(fid).bind(value)
                .execute(&mut *tx).await?;
        }
    }
    
    tx.commit().await?;
    
    crate::utils::audit::log_action(&state.db, Some(user.school_id), Some(user.id), "CREATE_ADMISSION_SRD", None, None, None, Some(addr.to_string())).await.ok();
    
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
    
    let admission = sqlx::query(
        "SELECT status::text FROM admissions WHERE id = $1 AND school_id = $2 FOR UPDATE"
    )
    .bind(admission_id).bind(user.school_id)
    .fetch_optional(&mut *tx).await?
    .ok_or_else(|| AppError::NotFound("Admission not found".into()))?;
    
    use sqlx::Row;
    let status: String = admission.try_get("status").unwrap();
    
    if status != "APPROVED" && status != "PENDING" {
        return Err(AppError::BadRequest("Admission cannot be enrolled. It is either already enrolled or rejected.".into()));
    }
    
    // Fetch applicant details
    let applicant = sqlx::query(
        "SELECT first_name, last_name, date_of_birth FROM admission_applicants WHERE admission_id = $1"
    ).bind(admission_id).fetch_one(&mut *tx).await?;
    
    let applicant_first_name: String = applicant.try_get("first_name").unwrap();
    let applicant_last_name: String = applicant.try_get("last_name").unwrap();
    let date_of_birth: time::Date = applicant.try_get("date_of_birth").unwrap();
    
    // Fetch guardian details
    let guardian = sqlx::query(
        "SELECT name, phone, email, relationship FROM admission_guardians WHERE admission_id = $1 LIMIT 1"
    ).bind(admission_id).fetch_one(&mut *tx).await?;
    
    let guardian_name: String = guardian.try_get("name").unwrap();
    let guardian_phone: String = guardian.try_get("phone").unwrap();
    let guardian_email: Option<String> = guardian.try_get("email").ok();
    
    let default_pass = crate::utils::password::hash_password("Welcome123!").unwrap();
    
    // 2. Create Student User
    let s_username = format!("{}.{}", applicant_first_name.to_lowercase(), admission_id.to_string().chars().take(5).collect::<String>());
    let s_user_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (school_id, first_name, last_name, username, password_hash, status, roles, updated_by_user_id)
           VALUES ($1, $2, $3, $4, $5, 'ACTIVE', ARRAY['STUDENT']::system_role_type[], $6) RETURNING id"#
    )
    .bind(user.school_id).bind(&applicant_first_name).bind(&applicant_last_name)
    .bind(&s_username).bind(&default_pass).bind(user.id)
    .fetch_one(&mut *tx).await?;
    
    let admission_number = format!("ADM-{}", admission_id.to_string().chars().take(8).collect::<String>().to_uppercase());
    
    let student_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO students (school_id, user_id, admission_number, first_name, last_name, date_of_birth)
           VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#
    )
    .bind(user.school_id).bind(s_user_id).bind(&admission_number)
    .bind(&applicant_first_name).bind(&applicant_last_name).bind(date_of_birth)
    .fetch_one(&mut *tx).await?;
    
    // 3. Create Guardian User
    let names: Vec<&str> = guardian_name.split_whitespace().collect();
    let g_first = names.first().unwrap_or(&"Guardian");
    let g_last = names.get(1).unwrap_or(&"");
    
    let g_username = format!("{}_{}", g_first.to_lowercase(), guardian_phone);
    let g_user_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status, roles, updated_by_user_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE', ARRAY['PARENT']::system_role_type[], $8) RETURNING id"#
    )
    .bind(user.school_id).bind(g_first).bind(g_last)
    .bind(&guardian_email).bind(&guardian_phone)
    .bind(&g_username).bind(&default_pass).bind(user.id)
    .fetch_one(&mut *tx).await?;
    
    let guardian_record_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO guardians (school_id, user_id, first_name, last_name, email, phone)
           VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#
    )
    .bind(user.school_id).bind(g_user_id)
    .bind(g_first).bind(g_last)
    .bind(&guardian_email).bind(&guardian_phone)
    .fetch_one(&mut *tx).await?;
    
    // 4. Link Student & Guardian
    sqlx::query(
        r#"INSERT INTO student_guardians (student_id, guardian_id, relationship) VALUES ($1, $2, 'PARENT')"#
    )
    .bind(student_id).bind(guardian_record_id)
    .execute(&mut *tx).await?;
    
    // 5. Update Admission Status
    sqlx::query(
        "UPDATE admissions SET status = 'ENROLLED', application_number = $1, approved_at = now(), approved_by = $2, updated_at = now() WHERE id = $3"
    )
    .bind(&admission_number).bind(user.id).bind(admission_id)
    .execute(&mut *tx).await?;
    
    sqlx::query(
        r#"INSERT INTO admission_status_history (admission_id, from_status, to_status, changed_by, reason) 
           VALUES ($1, 'APPROVED', 'ENROLLED', $2, 'System Enrollment')"#
    )
    .bind(admission_id).bind(user.id)
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
