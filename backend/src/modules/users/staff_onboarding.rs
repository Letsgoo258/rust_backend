use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use crate::utils::csv_engine::{ColumnSchema, ColumnType, CsvSchema};

#[derive(Deserialize)]
pub struct StaffBulkQuery {
    pub role: String,
}

pub async fn bulk_upload_staff(
    auth_session: AuthSession,
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Query(query): Query<StaffBulkQuery>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session.user.ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        return Err(AppError::Forbidden("Only School Admins can add staff".into()));
    }


    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        return Err(AppError::Forbidden("Only School Admins can perform bulk staff uploads".into()));
    }

    
    // Dynamic Schema Selection
    let mut columns = vec![
        ColumnSchema { header: "First Name", col_type: ColumnType::String, required: true, min_length: Some(1), max_length: Some(100) },
        ColumnSchema { header: "Last Name", col_type: ColumnType::String, required: true, min_length: Some(1), max_length: Some(100) },
        ColumnSchema { header: "Email", col_type: ColumnType::Email, required: true, min_length: None, max_length: None },
        ColumnSchema { header: "Phone", col_type: ColumnType::Phone, required: true, min_length: None, max_length: None },
        ColumnSchema { header: "Employee ID", col_type: ColumnType::String, required: true, min_length: Some(1), max_length: Some(50) },
        ColumnSchema { header: "Joining Date (YYYY-MM-DD)", col_type: ColumnType::Date, required: true, min_length: None, max_length: None },
    ];
    
    // Add Academic fields if role is Academic
    let academic_roles = ["TEACHER", "CLASS_TEACHER", "HEADMASTER", "SUBJECT_COORDINATOR", "EXAM_COORDINATOR", "LAB_ASSISTANT"];
    if academic_roles.contains(&query.role.as_str()) {
        columns.push(ColumnSchema { header: "Department", col_type: ColumnType::String, required: false, min_length: None, max_length: Some(100) });
    }
    
    let schema = CsvSchema { columns };
    
    let mut raw_data = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(|_| AppError::BadRequest("Multipart error".into()))? {
        if field.name() == Some("file") {
            raw_data = field.bytes().await.map_err(|_| AppError::BadRequest("Failed to read file".into()))?.to_vec();
            break;
        }
    }
    
    if raw_data.is_empty() {
        return Err(AppError::BadRequest("No CSV file provided".into()));
    }
    
    let parse_result = schema.parse(&raw_data).map_err(|e| AppError::BadRequest(e))?;
    let mut final_success = 0;
    
    let default_pass = crate::utils::password::hash_password("Welcome123!").unwrap();
    
    for row in parse_result.valid_rows {
        let first_name = row.get("First Name").unwrap().clone();
        let last_name = row.get("Last Name").unwrap().clone();
        let email = row.get("Email").unwrap().clone();
        let phone = row.get("Phone").unwrap().clone();
        let emp_id = row.get("Employee ID").unwrap().clone();
        let joining_date_str = row.get("Joining Date (YYYY-MM-DD)").unwrap().clone();
        
        let department = row.get("Department").cloned();
        
        let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]").unwrap();
        let joining_date = time::Date::parse(&joining_date_str, &format)
            .map_err(|_| AppError::BadRequest("Invalid joining date format".into()))?;
        
        let username = format!("{}{}", first_name.to_lowercase(), emp_id);
        
        let mut tx = state.db.begin().await?;
        
        // Wait, query.role needs to be cast to system_role_type in an ARRAY. 
        // We'll use a dynamic query or just text query because sqlx macros struggle with dynamic enums in arrays.
        // Or we just insert it dynamically using query (not query!).
        
        let user_id: uuid::Uuid = sqlx::query_scalar(
            r#"INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status, roles, imported_via_csv, updated_by_user_id) 
               VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE', ARRAY[$8::system_role_type], true, $9) 
               RETURNING id"#
        )
        .bind(user.school_id)
        .bind(&first_name)
        .bind(&last_name)
        .bind(&email)
        .bind(&phone)
        .bind(&username)
        .bind(&default_pass)
        .bind(&query.role)
        .bind(user.id)
        .fetch_one(&mut *tx)
        .await?;
        
        sqlx::query(
            "INSERT INTO staff (school_id, user_id, employee_id, department, joining_date)
             VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(user.school_id)
        .bind(user_id)
        .bind(&emp_id)
        .bind(department)
        .bind(joining_date)
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        final_success += 1;
    }
    
    crate::utils::audit::log_action(&state.db, Some(user.school_id), Some(user.id), "BULK_CSV_UPLOAD_STAFF", None, None, None, Some(addr.to_string())).await.ok();
    
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "message": format!("Successfully processed {} staff rows", final_success),
            "errors_base64": parse_result.failed_csv
        }))
    ))
}


#[derive(Deserialize)]
pub struct StaffSingleQuery {
    pub role: String,
}

pub async fn single_add_staff(
    auth_session: AuthSession,
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Query(query): Query<StaffSingleQuery>,
    Json(payload): Json<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session.user.ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        return Err(AppError::Forbidden("Only School Admins can add staff".into()));
    }


    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        return Err(AppError::Forbidden("Only School Admins can perform bulk staff uploads".into()));
    }

    
    let first_name = payload.get("First Name").cloned().unwrap_or_default();
    let last_name = payload.get("Last Name").cloned().unwrap_or_default();
    let email = payload.get("Email").cloned().unwrap_or_default();
    let phone = payload.get("Phone").cloned().unwrap_or_default();
    let emp_id = payload.get("Employee ID").cloned().unwrap_or_default();
    let joining_date_str = payload.get("Joining Date (YYYY-MM-DD)").cloned().unwrap_or_default();
    let department = payload.get("Department").cloned();
    
    let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]").unwrap();
    let joining_date = time::Date::parse(&joining_date_str, &format)
        .map_err(|_| AppError::BadRequest("Invalid joining date format".into()))?;
    
    let default_pass = crate::utils::password::hash_password("Welcome123!").unwrap();
    let username = format!("{}{}", first_name.to_lowercase(), emp_id);
    
    let mut tx = state.db.begin().await?;
    
    let user_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status, roles, updated_by_user_id) 
           VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE', ARRAY[$8::system_role_type], $9) 
           RETURNING id"#
    )
    .bind(user.school_id).bind(&first_name).bind(&last_name).bind(&email).bind(&phone)
    .bind(&username).bind(&default_pass).bind(&query.role).bind(user.id)
    .fetch_one(&mut *tx).await?;
    
    sqlx::query(
        "INSERT INTO staff (school_id, user_id, employee_id, department, joining_date)
         VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(user.school_id).bind(user_id).bind(&emp_id).bind(department).bind(joining_date)
    .execute(&mut *tx).await?;
    
    tx.commit().await?;
    crate::utils::audit::log_action(&state.db, Some(user.school_id), Some(user.id), "SINGLE_ADD_STAFF", None, None, None, Some(addr.to_string())).await.ok();
    
    Ok((StatusCode::OK, Json(serde_json::json!({"message": "Staff added successfully"}))))
}
