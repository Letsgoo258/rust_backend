use crate::utils::csv_engine::{ColumnSchema, ColumnType, CsvSchema};
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct StaffBulkQuery {
    pub role: Option<String>,
}

pub async fn bulk_upload_staff(
    auth_session: AuthSession,
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Query(query): Query<StaffBulkQuery>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SUPER_ADMIN".to_string()) && !user.is_school_admin() {
        crate::utils::telemetry::dispatch_security_alert(
            &state.db,
            user.school_id,
            user.id,
            "BULK_STAFF_UPLOAD_ATTEMPT",
            "staff_bulk_upload",
            "Only School Admins or Super Admins can perform bulk staff uploads",
            &user.roles,
            Some(addr.to_string()),
        )
        .await;
        return Err(AppError::Forbidden(
            "Only School Admins or Super Admins can perform bulk staff uploads".into(),
        ));
    }

    let target_role = query.role.as_deref().unwrap_or("TEACHER");

    // Dynamic Schema Selection
    let columns = vec![
        ColumnSchema {
            header: "First Name",
            col_type: ColumnType::String,
            required: true,
            min_length: Some(1),
            max_length: Some(100),
        },
        ColumnSchema {
            header: "Last Name",
            col_type: ColumnType::String,
            required: true,
            min_length: Some(1),
            max_length: Some(100),
        },
        ColumnSchema {
            header: "Email",
            col_type: ColumnType::Email,
            required: true,
            min_length: None,
            max_length: None,
        },
        ColumnSchema {
            header: "Phone",
            col_type: ColumnType::Phone,
            required: true,
            min_length: None,
            max_length: None,
        },
        ColumnSchema {
            header: "Employee ID",
            col_type: ColumnType::String,
            required: false,
            min_length: None,
            max_length: Some(50),
        },
        ColumnSchema {
            header: "Joining Date (YYYY-MM-DD)",
            col_type: ColumnType::Date,
            required: false,
            min_length: None,
            max_length: None,
        },
        ColumnSchema {
            header: "Department",
            col_type: ColumnType::String,
            required: false,
            min_length: None,
            max_length: Some(100),
        },
    ];

    let schema = CsvSchema { columns };

    let mut raw_data = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("Multipart error".into()))?
    {
        if field.name() == Some("file") {
            raw_data = field
                .bytes()
                .await
                .map_err(|_| AppError::BadRequest("Failed to read file".into()))?
                .to_vec();
            break;
        }
    }

    if raw_data.is_empty() {
        return Err(AppError::BadRequest("No CSV file provided".into()));
    }

    let parse_result = schema
        .parse(&raw_data)
        .map_err(|e| AppError::BadRequest(e))?;

    let job_id = uuid::Uuid::new_v4();
    let total_valid = parse_result.valid_rows.len();
    let now_ts = time::OffsetDateTime::now_utc().unix_timestamp();

    let initial_job = crate::utils::jobs::BulkUploadJob {
        id: job_id,
        school_id: user.school_id,
        user_id: user.id,
        job_type: "STAFF_BULK_UPLOAD".to_string(),
        status: crate::utils::jobs::JobStatus::PROCESSING,
        total_rows: total_valid,
        processed_rows: 0,
        success_count: 0,
        failed_count: parse_result.failed_count,
        failed_csv: parse_result.failed_csv.clone(),
        error: None,
        created_at: now_ts,
        updated_at: now_ts,
    };

    state.jobs.insert(job_id, initial_job.clone());
    crate::utils::jobs::save_job_to_db(&state.db, &initial_job, None).await;

    let bg_state = state.clone();
    let bg_school_id = user.school_id;
    let bg_user_id = user.id;
    let bg_target_role = target_role.to_string();

    tokio::spawn(async move {
        process_bulk_staff_background(
            bg_state,
            bg_school_id,
            bg_user_id,
            job_id,
            bg_target_role,
            parse_result,
            addr,
        )
        .await;
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": "Bulk staff upload job started",
            "job_id": job_id,
            "status": "PROCESSING",
            "total_rows": total_valid,
        })),
    ))
}

