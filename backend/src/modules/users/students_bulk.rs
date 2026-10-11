use crate::utils::csv_engine::{ColumnSchema, ColumnType, CsvSchema};
use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct StudentPreviewItem {
    pub first_name: String,
    pub last_name: String,
    pub admission_number: String,
    pub date_of_birth: String,
    pub guardian_first_name: String,
    pub guardian_last_name: String,
    pub guardian_email: String,
    pub guardian_phone: String,
    pub student_phone: Option<String>,
    pub class_name: Option<String>,
    pub section_name: Option<String>,
    pub class_section_id: Option<String>,
    pub is_duplicate: bool,
    pub error_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StudentPreviewError {
    pub row: usize,
    pub admission_number: String,
    pub error_reason: String,
}

fn parse_flexible_date(date_str: &str) -> Result<time::Date, String> {
    crate::utils::datetime::DateUtils::parse_iso_date(date_str)
}

pub async fn preview_bulk_students(
    auth_session: AuthSession,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        crate::utils::telemetry::dispatch_security_alert(
            &state.db,
            user.school_id,
            user.id,
            "CSV_PREVIEW_ATTEMPT",
            "students_bulk_preview",
            "Only School Admins can perform CSV preview",
            &user.roles,
            None,
        )
        .await;
        return Err(AppError::Forbidden(
            "Only School Admins can perform CSV preview".into(),
        ));
    }

    let schema = CsvSchema {
        columns: vec![
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
                header: "Admission Number",
                col_type: ColumnType::String,
                required: true,
                min_length: Some(1),
                max_length: Some(50),
            },
            ColumnSchema {
                header: "Date of Birth",
                col_type: ColumnType::String,
                required: true,
                min_length: Some(1),
                max_length: Some(50),
            },
            ColumnSchema {
                header: "Guardian First Name",
                col_type: ColumnType::String,
                required: true,
                min_length: Some(1),
                max_length: Some(100),
            },
            ColumnSchema {
                header: "Guardian Last Name",
                col_type: ColumnType::String,
                required: true,
                min_length: Some(1),
                max_length: Some(100),
            },
            ColumnSchema {
                header: "Guardian Email",
                col_type: ColumnType::Email,
                required: true,
                min_length: None,
                max_length: None,
            },
            ColumnSchema {
                header: "Guardian Phone",
                col_type: ColumnType::Phone,
                required: true,
                min_length: None,
                max_length: None,
            },
            ColumnSchema {
                header: "Student Phone",
                col_type: ColumnType::Phone,
                required: false,
                min_length: None,
                max_length: None,
            },
            ColumnSchema {
                header: "Class Section ID",
                col_type: ColumnType::String,
                required: false,
                min_length: None,
                max_length: Some(100),
            },
        ],
    };

    let mut raw_data = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Preview multipart error: {:?}", e);
        AppError::BadRequest("Multipart form read error".into())
    })? {
        if field.name() == Some("file") {
            raw_data = field
                .bytes()
                .await
                .map_err(|e| {
                    tracing::error!("Preview file bytes error: {:?}", e);
                    AppError::BadRequest("Failed to read file".into())
                })?
                .to_vec();
            break;
        }
    }

    if raw_data.is_empty() {
        return Err(AppError::BadRequest("No CSV file provided".into()));
    }

    let parse_result = schema.parse(&raw_data).map_err(|e| {
        tracing::error!("CSV Schema parsing error: {}", e);
        AppError::BadRequest(e)
    })?;

    let existing_adms: std::collections::HashSet<String> = sqlx::query_scalar!(
        "SELECT UPPER(admission_number) FROM users WHERE school_id = $1 AND admission_number IS NOT NULL",
        user.school_id
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .flatten()
    .collect();

    // Query active class sections & classes for school to validate existence
    let class_sections_db: Vec<(uuid::Uuid, String, String, String, String)> = sqlx::query_as(
        r#"SELECT cs.id, c.name, c.code, s.name, s.code
           FROM class_sections cs
           JOIN classes c ON cs.class_id = c.id
           JOIN sections s ON cs.section_id = s.id
           JOIN academic_years ay ON cs.academic_year_id = ay.id
           WHERE c.school_id = $1 AND ay.status = 'ACTIVE'"#,
    )
    .bind(user.school_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut cs_lookup_map: std::collections::HashMap<String, (uuid::Uuid, String, String)> =
        std::collections::HashMap::new();
    for (cs_id, cname, ccode, sname, scode) in &class_sections_db {
        let val = (*cs_id, cname.clone(), sname.clone());
        cs_lookup_map.insert(cs_id.to_string().to_lowercase(), val.clone());

        let ccode_upper = ccode.trim().to_uppercase();
        let scode_upper = scode.trim().to_uppercase();
        let cname_upper = cname.trim().to_uppercase();
        let sname_upper = sname.trim().to_uppercase();

        cs_lookup_map.insert(format!("{}-{}", ccode_upper, scode_upper), val.clone());
        cs_lookup_map.insert(format!("{}:{}", ccode_upper, scode_upper), val.clone());
        cs_lookup_map.insert(format!("{}{}", ccode_upper, scode_upper), val.clone());
        cs_lookup_map.insert(format!("{}-{}", cname_upper, sname_upper), val.clone());
        cs_lookup_map.insert(format!("{}:{}", cname_upper, sname_upper), val.clone());
        cs_lookup_map.insert(format!("{} {}", cname_upper, sname_upper), val.clone());
    }

    let mut preview_items = Vec::new();
    let mut errors = Vec::new();
    let mut seen_in_file = std::collections::HashSet::new();

    for (idx, row) in parse_result.valid_rows.iter().enumerate() {
        let first = row.get("First Name").cloned().unwrap_or_default();
        let last = row.get("Last Name").cloned().unwrap_or_default();
        let adm = row.get("Admission Number").cloned().unwrap_or_default();
        let dob = row
            .get("Date of Birth")
            .cloned()
            .or_else(|| row.get("Date of Birth (YYYY-MM-DD)").cloned())
            .unwrap_or_default();
        let g_first = row.get("Guardian First Name").cloned().unwrap_or_default();
        let g_last = row.get("Guardian Last Name").cloned().unwrap_or_default();
        let g_email = row.get("Guardian Email").cloned().unwrap_or_default();
        let g_phone_raw = row.get("Guardian Phone").cloned().unwrap_or_default();
        let g_phone = crate::utils::phone::normalize_phone_number(&g_phone_raw);
        let s_phone_raw = row.get("Student Phone").cloned();
        let s_phone = s_phone_raw
            .map(|p| crate::utils::phone::normalize_phone_number(&p))
            .filter(|p| !p.is_empty());
        let cls_section_id_raw = row
            .get("Class Section ID")
            .cloned()
            .or_else(|| row.get("Class Section Id").cloned())
            .or_else(|| row.get("Class Section Code").cloned())
            .or_else(|| row.get("Class ID").cloned())
            .or_else(|| row.get("Class Id").cloned());

        let mut row_error: Option<String> = None;
        let mut resolved_cls_name: Option<String> = None;
        let mut resolved_sec_name: Option<String> = None;

        // Validate Date format
        if let Err(date_err) = parse_flexible_date(&dob) {
            errors.push(StudentPreviewError {
                row: idx + 1,
                admission_number: adm.clone(),
                error_reason: date_err.clone(),
            });
            row_error = Some(date_err);
        }

        // Resolve Class Section ID / Code if exists
        if let Some(ref cs_id_str) = cls_section_id_raw {
            let trimmed = cs_id_str.trim();
            if !trimmed.is_empty() {
                let key_lower = trimmed.to_lowercase();
                let key_upper = trimmed.to_uppercase();

                if let Some((_cs_id, cname, sname)) = cs_lookup_map
                    .get(&key_lower)
                    .or_else(|| cs_lookup_map.get(&key_upper))
                {
                    resolved_cls_name = Some(cname.clone());
                    resolved_sec_name = Some(sname.clone());
                }
            }
        }

        let adm_upper = adm.trim().to_uppercase();
        let is_db_dup = existing_adms.contains(&adm_upper);
        let is_csv_dup = !seen_in_file.insert(adm_upper);

        if is_db_dup || is_csv_dup {
            let dup_msg = if is_db_dup {
                "Duplicate admission number already exists in database".to_string()
            } else {
                "Duplicate admission number repeated in this CSV file".to_string()
            };
            errors.push(StudentPreviewError {
                row: idx + 1,
                admission_number: adm.clone(),
                error_reason: dup_msg.clone(),
            });
            row_error = Some(match row_error {
                Some(existing_err) => format!("{}; {}", existing_err, dup_msg),
                None => dup_msg,
            });
        }

        let is_row_error = row_error.is_some();

        preview_items.push(StudentPreviewItem {
            first_name: first,
            last_name: last,
            admission_number: adm,
            date_of_birth: dob,
            guardian_first_name: g_first,
            guardian_last_name: g_last,
            guardian_email: g_email,
            guardian_phone: g_phone,
            student_phone: s_phone,
            class_name: resolved_cls_name,
            section_name: resolved_sec_name,
            class_section_id: cls_section_id_raw,
            is_duplicate: is_row_error,
            error_reason: row_error,
        });
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "total_rows": preview_items.len(),
            "valid_rows_count": preview_items.iter().filter(|i| !i.is_duplicate).count(),
            "error_rows_count": errors.len(),
            "preview": preview_items,
            "errors": errors,
        })),
    ))
}

