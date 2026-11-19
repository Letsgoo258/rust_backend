use crate::{error::AppError, modules::auth::backend::AuthSession, state::AppState};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModuleConfigItem {
    pub key: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub is_enabled: bool,
    pub settings: serde_json::Value,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StaffModuleAssignment {
    pub id: String,
    pub user_id: Uuid,
    pub user_name: String,
    pub user_username: String,
    pub user_email: Option<String>,
    pub employee_id: Option<String>,
    pub user_roles: Vec<String>,
    pub module_key: String,
    pub module_name: String,
    pub access_tier: String, // "SUPERVISOR", "OPERATOR", "AUDITOR"
    pub dynamic_config: serde_json::Value,
    pub status: String, // "ACTIVE", "SUSPENDED"
    pub notes: Option<String>,
    pub assigned_at: String,
    pub assigned_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StaffUserOption {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub employee_id: Option<String>,
    pub roles: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveModuleConfigPayload {
    pub module_key: String,
    pub is_enabled: bool,
    pub settings: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssignStaffPayload {
    pub id: Option<String>,
    pub user_id: Uuid,
    pub module_key: String,
    pub access_tier: String,
    pub dynamic_config: serde_json::Value,
    pub notes: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateAssignmentStatusPayload {
    pub status: String,
}

pub fn get_default_modules() -> Vec<ModuleConfigItem> {
    vec![
        ModuleConfigItem {
            key: "in_out".into(),
            name: "In-Out / Gate Pass".into(),
            category: "Operations".into(),
            description: "Real-time gate security, student movement register, visitor verification, and automated guardian SMS dispatch.".into(),
            is_enabled: true,
            settings: json!({
                "require_parent_consent": true,
                "auto_notify_guardian_sms": true,
                "max_gate_passes_per_day": 50,
                "gate_pass_validity_hours": 4,
                "allow_student_self_checkout": false,
                "mandatory_visitor_id_proof": true,
                "checkout_grace_period_mins": 15
            }),
            updated_at: None,
        },
        ModuleConfigItem {
            key: "ledger".into(),
            name: "Accounts & Fee Ledger".into(),
            category: "Finance".into(),
            description: "Institutional double-entry ledger, student fee structures, concessions, collection counters, and receipt books.".into(),
            is_enabled: true,
            settings: json!({
                "currency": "INR",
                "allow_partial_payments": true,
                "auto_generate_receipt_numbers": true,
                "receipt_prefix": "REC-2026",
                "enable_late_fine_calculation": true,
                "max_discount_waiver_pct": 15,
                "require_admin_approval_for_refund": true
            }),
            updated_at: None,
        },
        ModuleConfigItem {
            key: "asset_ledger".into(),
            name: "Asset Ledger & Inventory".into(),
            category: "Infrastructure".into(),
            description: "Campus physical asset register, laboratory equipment depreciation, room allocations, and maintenance audit schedules.".into(),
            is_enabled: true,
            settings: json!({
                "track_serial_numbers": true,
                "enable_maintenance_alerts": true,
                "depreciation_method": "straight_line",
                "asset_id_prefix": "AST-",
                "audit_frequency_days": 90,
                "require_hod_signoff_for_disposal": true
            }),
            updated_at: None,
        },
        ModuleConfigItem {
            key: "print_studio".into(),
            name: "Print & Certificate Studio".into(),
            category: "Documentation".into(),
            description: "Institutional printing desk: scan barcodes/IDs to print signed records directly from DB (TC, Bonafide, Study, Conduct, Fee receipts), or upload external documents for custom specimen printing with digital seal & watermark overlays.".into(),
            is_enabled: true,
            settings: json!({
                "tc_serial_prefix": "TC-2026/",
                "enable_scan_to_print": true,
                "allow_direct_document_upload": true,
                "embed_digital_signatures": true,
                "auto_archive_generated_pdfs": true,
                "watermark_enabled": true,
                "watermark_opacity": 0.12,
                "allow_bulk_specimen_printing": true,
                "qr_code_verification_url": true,
                "prevent_duplicate_tc_issuance": true,
                "daily_print_quota_per_user": 50,
                "max_upload_size_mb": 10,
                "allowed_upload_formats": "PDF, JPG, PNG"
            }),
            updated_at: None,
        },
        ModuleConfigItem {
            key: "front_office".into(),
            name: "Front Office & Reception".into(),
            category: "Administration".into(),
            description: "Visitor logging, badge issuing, inward/outward postal dispatch, enquiry follow-up pipeline, and digital reception logs.".into(),
            is_enabled: true,
            settings: json!({
                "visitor_badge_printing": true,
                "postal_dispatch_tracking": true,
                "call_log_recording": true,
                "visitor_overstay_alert_mins": 120,
                "enquiry_auto_followup_days": 3,
                "enable_digital_visitor_signature": true
            }),
            updated_at: None,
        },
        ModuleConfigItem {
            key: "library".into(),
            name: "Library & Catalog".into(),
            category: "Academic".into(),
            description: "Book cataloging, barcode scanning, student and staff lending quotas, overdue fine calculation, and reservation queues.".into(),
            is_enabled: true,
            settings: json!({
                "max_books_per_student": 3,
                "max_books_per_staff": 10,
                "lending_duration_days": 14,
                "overdue_fine_per_day": 5,
                "barcode_scanner_enabled": true,
                "allow_reservation_queue": true
            }),
            updated_at: None,
        },
        ModuleConfigItem {
            key: "canteen".into(),
            name: "Canteen & Meal POS".into(),
            category: "Services".into(),
            description: "Campus cafeteria meal management, cashless student smart cards, meal pre-booking, daily spending caps, and inventory.".into(),
            is_enabled: true,
            settings: json!({
                "cashless_wallet_enabled": true,
                "daily_spending_limit": 200,
                "allow_negative_balance": false,
                "inventory_stock_threshold_alert": 20,
                "meal_slot_booking_hours_ahead": 2,
                "dietary_allergen_warning": true
            }),
            updated_at: None,
        },
        ModuleConfigItem {
            key: "admissions".into(),
            name: "Admissions & Intake Engine".into(),
            category: "Intake".into(),
            description: "Online application intake pipeline, document verification, entrance scheduling, merit lists, and enrollment.".into(),
            is_enabled: true,
            settings: json!({
                "online_application_open": true,
                "auto_schedule_entrance_test": false,
                "intake_academic_year": "2026-2027",
                "application_fee_amount": 500,
                "require_document_verification_otp": true,
                "max_waitlist_per_grade": 25
            }),
            updated_at: None,
        },
    ]
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_modules_overview))
        .route("/config", post(save_module_config))
        .route("/assign-staff", post(assign_staff_to_module))
        .route(
            "/staff-assignment/:assignment_id",
            delete(delete_staff_assignment),
        )
        .route(
            "/staff-assignment/:assignment_id/status",
            patch(update_staff_assignment_status),
        )
}

/// GET /api/v1/settings/modules
/// Fetches modules configuration, current staff assignments, and eligible staff list.
pub async fn get_modules_overview(
    auth_session: AuthSession,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let school_id = user.school_id;

    // 1. Fetch saved modules_configuration
    let saved_mods_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'modules_configuration'
        "#,
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let default_modules = get_default_modules();
    let mut modules: Vec<ModuleConfigItem> = if let Some(val) = saved_mods_val {
        serde_json::from_value(val).unwrap_or_else(|_| default_modules.clone())
    } else {
        default_modules.clone()
    };

    // Ensure all 8 default modules exist even if database only had a subset
    for def in &default_modules {
        if !modules.iter().any(|m| m.key == def.key) {
            modules.push(def.clone());
        }
    }

    // 2. Fetch staff members belonging to the school
    let staff_rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Vec<String>,
        ),
    >(
        r#"
        SELECT 
            u.id, 
            u.username, 
            u.display_name,
            u.first_name,
            u.last_name,
            u.email,
            st.employee_id,
            COALESCE(array_agg(DISTINCT ro.code::TEXT) FILTER (WHERE ro.code IS NOT NULL), ARRAY[]::TEXT[]) as roles
        FROM users u
        LEFT JOIN staff st ON st.user_id = u.id
        LEFT JOIN user_roles ur ON ur.user_id = u.id
        LEFT JOIN roles ro ON ro.id = ur.role_id
        WHERE u.school_id = $1
        GROUP BY u.id, u.username, u.display_name, u.first_name, u.last_name, u.email, st.employee_id
        ORDER BY u.display_name ASC, u.username ASC
        "#,
    )
    .bind(school_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut staff_map: HashMap<Uuid, StaffUserOption> = HashMap::new();
    let mut staff_list: Vec<StaffUserOption> = Vec::new();

    for (id, username, display_name, first_name, last_name, email, employee_id, roles) in staff_rows
    {
        let primary_name = display_name
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                let f = first_name.as_deref().unwrap_or("").trim();
                let l = last_name.as_deref().unwrap_or("").trim();
                let full = format!("{} {}", f, l).trim().to_string();
                if full.is_empty() {
                    None
                } else {
                    Some(full)
                }
            })
            .unwrap_or_else(|| username.clone());

        let opt = StaffUserOption {
            id,
            username: username.clone(),
            display_name: primary_name,
            email,
            employee_id,
            roles,
        };

        staff_map.insert(id, opt.clone());
        staff_list.push(opt);
    }

    // 3. Fetch saved staff assignments
    let saved_assignments_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'modules_staff_assignments'
        "#,
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let mut assignments: Vec<StaffModuleAssignment> = if let Some(val) = saved_assignments_val {
        serde_json::from_value(val).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Enrich assignments with latest staff and module info
    let module_name_map: HashMap<String, String> = modules
        .iter()
        .map(|m| (m.key.clone(), m.name.clone()))
        .collect();

    for asg in &mut assignments {
        if let Some(st) = staff_map.get(&asg.user_id) {
            asg.user_name = st.display_name.clone();
            asg.user_username = st.username.clone();
            asg.user_email = st.email.clone();
            asg.employee_id = st.employee_id.clone();
            asg.user_roles = st.roles.clone();
        }
        if let Some(m_name) = module_name_map.get(&asg.module_key) {
            asg.module_name = m_name.clone();
        }
    }

    Ok(Json(json!({
        "modules": modules,
        "assignments": assignments,
        "staff": staff_list
    })))
}

/// POST /api/v1/settings/modules/config
/// Updates configuration settings and enabled status for a specific module.
pub async fn save_module_config(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<SaveModuleConfigPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let school_id = user.school_id;

    // Load existing modules
    let saved_mods_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'modules_configuration'
        "#,
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let mut modules: Vec<ModuleConfigItem> = if let Some(val) = saved_mods_val {
        serde_json::from_value(val).unwrap_or_else(|_| get_default_modules())
    } else {
        get_default_modules()
    };

    let now_str = Utc::now().to_rfc3339();

    if let Some(existing) = modules.iter_mut().find(|m| m.key == payload.module_key) {
        existing.is_enabled = payload.is_enabled;
        existing.settings = payload.settings;
        existing.updated_at = Some(now_str);
    } else {
        // Find in default list and insert
        if let Some(mut def) = get_default_modules()
            .into_iter()
            .find(|m| m.key == payload.module_key)
        {
            def.is_enabled = payload.is_enabled;
            def.settings = payload.settings;
            def.updated_at = Some(now_str);
            modules.push(def);
        } else {
            return Err(AppError::BadRequest(format!(
                "Module '{}' is not recognized",
                payload.module_key
            )));
        }
    }

    let val_json = serde_json::to_value(&modules).unwrap_or_else(|_| json!([]));

    sqlx::query(
        r#"
        INSERT INTO school_settings (school_id, key, value, updated_at)
        VALUES ($1, 'modules_configuration', $2, now())
        ON CONFLICT (school_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now()
        "#,
    )
    .bind(school_id)
    .bind(val_json)
    .execute(&state.db)
    .await?;

    tracing::info!(
        school_id = %school_id,
        module = %payload.module_key,
        enabled = %payload.is_enabled,
        "Module configuration updated"
    );

    Ok(Json(json!({
        "success": true,
        "modules": modules
    })))
}

/// POST /api/v1/settings/modules/assign-staff
/// Assigns a staff member to an institutional module with dynamic parameters.
pub async fn assign_staff_to_module(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Json(payload): Json<AssignStaffPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let school_id = user.school_id;

    // Load existing assignments
    let saved_assignments_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'modules_staff_assignments'
        "#,
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let mut assignments: Vec<StaffModuleAssignment> = if let Some(val) = saved_assignments_val {
        serde_json::from_value(val).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Lookup staff info
    let staff_row = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Vec<String>,
        ),
    >(
        r#"
        SELECT 
            u.id, 
            u.username, 
            u.display_name,
            u.first_name,
            u.last_name,
            u.email,
            st.employee_id,
            COALESCE(array_agg(DISTINCT ro.code::TEXT) FILTER (WHERE ro.code IS NOT NULL), ARRAY[]::TEXT[]) as roles
        FROM users u
        LEFT JOIN staff st ON st.user_id = u.id
        LEFT JOIN user_roles ur ON ur.user_id = u.id
        LEFT JOIN roles ro ON ro.id = ur.role_id
        WHERE u.id = $1 AND u.school_id = $2
        GROUP BY u.id, u.username, u.display_name, u.first_name, u.last_name, u.email, st.employee_id
        "#,
    )
    .bind(payload.user_id)
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let (user_id, username, display_name, first_name, last_name, email, employee_id, roles) =
        staff_row
            .ok_or_else(|| AppError::NotFound("Staff user not found in this school".into()))?;

    let primary_name = display_name
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let f = first_name.as_deref().unwrap_or("").trim();
            let l = last_name.as_deref().unwrap_or("").trim();
            let full = format!("{} {}", f, l).trim().to_string();
            if full.is_empty() {
                None
            } else {
                Some(full)
            }
        })
        .unwrap_or_else(|| username.clone());

    let module_name = get_default_modules()
        .into_iter()
        .find(|m| m.key == payload.module_key)
        .map(|m| m.name)
        .unwrap_or_else(|| payload.module_key.clone());

    let now_str = Utc::now().to_rfc3339();
    let assigner_name = user
        .display_name
        .clone()
        .unwrap_or_else(|| user.username.clone());

    let assignment_id = payload
        .id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    if let Some(existing) = assignments.iter_mut().find(|a| a.id == assignment_id) {
        existing.user_id = user_id;
        existing.user_name = primary_name;
        existing.user_username = username;
        existing.user_email = email;
        existing.employee_id = employee_id;
        existing.user_roles = roles;
        existing.module_key = payload.module_key;
        existing.module_name = module_name;
        existing.access_tier = payload.access_tier.to_uppercase();
        existing.dynamic_config = payload.dynamic_config;
        existing.notes = payload.notes;
        if let Some(st) = payload.status {
            existing.status = st.to_uppercase();
        }
    } else {
        assignments.push(StaffModuleAssignment {
            id: assignment_id.clone(),
            user_id,
            user_name: primary_name,
            user_username: username,
            user_email: email,
            employee_id,
            user_roles: roles,
            module_key: payload.module_key,
            module_name,
            access_tier: payload.access_tier.to_uppercase(),
            dynamic_config: payload.dynamic_config,
            status: payload
                .status
                .unwrap_or_else(|| "ACTIVE".into())
                .to_uppercase(),
            notes: payload.notes,
            assigned_at: now_str,
            assigned_by: Some(assigner_name),
        });
    }

    let val_json = serde_json::to_value(&assignments).unwrap_or_else(|_| json!([]));

    sqlx::query(
        r#"
        INSERT INTO school_settings (school_id, key, value, updated_at)
        VALUES ($1, 'modules_staff_assignments', $2, now())
        ON CONFLICT (school_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now()
        "#,
    )
    .bind(school_id)
    .bind(val_json)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({
        "success": true,
        "assignments": assignments
    })))
}

