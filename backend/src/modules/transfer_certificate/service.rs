use super::dto::{
    CancelIssuedReportPayload, IssueReportPayload, IssueTransferCertificatesRequest,
    IssueTransferCertificatesResponse, ListIssuedReportsQuery, RollbackPayload,
    SaveTemplatePayload, UpdateSequencePayload,
};
use super::repository::{IssuedReport, ReportSequence, TransferCertificateRepository};
use crate::error::AppError;
use crate::utils::audit::log_action;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

pub struct TransferCertificateService;

/// Pure document number formatter replacing tokens: {prefix}, {suffix}, {year}, {academic_year}, {number}
pub fn format_document_number(
    pattern: &str,
    prefix: &str,
    suffix: &str,
    number: i32,
    padding: i32,
    academic_year: Option<&str>,
) -> String {
    let now = chrono::Local::now();
    let year = now.format("%Y").to_string();
    let pad_len = if padding > 0 { padding as usize } else { 4 };
    let num_str = format!("{:0>width$}", number, width = pad_len);
    let acad_str = academic_year.unwrap_or(&year);

    let mut res = pattern.to_string();
    res = res.replace("{prefix}", prefix);
    res = res.replace("{suffix}", suffix);
    res = res.replace("{year}", &year);
    res = res.replace("{academic_year}", acad_str);
    res = res.replace("{number}", &num_str);
    res
}

impl TransferCertificateService {
    pub async fn get_template(
        db: &PgPool,
        school_id: Uuid,
        template_type: Option<&str>,
    ) -> Result<Option<Value>, AppError> {
        let tpl_type = template_type.unwrap_or("transfer_certificate");
        let setting_key = format!("report_tpl_{}", tpl_type);
        let val =
            TransferCertificateRepository::get_template_value(db, school_id, &setting_key).await?;
        Ok(val)
    }