pub async fn bulk_upload_students(
    auth_session: AuthSession,
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    // ENFORCE STRICT ROLE BOUNDARIES: Only SCHOOL_ADMIN can bulk upload
    if !user.roles.contains(&"SCHOOL_ADMIN".to_string()) {
        crate::utils::telemetry::dispatch_security_alert(
            &state.db,
            user.school_id,
            user.id,
            "BULK_STUDENT_UPLOAD_ATTEMPT",
            "students_bulk_upload",
            "Only School Admins can perform bulk CSV uploads",
            &user.roles,
            Some(addr.to_string()),
        )
        .await;
        return Err(AppError::Forbidden(
            "Only School Admins can perform bulk CSV uploads".into(),
        ));
    }

    let schema = CsvSchema {
        columns: vec![
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
                header: "Admission Number",
                col_type: ColumnType::String,
                required: true,
                min_length: Some(1),
                max_length: Some(50),
            },
            ColumnSchema {
                header: "Date of Birth",
                col_type: ColumnType::String,
                required: true,
                min_length: Some(1),
                max_length: Some(50),
            },
            ColumnSchema {
                header: "Guardian First Name",
                col_type: ColumnType::String,
                required: true,
                min_length: Some(1),
                max_length: Some(100),
            },
            ColumnSchema {
                header: "Guardian Last Name",
                col_type: ColumnType::String,
                required: true,
                min_length: Some(1),
                max_length: Some(100),
            },
            ColumnSchema {
                header: "Guardian Email",
                col_type: ColumnType::Email,
                required: true,
                min_length: None,
                max_length: None,
            },
            ColumnSchema {
                header: "Guardian Phone",
                col_type: ColumnType::Phone,
                required: true,
                min_length: None,
                max_length: None,
            },
            ColumnSchema {
                header: "Student Phone",
                col_type: ColumnType::Phone,
                required: false,
                min_length: None,
                max_length: None,
            },
            ColumnSchema {
                header: "Class",
                col_type: ColumnType::String,
                required: false,
                min_length: None,
                max_length: Some(100),
            },
            ColumnSchema {
                header: "Section",
                col_type: ColumnType::String,
                required: false,
                min_length: None,
                max_length: Some(50),
            },
        ],
    };

    let mut raw_data = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Bulk upload multipart error: {:?}", e);
        AppError::BadRequest("Multipart error".into())
    })? {
        if field.name() == Some("file") {
            raw_data = field
                .bytes()
                .await
                .map_err(|e| {
                    tracing::error!("Bulk upload file bytes error: {:?}", e);
                    AppError::BadRequest("Failed to read file".into())
                })?
                .to_vec();
            break;
        }
    }

    if raw_data.is_empty() {
        return Err(AppError::BadRequest("No CSV file provided".into()));
    }

    let parse_result = schema.parse(&raw_data).map_err(|e| {
        tracing::error!("Bulk upload CSV parse error: {}", e);
        AppError::BadRequest(e)
    })?;

    let job_id = uuid::Uuid::new_v4();
    let total_valid = parse_result.valid_rows.len();
    let now_ts = time::OffsetDateTime::now_utc().unix_timestamp();

    let initial_job = crate::utils::jobs::BulkUploadJob {
        id: job_id,
        school_id: user.school_id,
        user_id: user.id,
        job_type: "STUDENTS_BULK_UPLOAD".to_string(),
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

    tokio::spawn(async move {
        process_bulk_students_background(
            bg_state,
            bg_school_id,
            bg_user_id,
            job_id,
            parse_result,
            addr,
        )
        .await;
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": "Bulk upload job started",
            "job_id": job_id,
            "status": "PROCESSING",
            "total_rows": total_valid,
        })),
    ))
}