/// DELETE /api/v1/settings/modules/staff-assignment/:assignment_id
pub async fn delete_staff_assignment(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(assignment_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let school_id = user.school_id;

    let saved_assignments_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'modules_staff_assignments'
        "#,
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let mut assignments: Vec<StaffModuleAssignment> = if let Some(val) = saved_assignments_val {
        serde_json::from_value(val).unwrap_or_default()
    } else {
        Vec::new()
    };

    assignments.retain(|a| a.id != assignment_id);

    let val_json = serde_json::to_value(&assignments).unwrap_or_else(|_| json!([]));

    sqlx::query(
        r#"
        INSERT INTO school_settings (school_id, key, value, updated_at)
        VALUES ($1, 'modules_staff_assignments', $2, now())
        ON CONFLICT (school_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now()
        "#,
    )
    .bind(school_id)
    .bind(val_json)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({
        "success": true,
        "assignments": assignments
    })))
}

/// PATCH /api/v1/settings/modules/staff-assignment/:assignment_id/status
pub async fn update_staff_assignment_status(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(assignment_id): Path<String>,
    Json(payload): Json<UpdateAssignmentStatusPayload>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .user
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    let school_id = user.school_id;

    let saved_assignments_val = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM school_settings
        WHERE school_id = $1 AND key = 'modules_staff_assignments'
        "#,
    )
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let mut assignments: Vec<StaffModuleAssignment> = if let Some(val) = saved_assignments_val {
        serde_json::from_value(val).unwrap_or_default()
    } else {
        Vec::new()
    };

    if let Some(target) = assignments.iter_mut().find(|a| a.id == assignment_id) {
        target.status = payload.status.to_uppercase();
    } else {
        return Err(AppError::NotFound("Assignment not found".into()));
    }

    let val_json = serde_json::to_value(&assignments).unwrap_or_else(|_| json!([]));

    sqlx::query(
        r#"
        INSERT INTO school_settings (school_id, key, value, updated_at)
        VALUES ($1, 'modules_staff_assignments', $2, now())
        ON CONFLICT (school_id, key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now()
        "#,
    )
    .bind(school_id)
    .bind(val_json)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({
        "success": true,
        "assignments": assignments
    })))
}
