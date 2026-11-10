use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct UpdateUserProfileRequest {
    pub display_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub date_of_birth: Option<String>,
    pub gender: Option<String>,
    pub blood_group: Option<String>,
    pub status: Option<String>,
    pub caste_category: Option<String>,
    pub custom_fields: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize)]
pub struct UpdateUserProfileResponse {
    pub success: bool,
    pub message: String,
    pub user_id: Uuid,
    pub status: String,
}

/// Controller: Updates user profile details and enforces activation readiness.
/// If status is being transitioned to 'ACTIVE', verifies that all essential details
/// (Full Name and Contact or Admission Number) are present before allowing activation.
pub async fn update_user_profile(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(target_user_id): Path<Uuid>,
    Json(payload): Json<UpdateUserProfileRequest>,
) -> Result<impl IntoResponse, AppError> {
    let auth_user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let is_admin = auth_user.is_school_admin()
        || auth_user
            .roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN"));

    // Non-admin can only edit their own profile, and cannot modify status
    if !is_admin && auth_user.id != target_user_id {
        return Err(AppError::Forbidden(
            "Unauthorized to edit other users' profiles".into(),
        ));
    }
    if !is_admin && payload.status.is_some() {
        return Err(AppError::Forbidden(
            "Only administrators can update account lifecycle status".into(),
        ));
    }

    #[derive(sqlx::FromRow)]
    #[allow(dead_code)]
    struct TargetUserRow {
        pub id: Uuid,
        pub school_id: Uuid,
        pub first_name: String,
        pub last_name: Option<String>,
        pub display_name: Option<String>,
        pub email: Option<String>,
        pub phone: Option<String>,
        pub admission_number: Option<String>,
        pub status: String,
    }

    // Verify target user belongs to the same school (tenant isolation)
    let target_user = sqlx::query_as::<_, TargetUserRow>(
        r#"SELECT id, school_id, first_name, last_name, display_name, email, phone, admission_number, status::TEXT as status
           FROM users
           WHERE id = $1"#,
    )
    .bind(target_user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    if target_user.school_id != auth_user.school_id
        && !auth_user.roles.contains(&"SUPER_ADMIN".to_string())
    {
        return Err(AppError::Forbidden(
            "User belongs to a different school".into(),
        ));
    }

    let mut final_status = target_user.status.clone();

    // Check status transition logic
    if let Some(ref req_status) = payload.status {
        let norm_status = req_status.trim().to_uppercase().replace(['-', ' '], "_");

        // ACTIVATION GUARD: If transitioning to ACTIVE, enforce essential details completeness!
        if norm_status == "ACTIVE" {
            let eff_display_name = payload
                .display_name
                .as_deref()
                .or(target_user.display_name.as_deref());
            let eff_first_name = payload
                .first_name
                .as_deref()
                .or(Some(&target_user.first_name));
            let eff_email = payload.email.as_deref().or(target_user.email.as_deref());
            let eff_phone = payload.phone.as_deref().or(target_user.phone.as_deref());
            let eff_adm = target_user.admission_number.as_deref();

            let has_name = eff_display_name.is_some_and(|s| !s.trim().is_empty())
                || eff_first_name.is_some_and(|s| !s.trim().is_empty());

            let has_contact_or_id = eff_email.is_some_and(|s| !s.trim().is_empty())
                || eff_phone.is_some_and(|s| !s.trim().is_empty())
                || eff_adm.is_some_and(|s| !s.trim().is_empty());

            if !has_name || !has_contact_or_id {
                return Err(AppError::BadRequest(
                    "Cannot activate user: essential details (Full Name and Contact or Admission Number) must be filled before activation.".into()
                ));
            }
            final_status = "ACTIVE".to_string();
        } else {
            final_status = norm_status;
        }
    }

    // Clean inputs
    let clean_display_name = payload
        .display_name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let clean_first_name = payload
        .first_name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let clean_last_name = payload
        .last_name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let clean_email = payload
        .email
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let clean_phone = payload
        .phone
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let clean_gender = payload
        .gender
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let clean_blood_group = payload
        .blood_group
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let clean_dob: Option<time::Date> = if let Some(ref d_str) = payload.date_of_birth {
        let trimmed = d_str.trim();
        if trimmed.is_empty() {
            None
        } else {
            crate::utils::datetime::DateUtils::parse_iso_date(trimmed).ok()
        }
    } else {
        None
    };

    // Synchronize first_name, last_name, and display_name
    let (inferred_first, inferred_last) = if clean_first_name.is_none() && clean_last_name.is_none()
    {
        if let Some(dn) = clean_display_name {
            let parts: Vec<&str> = dn.split_whitespace().collect();
            if parts.len() >= 2 {
                (Some(parts[0].to_string()), Some(parts[1..].join(" ")))
            } else if parts.len() == 1 {
                (Some(parts[0].to_string()), Some("-".to_string()))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let final_first_name = clean_first_name.map(|s| s.to_string()).or(inferred_first);
    let final_last_name = clean_last_name.map(|s| s.to_string()).or(inferred_last);

    let generated_display_name = match (&final_first_name, &final_last_name) {
        (Some(f), Some(l)) => Some(format!("{} {}", f, l).trim().to_string()),
        (Some(f), None) => Some(f.to_string()),
        (None, Some(l)) => Some(l.to_string()),
        (None, None) => None,
    };
    let final_display_name = clean_display_name
        .map(|s| s.to_string())
        .or(generated_display_name);

    let clean_caste = payload
        .caste_category
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let custom_values_json = payload
        .custom_fields
        .as_ref()
        .map(|cf| serde_json::to_value(cf).unwrap_or(serde_json::json!({})));

    // Update user in database
    sqlx::query(
        r#"UPDATE users
           SET
               first_name = COALESCE($1, first_name),
               last_name = COALESCE($2, last_name),
               display_name = COALESCE($3, display_name),
               email = COALESCE($4, email),
               phone = COALESCE($5, phone),
               gender = COALESCE($6, gender),
               blood_group = COALESCE($7, blood_group),
               date_of_birth = COALESCE($8, date_of_birth),
               caste_category = COALESCE($9, caste_category),
               custom_values = CASE
                   WHEN $10::jsonb IS NOT NULL THEN COALESCE(custom_values, '{}'::jsonb) || $10::jsonb
                   ELSE custom_values
               END,
               status = $11,
               updated_at = NOW(),
               updated_by_user_id = $12
           WHERE id = $13"#,
    )
    .bind(final_first_name)
    .bind(final_last_name)
    .bind(final_display_name)
    .bind(clean_email)
    .bind(clean_phone)
    .bind(clean_gender)
    .bind(clean_blood_group)
    .bind(clean_dob)
    .bind(clean_caste)
    .bind(&custom_values_json)
    .bind(&final_status)
    .bind(auth_user.id)
    .bind(target_user_id)
    .execute(&state.db)
    .await?;

    if let Some(ref cf_map) = payload.custom_fields {
        for (f_key, f_val) in cf_map {
            let val_str = match f_val {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            };
            let _ = sqlx::query(
                r#"INSERT INTO user_custom_values (school_id, user_id, field_id, value, updated_at)
                   SELECT $1, $2, id, $3, NOW()
                   FROM custom_fields
                   WHERE school_id = $1 AND field_key = $4
                   ON CONFLICT (user_id, field_id)
                   DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()"#,
            )
            .bind(auth_user.school_id)
            .bind(target_user_id)
            .bind(&val_str)
            .bind(f_key)
            .execute(&state.db)
            .await;
        }
    }

    crate::modules::auth::backend::invalidate_user_cache(&target_user_id);

    if final_status == "TC_ISSUED" {
        let _ = sqlx::query(
            r#"UPDATE enrollments
               SET status = 'INACTIVE', updated_at = NOW()
               WHERE student_id = $1 AND status = 'ACTIVE'"#,
        )
        .bind(target_user_id)
        .execute(&state.db)
        .await;
    } else if target_user.status.to_uppercase() == "TC_ISSUED"
        && matches!(
            final_status.as_str(),
            "ACTIVE" | "READMITTED" | "TC_CANCELLED"
        )
    {
        // 1. Formally revoke/cancel any active Transfer Certificate in issued_reports
        let cancel_reason = "Revoked upon student re-activation by Admin";
        let canceller_name = auth_user
            .display_name
            .clone()
            .unwrap_or_else(|| auth_user.username.clone());
        let _ = sqlx::query(
            r#"UPDATE issued_reports
               SET status = 'CANCELLED',
                   cancelled_by = $1,
                   cancelled_by_name = $2,
                   cancelled_at = NOW(),
                   cancellation_reason = $3,
                   remarks = COALESCE(remarks, '') || ' [' || $3 || ']',
                   updated_at = NOW()
               WHERE school_id = $4 AND student_id = $5 AND status = 'ISSUED'"#,
        )
        .bind(auth_user.id)
        .bind(canceller_name)
        .bind(cancel_reason)
        .bind(auth_user.school_id)
        .bind(target_user_id)
        .execute(&state.db)
        .await;

        // 2. Reactivate active class enrollment for ACTIVE or READMITTED
        if final_status == "ACTIVE" || final_status == "READMITTED" {
            let _ = sqlx::query(
                r#"UPDATE enrollments
                   SET status = 'ACTIVE', updated_at = NOW()
                   WHERE student_id = $1 AND id = (
                       SELECT id FROM enrollments WHERE student_id = $1 ORDER BY updated_at DESC LIMIT 1
                   )"#,
            )
            .bind(target_user_id)
            .execute(&state.db)
            .await;
        }
    }

    let _ = crate::utils::audit::log_action(
        &state.db,
        Some(auth_user.school_id),
        Some(auth_user.id),
        "UPDATE_USER_PROFILE",
        Some("USERS"),
        Some(target_user_id),
        Some(serde_json::json!({
            "target_user_id": target_user_id,
            "status": final_status,
            "display_name": clean_display_name,
            "first_name": clean_first_name,
            "last_name": clean_last_name,
            "email": clean_email,
            "phone": clean_phone
        })),
        None,
    )
    .await;

    crate::modules::users::repository::UserRepository::invalidate_users_cache(Some(
        auth_user.school_id,
    ));

    Ok((
        StatusCode::OK,
        Json(UpdateUserProfileResponse {
            success: true,
            message: if final_status == "ACTIVE" {
                "User profile updated and account activated successfully!".into()
            } else {
                "User profile updated successfully".into()
            },
            user_id: target_user_id,
            status: final_status,
        }),
    ))
}

/// Controller: Quick activation endpoint for a user.
/// Checks essential completeness and immediately sets status to ACTIVE.
pub async fn activate_user(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(target_user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    update_user_profile(
        auth_session,
        State(state),
        Path(target_user_id),
        Json(UpdateUserProfileRequest {
            display_name: None,
            first_name: None,
            last_name: None,
            email: None,
            phone: None,
            date_of_birth: None,
            gender: None,
            blood_group: None,
            status: Some("ACTIVE".to_string()),
            caste_category: None,
            custom_fields: None,
        }),
    )
    .await
}

fn deserialize_optional_uuid<'de, D>(deserializer: D) -> Result<Option<Uuid>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    match opt {
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty()
                || trimmed.eq_ignore_ascii_case("null")
                || trimmed.eq_ignore_ascii_case("undefined")
            {
                Ok(None)
            } else {
                Uuid::parse_str(trimmed)
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
        }
        Some(serde_json::Value::Null) | None => Ok(None),
        Some(v) => serde_json::from_value(v)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Deserialize)]
pub struct BulkUpdateItemRequest {
    pub id: String,
    pub display_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub date_of_birth: Option<String>,
    pub gender: Option<String>,
    pub blood_group: Option<String>,
    pub status: Option<String>,
    pub admission_number: Option<String>,
    pub roll_number: Option<String>,
    pub class_name: Option<String>,
    pub section_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_uuid")]
    pub class_section_id: Option<Uuid>,
    pub caste_category: Option<String>,
    pub custom_fields: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct BulkUpdateUsersRequest {
    pub users: Vec<BulkUpdateItemRequest>,
}

#[derive(Debug, Serialize)]
pub struct BulkUpdateUsersResponse {
    pub success: bool,
    pub updated_count: usize,
    pub message: String,
}

/// Controller: Bulk updates multiple users (used by Admission Register Spreadsheet).
/// Validates tenant isolation, updates core attributes, and rolls back atomically on error.
pub async fn bulk_update_users(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<BulkUpdateUsersRequest>,
) -> Result<impl IntoResponse, AppError> {
    let auth_user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let is_admin = auth_user.is_school_admin()
        || auth_user
            .roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN"));

    if !is_admin {
        return Err(AppError::Forbidden(
            "Only administrators can perform bulk updates".into(),
        ));
    }

    if payload.users.is_empty() {
        return Ok((
            StatusCode::OK,
            Json(BulkUpdateUsersResponse {
                success: true,
                updated_count: 0,
                message: "No users provided for update".into(),
            }),
        ));
    }

    // 1. IN-MEMORY INTRA-BATCH VALIDATION (0 DB roundtrips)
    let mut seen_admissions: HashMap<String, String> = HashMap::new(); // UPPER(admission) -> item.id
    let mut seen_emails: HashMap<String, String> = HashMap::new(); // LOWER(email) -> item.id

    for item in &payload.users {
        if let Some(ref adm) = item.admission_number {
            let trimmed = adm.trim();
            if !trimmed.is_empty() {
                let upper = trimmed.to_uppercase();
                if let Some(prev_id) = seen_admissions.get(&upper) {
                    if prev_id != item.id.trim() {
                        return Err(AppError::BadRequest(format!(
                            "Duplicate admission number '{}' found multiple times within the submitted batch.",
                            trimmed
                        )));
                    }
                }
                seen_admissions.insert(upper, item.id.trim().to_string());
            }
        }

        if let Some(ref em) = item.email {
            let trimmed = em.trim();
            if !trimmed.is_empty() {
                let lower = trimmed.to_lowercase();
                if let Some(prev_id) = seen_emails.get(&lower) {
                    if prev_id != item.id.trim() {
                        return Err(AppError::BadRequest(format!(
                            "Duplicate email '{}' found multiple times within the submitted batch.",
                            trimmed
                        )));
                    }
                }
                seen_emails.insert(lower, item.id.trim().to_string());
            }
        }
    }

    let mut tx = state.db.begin().await?;
    let mut updated_count = 0;

    let active_ay_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM academic_years WHERE school_id = $1 AND status = 'ACTIVE' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(auth_user.school_id)
    .fetch_optional(&mut *tx)
    .await?;

    let is_super_admin = auth_user
        .roles
        .iter()
        .any(|r| r.eq_ignore_ascii_case("SUPER_ADMIN"));

    // 2. BULK PRE-FETCH: Tenant boundary & existence check
    let parsed_uids: Vec<Uuid> = payload
        .users
        .iter()
        .filter_map(|u| Uuid::parse_str(u.id.trim()).ok())
        .collect();

    let valid_user_ids: HashSet<Uuid> = if !parsed_uids.is_empty() {
        let rows: Vec<(Uuid,)> =
            sqlx::query_as("SELECT id FROM users WHERE id = ANY($1) AND (school_id = $2 OR $3)")
                .bind(&parsed_uids)
                .bind(auth_user.school_id)
                .bind(is_super_admin)
                .fetch_all(&mut *tx)
                .await?;
        rows.into_iter().map(|r| r.0).collect()
    } else {
        HashSet::new()
    };

    // 3. BULK PRE-CHECK: Database collisions for admission numbers & emails
    let batch_adms: Vec<String> = seen_admissions.keys().cloned().collect();
    let batch_emails: Vec<String> = seen_emails.keys().cloned().collect();

    if !batch_adms.is_empty() || !batch_emails.is_empty() {
        #[derive(sqlx::FromRow)]
        struct ConflictRow {
            id: Uuid,
            admission_number: Option<String>,
            email: Option<String>,
        }

        let conflicts: Vec<ConflictRow> = sqlx::query_as(
            r#"SELECT id, admission_number, email 
               FROM users 
               WHERE school_id = $1 
                 AND (
                   (admission_number IS NOT NULL AND UPPER(admission_number) = ANY($2))
                   OR
                   (email IS NOT NULL AND LOWER(email) = ANY($3))
                 )"#,
        )
        .bind(auth_user.school_id)
        .bind(&batch_adms)
        .bind(&batch_emails)
        .fetch_all(&mut *tx)
        .await?;

        for conflict in conflicts {
            if let Some(ref db_adm) = conflict.admission_number {
                let db_adm_upper = db_adm.trim().to_uppercase();
                if let Some(target_item_id_str) = seen_admissions.get(&db_adm_upper) {
                    let is_same = Uuid::parse_str(target_item_id_str)
                        .ok()
                        .map(|uid| uid == conflict.id)
                        .unwrap_or(false);
                    if !is_same {
                        return Err(AppError::BadRequest(format!(
                            "Admission number '{}' is already assigned to another student in this school.",
                            db_adm.trim()
                        )));
                    }
                }
            }

            if let Some(ref db_em) = conflict.email {
                let db_em_lower = db_em.trim().to_lowercase();
                if let Some(target_item_id_str) = seen_emails.get(&db_em_lower) {
                    let is_same = Uuid::parse_str(target_item_id_str)
                        .ok()
                        .map(|uid| uid == conflict.id)
                        .unwrap_or(false);
                    if !is_same {
                        return Err(AppError::BadRequest(format!(
                            "Email '{}' is already assigned to another user in this school.",
                            db_em.trim()
                        )));
                    }
                }
            }
        }
    }

    // 4. BULK PRE-FETCH: User metadata (class_section_id, student role)
    #[derive(sqlx::FromRow)]
    struct UserMetaRow {
        id: Uuid,
        class_section_id: Option<Uuid>,
        is_student: bool,
    }

    let user_meta_map: HashMap<Uuid, UserMetaRow> = if !parsed_uids.is_empty() {
        let rows: Vec<UserMetaRow> = sqlx::query_as(
            r#"SELECT 
                u.id, 
                u.class_section_id,
                (u.admission_number IS NOT NULL OR EXISTS (
                    SELECT 1 FROM user_roles ur 
                    JOIN roles r ON ur.role_id = r.id 
                    WHERE ur.user_id = u.id AND r.code = 'STUDENT'
                )) AS is_student
               FROM users u
               WHERE u.id = ANY($1)"#,
        )
        .bind(&parsed_uids)
        .fetch_all(&mut *tx)
        .await?;
        rows.into_iter().map(|r| (r.id, r)).collect()
    } else {
        HashMap::new()
    };

    // 5. BULK PRE-FETCH: Active enrollments
    #[derive(sqlx::FromRow)]
    struct EnrollmentMetaRow {
        id: Uuid,
        student_id: Uuid,
    }

    let enrollment_map: HashMap<Uuid, EnrollmentMetaRow> = if let Some(ay_id) = active_ay_id {
        if !parsed_uids.is_empty() {
            let rows: Vec<EnrollmentMetaRow> = sqlx::query_as(
                r#"SELECT id, student_id
                   FROM enrollments
                   WHERE academic_year_id = $1 AND student_id = ANY($2) AND status = 'ACTIVE'"#,
            )
            .bind(ay_id)
            .bind(&parsed_uids)
            .fetch_all(&mut *tx)
            .await?;
            rows.into_iter().map(|r| (r.student_id, r)).collect()
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    // 6. BULK PRE-FETCH: Valid class_section_ids passed directly in payload
    let raw_cs_ids: Vec<Uuid> = payload
        .users
        .iter()
        .filter_map(|u| u.class_section_id)
        .collect();

    let valid_cs_ids: HashSet<Uuid> = if !raw_cs_ids.is_empty() {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"SELECT cs.id 
               FROM class_sections cs 
               JOIN classes c ON c.id = cs.class_id 
               WHERE cs.id = ANY($1) AND c.school_id = $2"#,
        )
        .bind(&raw_cs_ids)
        .bind(auth_user.school_id)
        .fetch_all(&mut *tx)
        .await?;
        rows.into_iter().map(|r| r.0).collect()
    } else {
        HashSet::new()
    };

    // 7. IN-MEMORY CACHES FOR CLASS / SECTION RESOLUTION
    let mut class_cache: HashMap<String, Uuid> = HashMap::new();
    let mut section_cache: HashMap<String, Uuid> = HashMap::new();
    let mut cs_cache: HashMap<(Uuid, Uuid), Uuid> = HashMap::new();
    let mut student_role_id_cache: Option<Uuid> = None;

    for item in &payload.users {
        let parsed_uuid = Uuid::parse_str(item.id.trim()).ok();

        let clean_display_name = item
            .display_name
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let clean_first_name = item
            .first_name
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let clean_last_name = item
            .last_name
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let clean_email = item
            .email
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let clean_phone = item
            .phone
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let clean_gender = item
            .gender
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let clean_blood_group = item
            .blood_group
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let clean_adm = item
            .admission_number
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        let clean_dob: Option<time::Date> = if let Some(ref d_str) = item.date_of_birth {
            let trimmed = d_str.trim();
            if trimmed.is_empty() {
                None
            } else {
                crate::utils::datetime::DateUtils::parse_iso_date(trimmed).ok()
            }
        } else {
            None
        };

        let clean_caste = item
            .caste_category
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        let custom_values_json = item
            .custom_fields
            .as_ref()
            .map(|cf| serde_json::to_value(cf).unwrap_or(serde_json::json!({})));

        let status_val: Option<String> = item
            .status
            .as_deref()
            .map(|s| s.trim().to_uppercase().replace(['-', ' '], "_"))
            .filter(|s| !s.is_empty());

        let (inferred_first, inferred_last) =
            if clean_first_name.is_none() && clean_last_name.is_none() {
                if let Some(dn) = clean_display_name {
                    let parts: Vec<&str> = dn.split_whitespace().collect();
                    if parts.len() >= 2 {
                        (Some(parts[0].to_string()), Some(parts[1..].join(" ")))
                    } else if parts.len() == 1 {
                        (Some(parts[0].to_string()), Some("-".to_string()))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        let final_first_name = clean_first_name.map(|s| s.to_string()).or(inferred_first);
        let final_last_name = clean_last_name.map(|s| s.to_string()).or(inferred_last);

        let generated_display_name = match (&final_first_name, &final_last_name) {
            (Some(f), Some(l)) => Some(format!("{} {}", f, l).trim().to_string()),
            (Some(f), None) => Some(f.to_string()),
            (None, Some(l)) => Some(l.to_string()),
            (None, None) => None,
        };
        let final_display_name = clean_display_name
            .map(|s| s.to_string())
            .or(generated_display_name);

        let is_existing = parsed_uuid.is_some();
        let target_id = if let Some(uid) = parsed_uuid {
            if !valid_user_ids.contains(&uid) {
                continue;
            }
            uid
        } else {
            // If completely empty row was added, skip
            if final_first_name.is_none() && clean_adm.is_none() && item.roll_number.is_none() {
                continue;
            }

            let new_user_id = Uuid::new_v4();
            let new_first = final_first_name
                .clone()
                .unwrap_or_else(|| "Student".to_string());
            let new_last = final_last_name.clone().unwrap_or_else(|| "-".to_string());
            let base_username = clean_adm
                .unwrap_or("student")
                .to_lowercase()
                .replace([' ', '-'], "_");
            let new_username = format!("{}_{}", base_username, &new_user_id.to_string()[..6]);

            sqlx::query(
                r#"INSERT INTO users (
                    id, school_id, username, password_hash, first_name, last_name, display_name,
                    email, phone, gender, blood_group, date_of_birth, admission_number, caste_category,
                    custom_values, status,
                    email_verified, phone_verified, imported_via_csv, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, 'LOCKED_USER', $4, $5, $6,
                    $7, $8, $9, $10, $11, $12, $13,
                    COALESCE($14, '{}'::jsonb), COALESCE($15, 'ACTIVE'),
                    false, false, false, NOW(), NOW()
                )"#,
            )
            .bind(new_user_id)
            .bind(auth_user.school_id)
            .bind(&new_username)
            .bind(&new_first)
            .bind(&new_last)
            .bind(&final_display_name)
            .bind(clean_email)
            .bind(clean_phone)
            .bind(clean_gender)
            .bind(clean_blood_group)
            .bind(clean_dob)
            .bind(clean_adm)
            .bind(clean_caste)
            .bind(&custom_values_json)
            .bind(&status_val)
            .execute(&mut *tx)
            .await?;

            let student_role_id = match student_role_id_cache {
                Some(rid) => rid,
                None => {
                    let rid = crate::utils::roles::get_or_create_role_id(
                        &mut tx,
                        auth_user.school_id,
                        "STUDENT",
                        "Student",
                    )
                    .await?;
                    student_role_id_cache = Some(rid);
                    rid
                }
            };

            sqlx::query(
                "INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(new_user_id)
            .bind(student_role_id)
            .execute(&mut *tx)
            .await?;

            new_user_id
        };

        // Resolve class section if provided
        let mut target_cs_id: Option<Uuid> = None;

        if let (Some(c_name), Some(s_name), Some(ay_id)) = (
            item.class_name
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty()),
            item.section_name
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty()),
            active_ay_id,
        ) {
            let c_code = c_name.to_uppercase();
            let s_code = s_name.to_uppercase();

            let class_id = match class_cache.get(&c_code) {
                Some(&cid) => Some(cid),
                None => {
                    let mut cid: Option<Uuid> = sqlx::query_scalar(
                        "SELECT id FROM classes WHERE school_id = $1 AND (UPPER(name) = UPPER($2) OR UPPER(code) = UPPER($2)) LIMIT 1",
                    )
                    .bind(auth_user.school_id)
                    .bind(c_name)
                    .fetch_optional(&mut *tx)
                    .await?;

                    if cid.is_none() {
                        cid = sqlx::query_scalar(
                            "INSERT INTO classes (school_id, name, code, sequence) VALUES ($1, $2, $3, 1) ON CONFLICT (school_id, code) DO UPDATE SET name = EXCLUDED.name RETURNING id",
                        )
                        .bind(auth_user.school_id)
                        .bind(c_name)
                        .bind(&c_code)
                        .fetch_optional(&mut *tx)
                        .await?;
                    }
                    if let Some(c) = cid {
                        class_cache.insert(c_code, c);
                    }
                    cid
                }
            };

            let section_id = match section_cache.get(&s_code) {
                Some(&sid) => Some(sid),
                None => {
                    let mut sid: Option<Uuid> = sqlx::query_scalar(
                        "SELECT id FROM sections WHERE school_id = $1 AND (UPPER(name) = UPPER($2) OR UPPER(code) = UPPER($2)) LIMIT 1",
                    )
                    .bind(auth_user.school_id)
                    .bind(s_name)
                    .fetch_optional(&mut *tx)
                    .await?;

                    if sid.is_none() {
                        sid = sqlx::query_scalar(
                            "INSERT INTO sections (school_id, name, code) VALUES ($1, $2, $3) ON CONFLICT (school_id, code) DO UPDATE SET name = EXCLUDED.name RETURNING id",
                        )
                        .bind(auth_user.school_id)
                        .bind(s_name)
                        .bind(&s_code)
                        .fetch_optional(&mut *tx)
                        .await?;
                    }
                    if let Some(s) = sid {
                        section_cache.insert(s_code, s);
                    }
                    sid
                }
            };

            if let (Some(cid), Some(sid)) = (class_id, section_id) {
                let cs_id = match cs_cache.get(&(cid, sid)) {
                    Some(&csid) => Some(csid),
                    None => {
                        let mut csid: Option<Uuid> = sqlx::query_scalar(
                            "SELECT id FROM class_sections WHERE academic_year_id = $1 AND class_id = $2 AND section_id = $3 LIMIT 1",
                        )
                        .bind(ay_id)
                        .bind(cid)
                        .bind(sid)
                        .fetch_optional(&mut *tx)
                        .await?;

                        if csid.is_none() {
                            csid = sqlx::query_scalar(
                                "INSERT INTO class_sections (academic_year_id, class_id, section_id, status) VALUES ($1, $2, $3, 'ACTIVE') ON CONFLICT (academic_year_id, class_id, section_id) DO UPDATE SET status = 'ACTIVE' RETURNING id",
                            )
                            .bind(ay_id)
                            .bind(cid)
                            .bind(sid)
                            .fetch_optional(&mut *tx)
                            .await?;
                        }
                        if let Some(c) = csid {
                            cs_cache.insert((cid, sid), c);
                        }
                        csid
                    }
                };
                target_cs_id = cs_id;
            }
        } else if let Some(csid) = item.class_section_id {
            if valid_cs_ids.contains(&csid) {
                target_cs_id = Some(csid);
            }
        }

        if is_existing {
            sqlx::query(
                r#"UPDATE users
                   SET
                       first_name = COALESCE($1, first_name),
                       last_name = COALESCE($2, last_name),
                       display_name = COALESCE($3, display_name),
                       email = COALESCE($4, email),
                       phone = COALESCE($5, phone),
                       gender = COALESCE($6, gender),
                       blood_group = COALESCE($7, blood_group),
                       date_of_birth = COALESCE($8, date_of_birth),
                       admission_number = COALESCE($9, admission_number),
                       status = COALESCE($10, status),
                       class_section_id = COALESCE($11, class_section_id),
                       caste_category = COALESCE($12, caste_category),
                       custom_values = CASE
                           WHEN $13::jsonb IS NOT NULL THEN COALESCE(custom_values, '{}'::jsonb) || $13::jsonb
                           ELSE custom_values
                       END,
                       updated_at = NOW(),
                       updated_by_user_id = $14
                   WHERE id = $15"#,
            )
            .bind(final_first_name)
            .bind(final_last_name)
            .bind(final_display_name)
            .bind(clean_email)
            .bind(clean_phone)
            .bind(clean_gender)
            .bind(clean_blood_group)
            .bind(clean_dob)
            .bind(clean_adm)
            .bind(&status_val)
            .bind(target_cs_id)
            .bind(clean_caste)
            .bind(&custom_values_json)
            .bind(auth_user.id)
            .bind(target_id)
            .execute(&mut *tx)
            .await?;
        } else if target_cs_id.is_some() {
            sqlx::query("UPDATE users SET class_section_id = $1 WHERE id = $2")
                .bind(target_cs_id)
                .bind(target_id)
                .execute(&mut *tx)
                .await?;
        }

        if let Some(ref cf_map) = item.custom_fields {
            for (f_key, f_val) in cf_map {
                let val_str = match f_val {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                };
                let _ = sqlx::query(
                    r#"INSERT INTO user_custom_values (school_id, user_id, field_id, value, updated_at)
                       SELECT $1, $2, id, $3, NOW()
                       FROM custom_fields
                       WHERE school_id = $1 AND field_key = $4
                       ON CONFLICT (user_id, field_id)
                       DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()"#,
                )
                .bind(auth_user.school_id)
                .bind(target_id)
                .bind(&val_str)
                .bind(f_key)
                .execute(&mut *tx)
                .await;
            }
        }

        // If status marked as TC_ISSUED, deactivate their active enrollment
        if status_val.as_deref() == Some("TC_ISSUED") {
            let _ = sqlx::query(
                r#"UPDATE enrollments
                   SET status = 'INACTIVE', updated_at = NOW()
                   WHERE student_id = $1 AND status = 'ACTIVE'"#,
            )
            .bind(target_id)
            .execute(&mut *tx)
            .await;
        }

        // Check if user is a student using pre-fetched metadata
        let is_student: bool = if is_existing {
            user_meta_map
                .get(&target_id)
                .map(|m| m.is_student)
                .unwrap_or(false)
        } else {
            true
        };

        // If user is a student, sync roll_number and class_section_id to enrollments (closed system)
        if is_student {
            let clean_roll = item
                .roll_number
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());

            if let Some(ay_id) = active_ay_id {
                let resolved_cs = match target_cs_id {
                    Some(cs) => Some(cs),
                    None => user_meta_map
                        .get(&target_id)
                        .and_then(|m| m.class_section_id),
                };

                let existing_enrollment = enrollment_map.get(&target_id);

                if let Some(enroll) = existing_enrollment {
                    if clean_roll.is_some() || target_cs_id.is_some() {
                        sqlx::query(
                            r#"UPDATE enrollments
                               SET roll_number = COALESCE($1, roll_number),
                                   class_section_id = COALESCE($2, class_section_id),
                                   updated_at = NOW()
                               WHERE id = $3"#,
                        )
                        .bind(clean_roll)
                        .bind(target_cs_id)
                        .bind(enroll.id)
                        .execute(&mut *tx)
                        .await?;
                    }
                } else if clean_roll.is_some()
                    || resolved_cs.is_some()
                    || clean_adm.is_some()
                    || !is_existing
                {
                    sqlx::query(
                        r#"INSERT INTO enrollments (student_id, academic_year_id, class_section_id, roll_number, status, enrollment_date, created_at, updated_at)
                           VALUES ($1, $2, $3, $4, 'ACTIVE', CURRENT_DATE, NOW(), NOW())
                           ON CONFLICT (student_id, academic_year_id) DO UPDATE SET
                               class_section_id = COALESCE(EXCLUDED.class_section_id, enrollments.class_section_id),
                               roll_number = COALESCE(EXCLUDED.roll_number, enrollments.roll_number),
                               status = 'ACTIVE',
                               updated_at = NOW()"#,
                    )
                    .bind(target_id)
                    .bind(ay_id)
                    .bind(resolved_cs)
                    .bind(clean_roll)
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }

        updated_count += 1;
    }

    tx.commit().await?;

    let _ = crate::utils::audit::log_action(
        &state.db,
        Some(auth_user.school_id),
        Some(auth_user.id),
        "BULK_UPDATE_USERS",
        Some("USERS"),
        None,
        Some(serde_json::json!({
            "count": updated_count,
            "user_ids": payload.users.iter().map(|u| &u.id).collect::<Vec<_>>()
        })),
        None,
    )
    .await;

    crate::modules::users::repository::UserRepository::invalidate_users_cache(Some(
        auth_user.school_id,
    ));

    Ok((
        StatusCode::OK,
        Json(BulkUpdateUsersResponse {
            success: true,
            updated_count,
            message: format!("Successfully updated {} user records", updated_count),
        }),
    ))
}

#[allow(unused_imports)]
pub use crate::modules::transfer_certificate::dto::{
    IssueTransferCertificatesRequest, IssueTransferCertificatesResponse,
};
use crate::modules::transfer_certificate::TransferCertificateHandler;

/// Controller: Marks students as TC_ISSUED and deactivates their active enrollment.
pub async fn issue_transfer_certificates(
    auth_session: AuthSession,
    state: State<AppState>,
    payload: Json<IssueTransferCertificatesRequest>,
) -> Result<impl IntoResponse, AppError> {
    TransferCertificateHandler::issue_transfer_certificates(auth_session, state, payload).await
}