    pub async fn save_template(
        db: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        payload: SaveTemplatePayload,
        is_published: bool,
    ) -> Result<Value, AppError> {
        let tpl_type = payload
            .template_type
            .unwrap_or_else(|| "transfer_certificate".into());
        let name = payload
            .name
            .unwrap_or_else(|| "Standard Transfer Certificate".into());
        let setting_key = format!("report_tpl_{}", tpl_type);
        let versions_key = format!("report_tpl_{}_versions", tpl_type);

        let existing_val =
            TransferCertificateRepository::get_template_value(db, school_id, &setting_key).await?;

        let current_version = existing_val
            .as_ref()
            .and_then(|v| v.get("version"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let new_version = current_version + 1;

        let template_obj = json!({
            "id": Uuid::new_v4(),
            "template_type": tpl_type,
            "name": name,
            "config": payload.config,
            "is_published": is_published,
            "version": new_version,
            "updated_at": chrono::Utc::now().to_rfc3339()
        });

        TransferCertificateRepository::save_template_value(
            db,
            school_id,
            &setting_key,
            &template_obj,
        )
        .await?;

        // Append to versions history array in school_settings
        let version_entry = json!({
            "id": Uuid::new_v4(),
            "version": new_version,
            "snapshot": payload.config,
            "diff_summary": format!("Version {} saved", new_version),
            "created_by": user_id,
            "created_at": chrono::Utc::now().to_rfc3339()
        });

        let existing_versions =
            TransferCertificateRepository::get_template_value(db, school_id, &versions_key).await?;

        let mut versions_arr = existing_versions
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();

        versions_arr.insert(0, version_entry);
        if versions_arr.len() > 25 {
            versions_arr.truncate(25);
        }

        let _ = TransferCertificateRepository::save_template_value(
            db,
            school_id,
            &versions_key,
            &json!(versions_arr),
        )
        .await;

        let _ = log_action(
            db,
            Some(school_id),
            Some(user_id),
            "SAVE_TC_TEMPLATE",
            Some("SETTINGS_REPORTS"),
            None,
            Some(json!({ "version": new_version, "template_type": tpl_type })),
            None,
        )
        .await;

        Ok(template_obj)
    }

    pub async fn request_otp(
        db: &PgPool,
        _school_id: Uuid,
        user_id: Uuid,
    ) -> Result<String, AppError> {
        TransferCertificateRepository::clear_otps_for_action(db, user_id, "EDIT_TC_TEMPLATE")
            .await?;

        // Generate 6-digit alphanumeric OTP
        let otp: String = Uuid::new_v4()
            .to_string()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(6)
            .collect::<String>()
            .to_uppercase();

        TransferCertificateRepository::insert_otp(db, user_id, "EDIT_TC_TEMPLATE", &otp).await?;
        Ok(otp)
    }

    pub async fn verify_otp(
        db: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        otp_code: &str,
    ) -> Result<String, AppError> {
        let entered = otp_code.trim().to_uppercase();

        let otp_record =
            TransferCertificateRepository::get_active_otp(db, user_id, "EDIT_TC_TEMPLATE").await?;

        let (record_id, expected_code) = match otp_record {
            Some(r) => r,
            None => {
                return Err(AppError::BadRequest(
                    "No pending OTP found or expired. Request a new one.".into(),
                ))
            }
        };

        if expected_code.to_uppercase() != entered {
            return Err(AppError::BadRequest("Invalid OTP code".into()));
        }

        let _ = TransferCertificateRepository::delete_otp(db, record_id).await;

        let unlock_token = Uuid::new_v4().to_string();

        let _ = log_action(
            db,
            Some(school_id),
            Some(user_id),
            "UNLOCK_TC_TEMPLATE_LAYOUT",
            Some("SETTINGS_REPORTS"),
            None,
            Some(json!({ "unlocked_at": chrono::Utc::now() })),
            None,
        )
        .await;

        Ok(unlock_token)
    }

    pub async fn list_versions(
        db: &PgPool,
        school_id: Uuid,
        template_type: Option<&str>,
    ) -> Result<Vec<Value>, AppError> {
        let tpl_type = template_type.unwrap_or("transfer_certificate");
        let versions_key = format!("report_tpl_{}_versions", tpl_type);

        let versions_val =
            TransferCertificateRepository::get_template_value(db, school_id, &versions_key).await?;

        let versions = versions_val
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();

        Ok(versions)
    }

    pub async fn rollback_version(
        db: &PgPool,
        school_id: Uuid,
        _user_id: Uuid,
        payload: RollbackPayload,
    ) -> Result<Value, AppError> {
        let tpl_type = "transfer_certificate";
        let versions_key = format!("report_tpl_{}_versions", tpl_type);
        let setting_key = format!("report_tpl_{}", tpl_type);

        let versions_val =
            TransferCertificateRepository::get_template_value(db, school_id, &versions_key).await?;

        let versions = versions_val
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();

        let target_snapshot = versions.into_iter().find(|v| {
            v.get("id")
                .and_then(|id| id.as_str())
                .map(|s| s == payload.version_id.to_string())
                .unwrap_or(false)
        });

        let target = match target_snapshot {
            Some(t) => t,
            None => return Err(AppError::NotFound("Version snapshot not found".into())),
        };

        let snapshot_config = target.get("snapshot").cloned().unwrap_or(json!({}));
        let target_ver = target.get("version").and_then(|v| v.as_i64()).unwrap_or(1);

        let updated_obj = json!({
            "id": Uuid::new_v4(),
            "template_type": tpl_type,
            "name": "Standard Transfer Certificate",
            "config": snapshot_config,
            "is_published": false,
            "version": target_ver + 1,
            "updated_at": chrono::Utc::now().to_rfc3339()
        });

        TransferCertificateRepository::save_template_value(
            db,
            school_id,
            &setting_key,
            &updated_obj,
        )
        .await?;

        Ok(updated_obj)
    }

    pub async fn get_sequences(db: &PgPool, school_id: Uuid) -> Result<Vec<Value>, AppError> {
        TransferCertificateRepository::ensure_default_sequences(db, school_id).await?;

        let rows = TransferCertificateRepository::list_sequences(db, school_id).await?;

        let sequences: Vec<Value> = rows
            .into_iter()
            .map(|seq| {
                let next_num = seq.current_number + 1;
                let next_formatted = format_document_number(
                    &seq.format_pattern,
                    &seq.prefix,
                    seq.suffix.as_deref().unwrap_or(""),
                    next_num,
                    seq.padding_length,
                    seq.academic_year.as_deref(),
                );
                let current_formatted = if seq.current_number > 0 {
                    Some(format_document_number(
                        &seq.format_pattern,
                        &seq.prefix,
                        seq.suffix.as_deref().unwrap_or(""),
                        seq.current_number,
                        seq.padding_length,
                        seq.academic_year.as_deref(),
                    ))
                } else {
                    None
                };

                json!({
                    "id": seq.id,
                    "school_id": seq.school_id,
                    "report_type": seq.report_type,
                    "display_name": seq.display_name,
                    "prefix": seq.prefix,
                    "suffix": seq.suffix,
                    "current_number": seq.current_number,
                    "current_formatted_number": current_formatted,
                    "next_number": next_num,
                    "next_formatted_number": next_formatted,
                    "padding_length": seq.padding_length,
                    "format_pattern": seq.format_pattern,
                    "academic_year": seq.academic_year,
                    "is_locked": seq.is_locked,
                    "reset_annually": seq.reset_annually,
                    "created_at": seq.created_at,
                    "updated_at": seq.updated_at
                })
            })
            .collect();

        Ok(sequences)
    }

    pub async fn get_sequence_detail(
        db: &PgPool,
        school_id: Uuid,
        report_type: &str,
    ) -> Result<(ReportSequence, Option<String>, i32, String), AppError> {
        TransferCertificateRepository::ensure_default_sequences(db, school_id).await?;

        let seq_opt =
            TransferCertificateRepository::get_sequence(db, school_id, report_type).await?;

        match seq_opt {
            Some(s) => {
                let next_num = s.current_number + 1;
                let next_formatted = format_document_number(
                    &s.format_pattern,
                    &s.prefix,
                    s.suffix.as_deref().unwrap_or(""),
                    next_num,
                    s.padding_length,
                    s.academic_year.as_deref(),
                );
                let current_formatted = if s.current_number > 0 {
                    Some(format_document_number(
                        &s.format_pattern,
                        &s.prefix,
                        s.suffix.as_deref().unwrap_or(""),
                        s.current_number,
                        s.padding_length,
                        s.academic_year.as_deref(),
                    ))
                } else {
                    None
                };
                Ok((s, current_formatted, next_num, next_formatted))
            }
            None => Err(AppError::NotFound("Report sequence not found".into())),
        }
    }

    pub async fn update_sequence(
        db: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        report_type: &str,
        payload: UpdateSequencePayload,
    ) -> Result<(ReportSequence, i32, String), AppError> {
        TransferCertificateRepository::ensure_default_sequences(db, school_id).await?;

        let updated =
            TransferCertificateRepository::update_sequence(db, school_id, report_type, &payload)
                .await?;

        let next_num = updated.current_number + 1;
        let next_formatted = format_document_number(
            &updated.format_pattern,
            &updated.prefix,
            updated.suffix.as_deref().unwrap_or(""),
            next_num,
            updated.padding_length,
            updated.academic_year.as_deref(),
        );

        let _ = log_action(
            db,
            Some(school_id),
            Some(user_id),
            "UPDATE_REPORT_SEQUENCE",
            Some("SETTINGS_REPORTS"),
            None,
            Some(json!({
                "report_type": report_type,
                "current_number": updated.current_number,
                "prefix": updated.prefix,
                "is_locked": updated.is_locked
            })),
            None,
        )
        .await;

        Ok((updated, next_num, next_formatted))
    }

    pub async fn issue_report(
        db: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        issuer_name: &str,
        payload: IssueReportPayload,
    ) -> Result<(Vec<Value>, String, i32, String), AppError> {
        if payload.student_ids.is_empty() {
            return Err(AppError::BadRequest(
                "No student IDs provided for report issuance".into(),
            ));
        }

        TransferCertificateRepository::ensure_default_sequences(db, school_id).await?;

        let mut tx = db.begin().await?;

        let seq = match TransferCertificateRepository::get_sequence_for_update(
            &mut tx,
            school_id,
            &payload.report_type,
        )
        .await?
        {
            Some(s) => s,
            None => {
                return Err(AppError::NotFound(format!(
                    "Sequence configuration not found for '{}'",
                    payload.report_type
                )));
            }
        };

        if seq.is_locked {
            return Err(AppError::BadRequest(format!(
                "Numbering for '{}' is locked by School Admin. Unlock in Report Settings to issue.",
                seq.display_name
            )));
        }

        let mut current_counter = seq.current_number;
        let mut issued_docs: Vec<Value> = Vec::new();

        for student_id in &payload.student_ids {
            let student_row = TransferCertificateRepository::fetch_student_details(
                &mut tx,
                *student_id,
                school_id,
            )
            .await?;

            let (
                st_id,
                first_name,
                last_name,
                display_name,
                adm_no,
                sec_name,
                cls_name,
                cls_sec_id,
            ) = match student_row {
                Some(r) => r,
                None => {
                    return Err(AppError::NotFound(format!(
                        "Student not found with ID {}",
                        student_id
                    )));
                }
            };

            let student_name = display_name
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    let mut n = first_name.unwrap_or_default();
                    if let Some(l) = last_name {
                        if !n.is_empty() {
                            n.push(' ');
                        }
                        n.push_str(&l);
                    }
                    if n.is_empty() {
                        "Student".to_string()
                    } else {
                        n
                    }
                });

            // CRITICAL GOVERNANCE CHECK: Check if an active/valid document of this type has ALREADY been issued
            let existing_issued = sqlx::query_as::<_, (Uuid, String, i32, chrono::NaiveDate)>(
                r#"
                SELECT id, document_number, serial_number, issue_date
                FROM issued_reports
                WHERE school_id = $1 
                  AND student_id = $2 
                  AND LOWER(report_type) = LOWER($3) 
                  AND status = 'ISSUED'
                LIMIT 1
                "#,
            )
            .bind(school_id)
            .bind(*student_id)
            .bind(&payload.report_type)
            .fetch_optional(&mut *tx)
            .await?;

            if let Some((_, existing_doc_no, _, existing_date)) = existing_issued {
                return Err(AppError::BadRequest(format!(
                    "A valid {} ({}) has ALREADY been issued for {} ({}) on {}. A student cannot have multiple active certificates. Please use Reprint or cancel the existing certificate first.",
                    seq.display_name,
                    existing_doc_no,
                    student_name,
                    adm_no.as_deref().unwrap_or("N/A"),
                    existing_date.format("%d %b %Y")
                )));
            }

            // Allocate next serial number only after verifying student has no active certificate
            current_counter += 1;
            let doc_number = format_document_number(
                &seq.format_pattern,
                &seq.prefix,
                seq.suffix.as_deref().unwrap_or(""),
                current_counter,
                seq.padding_length,
                payload
                    .academic_year
                    .as_deref()
                    .or(seq.academic_year.as_deref()),
            );

            let class_section = match (cls_name, sec_name) {
                (Some(c), Some(s)) => Some(format!("Class {} - {}", c, s)),
                (Some(c), None) => Some(format!("Class {}", c)),
                _ => None,
            };

            let mut vars_snapshot = payload.custom_variables.clone().unwrap_or(json!({}));
            if let Some(obj) = vars_snapshot.as_object_mut() {
                obj.insert("document_number".to_string(), json!(doc_number));
                obj.insert("tc.tc_number".to_string(), json!(doc_number));
                obj.insert("student_name".to_string(), json!(student_name));
                obj.insert("admission_number".to_string(), json!(adm_no));
            }

            let inserted = TransferCertificateRepository::insert_issued_report(
                &mut tx,
                school_id,
                &payload.report_type,
                &doc_number,
                current_counter,
                st_id,
                &student_name,
                adm_no.as_deref(),
                class_section.as_deref(),
                user_id,
                issuer_name,
                payload.academic_year.as_deref(),
                &vars_snapshot,
                payload.remarks.as_deref(),
            )
            .await?;

            // If issuing a Transfer Certificate, transition the student's status to TC_ISSUED
            // and execute complete institutional detachment across all subsystems
            let is_tc = payload.report_type.to_lowercase().contains("transfer");
            if is_tc {
                // 1. Update users status
                let _ = TransferCertificateRepository::mark_student_tc_issued(
                    &mut tx, st_id, school_id, false, user_id,
                )
                .await?;

                // 2. Deactivate active academic year enrollments
                let _ =
                    TransferCertificateRepository::deactivate_active_enrollments(&mut tx, st_id)
                        .await?;

                // 3. Drop active subject/elective enrollments so student does not appear on elective rosters/marks
                let _ =
                    TransferCertificateRepository::drop_active_subject_enrollments(&mut tx, st_id)
                        .await?;

                // 4. Invalidate all active student sessions (web portal, WhatsApp, OTPs)
                let _ = TransferCertificateRepository::revoke_user_sessions(&mut tx, st_id).await?;

                // 5. Record section withdrawal history if student had an assigned class section
                if let Some(cs_id) = cls_sec_id {
                    let _ = TransferCertificateRepository::record_class_section_withdrawal(
                        &mut tx,
                        school_id,
                        cs_id,
                        st_id,
                        user_id,
                        &student_name,
                    )
                    .await;
                }

                // 6. Record universal audit change history
                let _ = TransferCertificateRepository::record_entity_withdrawal_history(
                    &mut tx,
                    school_id,
                    st_id,
                    user_id,
                    &doc_number,
                )
                .await;

                // 7. Invalidate in-memory user auth cache
                crate::modules::auth::backend::invalidate_user_cache(&st_id);
            }

            issued_docs.push(json!({
                "id": inserted.0,
                "document_number": inserted.1,
                "serial_number": inserted.2,
                "student_id": st_id,
                "student_name": student_name,
                "issue_date": inserted.3,
                "status": inserted.4
            }));
        }

        TransferCertificateRepository::update_sequence_counter(&mut tx, seq.id, current_counter)
            .await?;

        tx.commit().await?;

        let _ = log_action(
            db,
            Some(school_id),
            Some(user_id),
            "ISSUE_REPORT",
            Some("SETTINGS_REPORTS"),
            None,
            Some(json!({
                "report_type": payload.report_type,
                "count": issued_docs.len(),
                "first_doc": issued_docs.first().map(|d| d["document_number"].clone()),
                "last_doc": issued_docs.last().map(|d| d["document_number"].clone()),
                "new_current_number": current_counter
            })),
            None,
        )
        .await;

        let next_formatted = format_document_number(
            &seq.format_pattern,
            &seq.prefix,
            seq.suffix.as_deref().unwrap_or(""),
            current_counter + 1,
            seq.padding_length,
            payload
                .academic_year
                .as_deref()
                .or(seq.academic_year.as_deref()),
        );

        Ok((
            issued_docs,
            seq.display_name,
            current_counter,
            next_formatted,
        ))
    }