pub async fn get_staff_bulk_upload_status(
    auth_session: AuthSession,
    State(state): State<AppState>,
    axum::extract::Path(job_id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let job = if let Some(j) = state.jobs.get(&job_id) {
        j.value().clone()
    } else if let Some(j) = crate::utils::jobs::get_job_from_db(&state.db, job_id).await {
        state.jobs.insert(job_id, j.clone());
        j
    } else {
        return Err(AppError::NotFound("Job not found".into()));
    };

    if job.school_id != user.school_id {
        return Err(AppError::Forbidden("Access denied to this job".into()));
    }

    Ok((StatusCode::OK, Json(job)))
}

async fn process_bulk_staff_background(
    state: AppState,
    school_id: uuid::Uuid,
    user_id: uuid::Uuid,
    job_id: uuid::Uuid,
    target_role: String,
    parse_result: crate::utils::csv_engine::CsvParseResult,
    addr: std::net::SocketAddr,
) {
    let mut final_success = 0;
    let default_pass = crate::utils::password::hash_password("Welcome123!").unwrap_or_default();
    let mut failed_records_extra: Vec<Vec<String>> = Vec::new();
    let err_headers = vec![
        "First Name",
        "Last Name",
        "Email",
        "Phone",
        "Employee ID",
        "Joining Date",
        "Department",
        "Error_Reason",
    ];

    // Pre-fetch existing emails and phones in this school via repository
    let (mut existing_emails, mut existing_phones) =
        crate::modules::users::repository::UserRepository::get_existing_emails_and_phones(
            &state.db, school_id,
        )
        .await
        .unwrap_or_default();

    for (idx, row) in parse_result.valid_rows.iter().enumerate() {
        let first_name = match row.get("First Name") {
            Some(v) => v.clone(),
            None => continue,
        };
        let last_name = match row.get("Last Name") {
            Some(v) => v.clone(),
            None => continue,
        };
        let email = match row.get("Email") {
            Some(v) => v.clone(),
            None => continue,
        };
        let phone =
            crate::utils::phone::normalize_phone_number(row.get("Phone").unwrap_or(&String::new()));

        let emp_id = row
            .get("Employee ID")
            .cloned()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                format!(
                    "TCH{}",
                    uuid::Uuid::new_v4()
                        .to_string()
                        .chars()
                        .take(4)
                        .collect::<String>()
                        .to_uppercase()
                )
            });

        let joining_date_str = row
            .get("Joining Date (YYYY-MM-DD)")
            .cloned()
            .unwrap_or_default();
        let joining_date = row
            .get("Joining Date (YYYY-MM-DD)")
            .and_then(|s| crate::utils::datetime::DateUtils::parse_iso_date(s).ok())
            .unwrap_or_else(|| crate::utils::datetime::DateUtils::today_in_timezone(None));

        let department = row.get("Department").cloned();
        let dep_str = department.clone().unwrap_or_default();

        let email_lower = email.to_lowercase();

        // Validate required login credentials: Email and Phone are strictly required
        let has_email = !email.trim().is_empty();
        let has_phone = !phone.trim().is_empty();
        if !has_email || !has_phone {
            let err_reason = if !has_email && !has_phone {
                "Omitted from upload: Email and Phone are strictly required for staff login credentials.".to_string()
            } else if !has_email {
                "Omitted from upload: Email is strictly required for staff login credentials."
                    .to_string()
            } else {
                "Omitted from upload: Phone is strictly required for staff login credentials."
                    .to_string()
            };
            tracing::warn!(
                "Row skipped: staff {} lacks login credentials: {}",
                emp_id,
                err_reason
            );
            failed_records_extra.push(vec![
                first_name,
                last_name,
                email,
                phone,
                emp_id,
                joining_date_str,
                dep_str,
                err_reason,
            ]);
            if let Some(mut j) = state.jobs.get_mut(&job_id) {
                j.processed_rows = idx + 1;
                j.success_count = final_success;
                j.failed_count = parse_result.failed_count + failed_records_extra.len();
                j.updated_at = time::OffsetDateTime::now_utc().unix_timestamp();
            }
            continue;
        }

        if existing_emails.contains(&email_lower) {
            tracing::info!("Row skipped: email {} already registered in school", email);
            failed_records_extra.push(vec![
                first_name,
                last_name,
                email,
                phone,
                emp_id,
                joining_date_str,
                dep_str,
                "Email already registered in school".to_string(),
            ]);
            if let Some(mut j) = state.jobs.get_mut(&job_id) {
                j.processed_rows = idx + 1;
                j.success_count = final_success;
                j.failed_count = parse_result.failed_count + failed_records_extra.len();
                j.updated_at = time::OffsetDateTime::now_utc().unix_timestamp();
            }
            continue;
        }

        if !phone.is_empty() && existing_phones.contains(&phone) {
            tracing::info!("Row skipped: phone {} already registered in school", phone);
            failed_records_extra.push(vec![
                first_name,
                last_name,
                email,
                phone,
                emp_id,
                joining_date_str,
                dep_str,
                "Phone already registered in school".to_string(),
            ]);
            if let Some(mut j) = state.jobs.get_mut(&job_id) {
                j.processed_rows = idx + 1;
                j.success_count = final_success;
                j.failed_count = parse_result.failed_count + failed_records_extra.len();
                j.updated_at = time::OffsetDateTime::now_utc().unix_timestamp();
            }
            continue;
        }

        let username = format!("{}{}", first_name.to_lowercase(), emp_id.to_lowercase());

        let mut tx_guard =
            match crate::utils::write_controller::DbWriteController::begin(&state.db).await {
                Ok(tx) => tx,
                Err(e) => {
                    tracing::error!("Failed to begin tx for staff {}: {:?}", email, e);
                    failed_records_extra.push(vec![
                        first_name,
                        last_name,
                        email,
                        phone,
                        emp_id,
                        joining_date_str,
                        dep_str,
                        "DB Transaction Error".to_string(),
                    ]);
                    if let Some(mut j) = state.jobs.get_mut(&job_id) {
                        j.processed_rows = idx + 1;
                        j.success_count = final_success;
                        j.failed_count = parse_result.failed_count + failed_records_extra.len();
                        j.updated_at = time::OffsetDateTime::now_utc().unix_timestamp();
                    }
                    continue;
                }
            };

        let create_res = crate::modules::users::repository::UserRepository::create_staff_user(
            tx_guard.conn(),
            school_id,
            &first_name,
            &last_name,
            &email,
            &phone,
            &username,
            &default_pass,
            &emp_id,
            joining_date,
            department,
            &target_role,
            user_id,
            true,
        )
        .await;

        match create_res {
            Ok(_) => {
                if let Err(e) = tx_guard.commit().await {
                    tracing::error!("Failed to commit tx for staff {}: {:?}", email, e);
                    failed_records_extra.push(vec![
                        first_name,
                        last_name,
                        email,
                        phone,
                        emp_id,
                        joining_date_str,
                        dep_str,
                        "DB Commit Error".to_string(),
                    ]);
                } else {
                    existing_emails.insert(email_lower);
                    if !phone.is_empty() {
                        existing_phones.insert(phone);
                    }
                    final_success += 1;
                }
            }
            Err(e) => {
                let reason = format!("Staff creation error: {}", e);
                tracing::info!("Row skipped for staff {}: {}", email, reason);
                failed_records_extra.push(vec![
                    first_name,
                    last_name,
                    email,
                    phone,
                    emp_id,
                    joining_date_str,
                    dep_str,
                    reason,
                ]);
            }
        };

        // Update progress in job store on every iteration
        if let Some(mut j) = state.jobs.get_mut(&job_id) {
            j.processed_rows = idx + 1;
            j.success_count = final_success;
            j.failed_count = parse_result.failed_count + failed_records_extra.len();
            j.updated_at = time::OffsetDateTime::now_utc().unix_timestamp();
        }
    }

    let mut final_failed_csv = parse_result.failed_csv.clone();

    if !failed_records_extra.is_empty() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};

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

    crate::utils::audit::log_action(
        &state.db,
        Some(school_id),
        Some(user_id),
        "BULK_CSV_UPLOAD_STAFF",
        Some(&target_role),
        None,
        Some(serde_json::json!({"success": final_success, "failed": parse_result.failed_count + failed_records_extra.len()})),
        Some(addr.to_string())
    ).await.ok();

    // Mark Job as COMPLETED
    if let Some(mut j) = state.jobs.get_mut(&job_id) {
        j.status = crate::utils::jobs::JobStatus::COMPLETED;
        j.processed_rows = parse_result.valid_rows.len();
        j.success_count = final_success;
        j.failed_count = parse_result.failed_count + failed_records_extra.len();
        j.failed_csv = final_failed_csv;
        j.updated_at = time::OffsetDateTime::now_utc().unix_timestamp();
        crate::utils::jobs::save_job_to_db(&state.db, &j, None).await;
    }

    tracing::info!(
        "Async staff bulk upload job {} completed: {} succeeded, {} skipped/failed",
        job_id,
        final_success,
        parse_result.failed_count + failed_records_extra.len()
    );
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
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SUPER_ADMIN".to_string()) && !user.is_school_admin() {
        crate::utils::telemetry::dispatch_security_alert(
            &state.db,
            user.school_id,
            user.id,
            "SINGLE_ADD_STAFF_ATTEMPT",
            "single_add_staff",
            "Only School Admins or Super Admins can add staff",
            &user.roles,
            Some(addr.to_string()),
        )
        .await;
        return Err(AppError::Forbidden(
            "Only School Admins or Super Admins can add staff".into(),
        ));
    }

    let first_name = payload.get("First Name").cloned().unwrap_or_default();
    let last_name = payload.get("Last Name").cloned().unwrap_or_default();
    let email = payload.get("Email").cloned().unwrap_or_default();
    let raw_phone = payload.get("Phone").map(|v| v.as_str()).unwrap_or_default();
    let phone = crate::utils::phone::normalize_phone_number(raw_phone);
    if phone.is_empty() || phone.len() < 10 {
        return Err(AppError::BadRequest(
            "Phone number is required and must be valid (+91<10_digits>)".into(),
        ));
    }

    let phone_exists =
        crate::modules::users::repository::UserRepository::check_phone_exists_in_school(
            &state.db,
            user.school_id,
            &phone,
        )
        .await?;

    if phone_exists {
        return Err(AppError::BadRequest(
            "Phone number is already registered for another user in this school".into(),
        ));
    }

    let emp_id = payload.get("Employee ID").cloned().unwrap_or_default();
    let joining_date_str = payload
        .get("Joining Date (YYYY-MM-DD)")
        .cloned()
        .unwrap_or_default();
    let department = payload.get("Department").cloned();

    let joining_date = crate::utils::datetime::DateUtils::parse_iso_date(&joining_date_str)
        .map_err(|e| AppError::BadRequest(e))?;

    let default_pass = crate::utils::password::hash_password("Welcome123!").unwrap();
    let username = format!("{}{}", first_name.to_lowercase(), emp_id);

    let email_lower = email.to_lowercase();
    let _email_lock = state
        .key_locks
        .acquire_lock(&(user.school_id, format!("user:email:{}", email_lower)))
        .await;
    let _phone_lock = state
        .key_locks
        .acquire_lock(&(user.school_id, format!("user:phone:{}", phone)))
        .await;

    let mut tx_guard = crate::utils::write_controller::DbWriteController::begin(&state.db).await?;
    crate::modules::users::repository::UserRepository::create_staff_user(
        tx_guard.conn(),
        user.school_id,
        &first_name,
        &last_name,
        &email,
        &phone,
        &username,
        &default_pass,
        &emp_id,
        joining_date,
        department,
        &query.role,
        user.id,
        false,
    )
    .await?;
    tx_guard.commit().await?;

    crate::utils::audit::log_action(
        &state.db,
        Some(user.school_id),
        Some(user.id),
        "SINGLE_ADD_STAFF",
        None,
        None,
        None,
        Some(addr.to_string()),
    )
    .await
    .ok();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"message": "Staff added successfully"})),
    ))
}
