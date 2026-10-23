use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
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

            let has_name = eff_display_name.map_or(false, |s| !s.trim().is_empty())
                || eff_first_name.map_or(false, |s| !s.trim().is_empty());

            let has_contact_or_id = eff_email.map_or(false, |s| !s.trim().is_empty())
                || eff_phone.map_or(false, |s| !s.trim().is_empty())
                || eff_adm.map_or(false, |s| !s.trim().is_empty());

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
               status = $9,
               updated_at = NOW(),
               updated_by_user_id = $10
           WHERE id = $11"#,
    )
    .bind(final_first_name)
    .bind(final_last_name)
    .bind(final_display_name)
    .bind(clean_email)
    .bind(clean_phone)
    .bind(clean_gender)
    .bind(clean_blood_group)
    .bind(clean_dob)
    .bind(&final_status)
    .bind(auth_user.id)
    .bind(target_user_id)
    .execute(&state.db)
    .await?;

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
        }),
    )
    .await
}

#[derive(Debug, Deserialize)]
pub struct BulkUpdateItemRequest {
    pub id: Uuid,
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

    let mut tx = state.db.begin().await?;
    let mut updated_count = 0;

    for item in &payload.users {
        // Tenant boundary check
        let is_super_admin = auth_user.roles.contains(&"SUPER_ADMIN".to_string());
        let exists: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM users WHERE id = $1 AND (school_id = $2 OR $3)"#,
        )
        .bind(item.id)
        .bind(auth_user.school_id)
        .bind(is_super_admin)
        .fetch_one(&mut *tx)
        .await?;

        if exists.0 == 0 {
            continue;
        }

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
                   updated_at = NOW(),
                   updated_by_user_id = $11
               WHERE id = $12"#,
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
        .bind(auth_user.id)
        .bind(item.id)
        .execute(&mut *tx)
        .await?;

        // If status marked as TC_ISSUED, deactivate their active enrollment
        if status_val.as_deref() == Some("TC_ISSUED") {
            let _ = sqlx::query(
                r#"UPDATE enrollments
                   SET status = 'INACTIVE', updated_at = NOW()
                   WHERE student_id = $1 AND status = 'ACTIVE'"#,
            )
            .bind(item.id)
            .execute(&mut *tx)
            .await;
        }

        // If roll_number provided, update active enrollment
        if let Some(ref roll) = item.roll_number {
            let clean_roll = roll.trim();
            if !clean_roll.is_empty() {
                let _ = sqlx::query(
                    r#"UPDATE enrollments
                       SET roll_number = $1
                       WHERE student_id = $2 AND status = 'ACTIVE'"#,
                )
                .bind(clean_roll)
                .bind(item.id)
                .execute(&mut *tx)
                .await;
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
            "user_ids": payload.users.iter().map(|u| u.id).collect::<Vec<_>>()
        })),
        None,
    )
    .await;

    Ok((
        StatusCode::OK,
        Json(BulkUpdateUsersResponse {
            success: true,
            updated_count,
            message: format!("Successfully updated {} user records", updated_count),
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct IssueTransferCertificatesRequest {
    pub user_ids: Vec<Uuid>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IssueTransferCertificatesResponse {
    pub success: bool,
    pub issued_count: usize,
    pub message: String,
}

/// Controller: Marks students as TC_ISSUED and deactivates their active enrollment.
pub async fn issue_transfer_certificates(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<IssueTransferCertificatesRequest>,
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
            "Only administrators can issue Transfer Certificates".into(),
        ));
    }

    if payload.user_ids.is_empty() {
        return Err(AppError::BadRequest(
            "No user IDs provided for TC issuance".into(),
        ));
    }

    let is_super_admin = auth_user.roles.contains(&"SUPER_ADMIN".to_string());
    let mut tx = state.db.begin().await?;
    let mut issued_count = 0;

    for user_id in &payload.user_ids {
        let result = sqlx::query(
            r#"UPDATE users
               SET status = 'TC_ISSUED',
                   updated_at = NOW(),
                   updated_by_user_id = $1
               WHERE id = $2 AND (school_id = $3 OR $4)"#,
        )
        .bind(auth_user.id)
        .bind(user_id)
        .bind(auth_user.school_id)
        .bind(is_super_admin)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() > 0 {
            // Deactivate active enrollment for student
            let _ = sqlx::query(
                r#"UPDATE enrollments
                   SET status = 'INACTIVE', updated_at = NOW()
                   WHERE student_id = $1 AND status = 'ACTIVE'"#,
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await;

            issued_count += 1;
        }
    }

    tx.commit().await?;

    let _ = crate::utils::audit::log_action(
        &state.db,
        Some(auth_user.school_id),
        Some(auth_user.id),
        "ISSUE_TRANSFER_CERTIFICATES",
        Some("USERS"),
        None,
        Some(serde_json::json!({
            "count": issued_count,
            "user_ids": payload.user_ids,
            "reason": payload.reason
        })),
        None,
    )
    .await;

    Ok((
        StatusCode::OK,
        Json(IssueTransferCertificatesResponse {
            success: true,
            issued_count,
            message: format!(
                "Successfully issued Transfer Certificate(s) for {} student(s)",
                issued_count
            ),
        }),
    ))
}
