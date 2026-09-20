use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use crate::utils::csv_engine::{ColumnSchema, ColumnType, CsvSchema};

pub async fn bulk_upload_students(
    auth_session: AuthSession,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session.user.ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;
    
    let schema = CsvSchema {
        columns: vec![
            ColumnSchema { header: "First Name", col_type: ColumnType::String, required: true, min_length: Some(1), max_length: Some(100) },
            ColumnSchema { header: "Last Name", col_type: ColumnType::String, required: true, min_length: Some(1), max_length: Some(100) },
            ColumnSchema { header: "Admission Number", col_type: ColumnType::String, required: true, min_length: Some(1), max_length: Some(50) },
            ColumnSchema { header: "Date of Birth (YYYY-MM-DD)", col_type: ColumnType::Date, required: true, min_length: None, max_length: None },
            ColumnSchema { header: "Guardian First Name", col_type: ColumnType::String, required: true, min_length: Some(1), max_length: Some(100) },
            ColumnSchema { header: "Guardian Last Name", col_type: ColumnType::String, required: true, min_length: Some(1), max_length: Some(100) },
            ColumnSchema { header: "Guardian Email", col_type: ColumnType::Email, required: true, min_length: None, max_length: None },
            ColumnSchema { header: "Guardian Phone", col_type: ColumnType::Phone, required: true, min_length: None, max_length: None },
            ColumnSchema { header: "Student Phone", col_type: ColumnType::Phone, required: false, min_length: None, max_length: None },
        ]
    };
    
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
    
    let mut failed_records_extra = Vec::new();
    let mut err_headers = vec![
        "First Name", "Last Name", "Admission Number", "Date of Birth (YYYY-MM-DD)", 
        "Guardian First Name", "Guardian Last Name", "Guardian Email", "Guardian Phone", "Error_Reason"
    ];
    
    for row in parse_result.valid_rows {
        let first = row.get("First Name").unwrap();
        let last = row.get("Last Name").unwrap();
        let adm = row.get("Admission Number").unwrap();
        let dob_str = row.get("Date of Birth (YYYY-MM-DD)").unwrap();
        let g_first = row.get("Guardian First Name").unwrap();
        let g_last = row.get("Guardian Last Name").unwrap();
        let g_email = row.get("Guardian Email").unwrap();
        let g_phone = row.get("Guardian Phone").unwrap();
        let s_phone_raw = row.get("Student Phone").unwrap_or(&String::new()).clone();
        let s_phone = if s_phone_raw.is_empty() { None } else { Some(s_phone_raw.clone()) };
        
        let format = time::format_description::parse("[year]-[month]-[day]").unwrap();
        let dob = match time::Date::parse(dob_str, &format) {
            Ok(d) => d,
            Err(_) => {
                failed_records_extra.push(vec![first.clone(), last.clone(), adm.clone(), dob_str.clone(), g_first.clone(), g_last.clone(), g_email.clone(), g_phone.clone(), s_phone_raw.clone(), "Invalid Date format (Use YYYY-MM-DD)".to_string()]);
                continue;
            }
        };
        
        // Ensure adm is unique
        let existing = sqlx::query!("SELECT id FROM students WHERE school_id = $1 AND admission_number = $2", user.school_id, adm)
            .fetch_optional(&state.db).await?;
            
        if existing.is_some() {
            failed_records_extra.push(vec![first.clone(), last.clone(), adm.clone(), dob_str.clone(), g_first.clone(), g_last.clone(), g_email.clone(), g_phone.clone(), s_phone_raw.clone(), "Duplicate Admission Number".to_string()]);
            continue;
        }
        
        // Transaction to insert Student, Guardian, and User accounts
        let mut tx = match state.db.begin().await {
            Ok(tx) => tx,
            Err(_) => {
                failed_records_extra.push(vec![first.clone(), last.clone(), adm.clone(), dob_str.clone(), g_first.clone(), g_last.clone(), g_email.clone(), g_phone.clone(), s_phone_raw.clone(), "DB Transaction Error".to_string()]);
                continue;
            }
        };
        
        let default_pass = crate::utils::password::hash_password("Welcome123!").unwrap();
        let s_username = format!("{}{}", adm.to_lowercase(), user.school_id.to_string().chars().take(4).collect::<String>());
        
        let s_user = sqlx::query!(
            r#"INSERT INTO users (school_id, first_name, last_name, username, password_hash, status, roles, imported_via_csv, updated_by_user_id, phone)
               VALUES ($1, $2, $3, $4, $5, 'INACTIVE', ARRAY['STUDENT']::system_role_type[], true, $6, $7) RETURNING id"#,
            user.school_id, first, last, s_username, default_pass, user.id, s_phone
        ).fetch_one(&mut *tx).await.unwrap();
        
        let s_record = sqlx::query!(
            r#"INSERT INTO students (school_id, user_id, admission_number, first_name, last_name, date_of_birth)
               VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#,
            user.school_id, s_user.id, adm, first, last, dob
        ).fetch_one(&mut *tx).await.unwrap();
        
        let g_username = format!("{}_{}", g_first.to_lowercase(), g_phone);
        let g_user = sqlx::query!(
            r#"INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status, roles, imported_via_csv, updated_by_user_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, 'INACTIVE', ARRAY['PARENT']::system_role_type[], true, $8) RETURNING id"#,
            user.school_id, g_first, g_last, g_email, g_phone, g_username, default_pass, user.id
        ).fetch_one(&mut *tx).await.unwrap();
        
        let g_record = sqlx::query!(
            r#"INSERT INTO guardians (school_id, user_id, first_name, last_name, email, phone)
               VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#,
            user.school_id, g_user.id, g_first, g_last, g_email, g_phone
        ).fetch_one(&mut *tx).await.unwrap();
        
        sqlx::query!(
            r#"INSERT INTO student_guardians (student_id, guardian_id, relationship) VALUES ($1, $2, 'PARENT')"#,
            s_record.id, g_record.id
        ).execute(&mut *tx).await.unwrap();
        
        if tx.commit().await.is_ok() {
            final_success += 1;
        } else {
            failed_records_extra.push(vec![first.clone(), last.clone(), adm.clone(), dob_str.clone(), g_first.clone(), g_last.clone(), g_email.clone(), g_phone.clone(), s_phone_raw.clone(), "DB Commit Error".to_string()]);
        }
    }
    
    let mut final_failed_csv = parse_result.failed_csv.clone();
    
    // If we have DB constraint failures, append them to the failed CSV buffer
    if !failed_records_extra.is_empty() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        
        let current_bytes = if let Some(b64) = &parse_result.failed_csv {
            STANDARD.decode(b64).unwrap_or_default()
        } else {
            let mut wtr = csv::Writer::from_writer(vec![]);
            let _ = wtr.write_record(&err_headers);
            wtr.into_inner().unwrap_or_default()
        };
        
        let mut wtr = csv::Writer::from_writer(current_bytes);
        for fr in &failed_records_extra {
            let _ = wtr.write_record(fr);
        }
        
        if let Ok(bytes) = wtr.into_inner() {
            final_failed_csv = Some(STANDARD.encode(&bytes));
        }
    }
    
    Ok((StatusCode::OK, Json(serde_json::json!({
        "success_count": final_success,
        "failed_count": parse_result.failed_count + failed_records_extra.len(),
        "failed_csv": final_failed_csv
    }))))
}