pub async fn get_student_bulk_upload_status(
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

async fn process_bulk_students_background(
    state: AppState,
    school_id: uuid::Uuid,
    user_id: uuid::Uuid,
    job_id: uuid::Uuid,
    parse_result: crate::utils::csv_engine::CsvParseResult,
    addr: std::net::SocketAddr,
) {
    let mut final_success = 0;

    let active_ay_id = sqlx::query_scalar!(
        "SELECT id FROM academic_years WHERE school_id = $1 AND status = 'ACTIVE' ORDER BY created_at DESC LIMIT 1",
        school_id
    ).fetch_optional(&state.db).await.unwrap_or(None);

    // Pre-fetch all existing admission numbers in one single query
    let mut existing_adms =
        crate::modules::users::repository::UserRepository::get_existing_admission_numbers(
            &state.db, school_id,
        )
        .await
        .unwrap_or_default();

    // Query active class sections & classes for school to validate existence in background worker
    let class_sections_db: Vec<(uuid::Uuid, String, String, String, String)> = sqlx::query_as(
        r#"SELECT cs.id, UPPER(c.name), UPPER(c.code), UPPER(s.name), UPPER(s.code)
           FROM class_sections cs
           JOIN classes c ON cs.class_id = c.id
           JOIN sections s ON cs.section_id = s.id
           JOIN academic_years ay ON cs.academic_year_id = ay.id
           WHERE c.school_id = $1 AND ay.status = 'ACTIVE'"#,
    )
    .bind(school_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut cs_lookup_map: std::collections::HashMap<String, uuid::Uuid> =
        std::collections::HashMap::new();
    for (cs_id, cname, ccode, sname, scode) in &class_sections_db {
        cs_lookup_map.insert(cs_id.to_string().to_lowercase(), *cs_id);

        let ccode_upper = ccode.trim().to_uppercase();
        let scode_upper = scode.trim().to_uppercase();
        let cname_upper = cname.trim().to_uppercase();
        let sname_upper = sname.trim().to_uppercase();

        cs_lookup_map.insert(format!("{}-{}", ccode_upper, scode_upper), *cs_id);
        cs_lookup_map.insert(format!("{}:{}", ccode_upper, scode_upper), *cs_id);
        cs_lookup_map.insert(format!("{}{}", ccode_upper, scode_upper), *cs_id);
        cs_lookup_map.insert(format!("{}-{}", cname_upper, sname_upper), *cs_id);
        cs_lookup_map.insert(format!("{}:{}", cname_upper, sname_upper), *cs_id);
        cs_lookup_map.insert(format!("{} {}", cname_upper, sname_upper), *cs_id);
    }

    let default_pass = crate::utils::password::hash_password("Welcome123!").unwrap_or_default();

    let mut failed_records_extra: Vec<Vec<String>> = Vec::new();
    let err_headers = vec![
        "First Name",
        "Last Name",
        "Admission Number",
        "Date of Birth",
        "Guardian First Name",
        "Guardian Last Name",
        "Guardian Email",
        "Guardian Phone",
        "Student Phone",
        "Class Section ID",
        "Error_Reason",
    ];

    for (idx, row) in parse_result.valid_rows.iter().enumerate() {
        let first = match row.get("First Name") {
            Some(v) => v,
            None => continue,
        };
        let last = match row.get("Last Name") {
            Some(v) => v,
            None => continue,
        };
        let adm = match row.get("Admission Number") {
            Some(v) => v,
            None => continue,
        };
        let dob_str = match row
            .get("Date of Birth")
            .or_else(|| row.get("Date of Birth (YYYY-MM-DD)"))
        {
            Some(v) => v,
            None => continue,
        };
        let g_first = match row.get("Guardian First Name") {
            Some(v) => v,
            None => continue,
        };
        let g_last = match row.get("Guardian Last Name") {
            Some(v) => v,
            None => continue,
        };
        let g_email = match row.get("Guardian Email") {
            Some(v) => v,
            None => continue,
        };
        let g_phone_raw = match row.get("Guardian Phone") {
            Some(v) => v,
            None => continue,
        };
        let g_phone = crate::utils::phone::normalize_phone_number(g_phone_raw);
        let s_phone_raw = row.get("Student Phone").cloned().unwrap_or_default();
        let s_phone_norm = crate::utils::phone::normalize_phone_number(&s_phone_raw);
        let s_phone = if s_phone_norm.is_empty() {
            None
        } else {
            Some(s_phone_norm)
        };
        let cls_section_id_raw = row
            .get("Class Section ID")
            .cloned()
            .or_else(|| row.get("Class Section Id").cloned())
            .or_else(|| row.get("Class Section Code").cloned())
            .or_else(|| row.get("Class ID").cloned())
            .or_else(|| row.get("Class Id").cloned());

        let cs_id_str = cls_section_id_raw.clone().unwrap_or_default();

        let dob = match parse_flexible_date(dob_str) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Row skipped: invalid date for admission {}: {}", adm, e);
                failed_records_extra.push(vec![
                    first.clone(),
                    last.clone(),
                    adm.clone(),
                    dob_str.clone(),
                    g_first.clone(),
                    g_last.clone(),
                    g_email.clone(),
                    g_phone.clone(),
                    s_phone_raw.clone(),
                    cs_id_str,
                    "Invalid Date format (Use YYYY-MM-DD or DD/MM/YYYY)".to_string(),
                ]);
                continue;
            }
        };

        let adm_upper = adm.trim().to_uppercase();

        if existing_adms.contains(&adm_upper) {
            tracing::info!(
                "Row skipped: duplicate admission number {} already in database/batch",
                adm
            );
            failed_records_extra.push(vec![
                first.clone(),
                last.clone(),
                adm.clone(),
                dob_str.clone(),
                g_first.clone(),
                g_last.clone(),
                g_email.clone(),
                g_phone.clone(),
                s_phone_raw.clone(),
                cs_id_str,
                "Duplicate Admission Number".to_string(),
            ]);
            if let Some(mut j) = state.jobs.get_mut(&job_id) {
                j.processed_rows = idx + 1;
                j.success_count = final_success;
                j.failed_count = parse_result.failed_count + failed_records_extra.len();
                j.updated_at = time::OffsetDateTime::now_utc().unix_timestamp();
            }
            continue;
        }

        let mut pre_cs_id: Option<uuid::Uuid> = None;

        if let Some(ref cs_str) = cls_section_id_raw {
            let trimmed = cs_str.trim();
            if !trimmed.is_empty() {
                let key_lower = trimmed.to_lowercase();
                let key_upper = trimmed.to_uppercase();

                if let Some(&found_cs_id) = cs_lookup_map
                    .get(&key_lower)
                    .or_else(|| cs_lookup_map.get(&key_upper))
                {
                    pre_cs_id = Some(found_cs_id);
                }
            }
        }

        let mut tx_guard =
            match crate::utils::write_controller::DbWriteController::begin(&state.db).await {
                Ok(tx) => tx,
                Err(e) => {
                    tracing::error!("Failed to begin transaction for adm {}: {:?}", adm, e);
                    failed_records_extra.push(vec![
                        first.clone(),
                        last.clone(),
                        adm.clone(),
                        dob_str.clone(),
                        g_first.clone(),
                        g_last.clone(),
                        g_email.clone(),
                        g_phone.clone(),
                        s_phone_raw.clone(),
                        cs_id_str,
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

        let create_res =
            crate::modules::users::repository::UserRepository::create_bulk_student_item(
                tx_guard.conn(),
                school_id,
                user_id,
                &default_pass,
                first,
                last,
                adm,
                dob,
                g_first,
                g_last,
                g_email,
                &g_phone,
                s_phone.as_deref(),
                None,
                None,
                active_ay_id,
                pre_cs_id,
            )
            .await;

        match create_res {
            Ok(_) => {
                if let Err(e) = tx_guard.commit().await {
                    tracing::error!("DB Commit Error for adm {}: {:?}", adm, e);
                    failed_records_extra.push(vec![
                        first.clone(),
                        last.clone(),
                        adm.clone(),
                        dob_str.clone(),
                        g_first.clone(),
                        g_last.clone(),
                        g_email.clone(),
                        g_phone.clone(),
                        s_phone_raw.clone(),
                        cs_id_str,
                        format!("DB Commit Error: {}", e),
                    ]);
                } else {
                    existing_adms.insert(adm_upper);
                    final_success += 1;
                }
            }
            Err(e) => {
                let _ = tx_guard.rollback().await;
                tracing::error!("Failed to create student for adm {}: {:?}", adm, e);
                failed_records_extra.push(vec![
                    first.clone(),
                    last.clone(),
                    adm.clone(),
                    dob_str.clone(),
                    g_first.clone(),
                    g_last.clone(),
                    g_email.clone(),
                    g_phone.clone(),
                    s_phone_raw.clone(),
                    cs_id_str,
                    format!("Student creation error: {}", e),
                ]);
            }
        }

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
        "BULK_CSV_UPLOAD",
        Some("STUDENTS"),
        None,
        Some(serde_json::json!({"success": final_success, "failed": parse_result.failed_count + failed_records_extra.len()})),
        Some(addr.to_string()),
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
        "Async bulk upload job {} completed: {} succeeded, {} skipped/failed",
        job_id,
        final_success,
        parse_result.failed_count + failed_records_extra.len()
    );
}