    pub async fn list_issued_reports(
        db: &PgPool,
        school_id: Uuid,
        query: ListIssuedReportsQuery,
    ) -> Result<(Vec<IssuedReport>, i64, i64, i64), AppError> {
        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(25).clamp(1, 100);

        let (rows, total) =
            TransferCertificateRepository::list_issued_reports(db, school_id, &query).await?;

        Ok((rows, total, page, limit))
    }

    pub async fn cancel_issued_report(
        db: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        report_id: Uuid,
        payload: CancelIssuedReportPayload,
    ) -> Result<Uuid, AppError> {
        let reason_note = payload
            .reason
            .unwrap_or_else(|| "Cancelled by administrative action".into());

        let canceller_name = sqlx::query_scalar::<_, Option<String>>(
            "SELECT COALESCE(display_name, username) FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .flatten()
        .or_else(|| Some("System Administrator".to_string()));

        // 1. Fetch document details before beginning transaction
        let report_details = sqlx::query_as::<_, (String, Option<Uuid>, String)>(
            "SELECT report_type, student_id, document_number FROM issued_reports WHERE id = $1 AND school_id = $2",
        )
        .bind(report_id)
        .bind(school_id)
        .fetch_optional(db)
        .await?;

        let (report_type, maybe_student_id, doc_number) = match report_details {
            Some(r) => r,
            None => {
                return Err(AppError::NotFound(
                    "Issued report document not found".into(),
                ))
            }
        };

        let mut tx = db.begin().await?;

        let affected = TransferCertificateRepository::cancel_issued_report_tx(
            &mut tx,
            school_id,
            report_id,
            user_id,
            canceller_name,
            &reason_note,
        )
        .await?;

        if affected == 0 {
            return Err(AppError::NotFound(
                "Issued report document not found".into(),
            ));
        }

        // 2. If a Transfer Certificate is cancelled, restore student to ACTIVE and reinstate enrollments
        let is_tc = report_type.to_lowercase().contains("transfer");
        if is_tc {
            if let Some(student_id) = maybe_student_id {
                let other_active_tc = sqlx::query_scalar::<_, bool>(
                    r#"SELECT EXISTS (
                        SELECT 1 FROM issued_reports
                        WHERE school_id = $1 AND student_id = $2 
                          AND id != $3 AND LOWER(report_type) LIKE '%transfer%' 
                          AND status = 'ISSUED'
                    )"#,
                )
                .bind(school_id)
                .bind(student_id)
                .bind(report_id)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);

                if !other_active_tc {
                    // Restore student status to ACTIVE
                    let _ = sqlx::query(
                        r#"UPDATE users
                           SET status = 'ACTIVE',
                               updated_at = NOW(),
                               updated_by_user_id = $1
                           WHERE id = $2 AND school_id = $3"#,
                    )
                    .bind(user_id)
                    .bind(student_id)
                    .bind(school_id)
                    .execute(&mut *tx)
                    .await;

                    // Restore most recent enrollment to ACTIVE
                    let _ = sqlx::query(
                        r#"UPDATE enrollments
                           SET status = 'ACTIVE', updated_at = NOW()
                           WHERE student_id = $1 AND id = (
                               SELECT id FROM enrollments WHERE student_id = $1 ORDER BY updated_at DESC LIMIT 1
                           )"#,
                    )
                    .bind(student_id)
                    .execute(&mut *tx)
                    .await;

                    // Restore dropped subjects to ACTIVE
                    let _ = sqlx::query(
                        r#"UPDATE student_subject_enrollments
                           SET status = 'ACTIVE', updated_at = NOW()
                           WHERE student_id = $1 AND status = 'DROPPED'"#,
                    )
                    .bind(student_id)
                    .execute(&mut *tx)
                    .await;

                    // Audit in entity_change_history
                    let _ = sqlx::query(
                        r#"INSERT INTO entity_change_history (
                            school_id, entity_type, entity_id, action,
                            updated_by_user_id, before_state, after_state, created_at
                        ) VALUES (
                            $1, 'STUDENT_TC', $2, 'RESTORE', $3,
                            jsonb_build_object('status', 'TC_ISSUED', 'cancelled_doc', $4),
                            jsonb_build_object('status', 'ACTIVE', 'cancelled_doc', $4),
                            NOW()
                        )"#,
                    )
                    .bind(school_id)
                    .bind(student_id)
                    .bind(user_id)
                    .bind(&doc_number)
                    .execute(&mut *tx)
                    .await;

                    crate::modules::auth::backend::invalidate_user_cache(&student_id);
                }
            }
        }

        tx.commit().await?;

        crate::modules::users::repository::UserRepository::invalidate_users_cache(Some(school_id));

        let _ = log_action(
            db,
            Some(school_id),
            Some(user_id),
            "CANCEL_ISSUED_REPORT",
            Some("SETTINGS_REPORTS"),
            None,
            Some(json!({ "document_id": report_id, "reason": reason_note })),
            None,
        )
        .await;

        Ok(report_id)
    }

    pub async fn issue_transfer_certificates(
        db: &PgPool,
        school_id: Uuid,
        user_id: Uuid,
        _is_super_admin: bool,
        payload: IssueTransferCertificatesRequest,
    ) -> Result<IssueTransferCertificatesResponse, AppError> {
        if payload.user_ids.is_empty() {
            return Err(AppError::BadRequest(
                "No user IDs provided for TC issuance".into(),
            ));
        }

        let issuer_name = sqlx::query_scalar::<_, Option<String>>(
            "SELECT COALESCE(display_name, username) FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .flatten()
        .unwrap_or_else(|| "School Administrator".to_string());

        let issue_payload = IssueReportPayload {
            report_type: "transfer_certificate".to_string(),
            student_ids: payload.user_ids,
            academic_year: None,
            custom_variables: None,
            remarks: payload.reason,
        };

        let (issued_docs, _, _, _) =
            Self::issue_report(db, school_id, user_id, &issuer_name, issue_payload).await?;

        Ok(IssueTransferCertificatesResponse {
            success: true,
            issued_count: issued_docs.len(),
            message: format!(
                "Successfully issued Transfer Certificate(s) for {} student(s)",
                issued_docs.len()
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_document_number_standard() {
        let res = format_document_number(
            "{prefix}/{year}/{number}",
            "TC",
            "",
            42,
            4,
            Some("2026-2027"),
        );
        let cur_year = chrono::Local::now().format("%Y").to_string();
        assert_eq!(res, format!("TC/{cur_year}/0042"));
    }

    #[test]
    fn test_format_document_number_with_academic_year_and_suffix() {
        let res = format_document_number(
            "{prefix}/{academic_year}/{number}{suffix}",
            "TC",
            "-A",
            5,
            6,
            Some("2026-2027"),
        );
        assert_eq!(res, "TC/2026-2027/000005-A");
    }

    #[test]
    fn test_format_document_number_custom_padding() {
        let res = format_document_number("{prefix}-{number}", "CERT", "", 123, 2, None);
        assert_eq!(res, "CERT-123");
    }
}
