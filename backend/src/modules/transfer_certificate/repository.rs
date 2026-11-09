use super::dto::{ListIssuedReportsQuery, UpdateSequencePayload};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReportSequence {
    pub id: Uuid,
    pub school_id: Uuid,
    pub report_type: String,
    pub display_name: String,
    pub prefix: String,
    pub suffix: Option<String>,
    pub current_number: i32,
    pub padding_length: i32,
    pub format_pattern: String,
    pub academic_year: Option<String>,
    pub is_locked: bool,
    pub reset_annually: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IssuedReport {
    pub id: Uuid,
    pub school_id: Uuid,
    pub report_type: String,
    pub document_number: String,
    pub serial_number: i32,
    pub student_id: Option<Uuid>,
    pub student_name: String,
    pub admission_number: Option<String>,
    pub class_section: Option<String>,
    pub issued_by: Option<Uuid>,
    pub issuer_name: Option<String>,
    pub issue_date: chrono::NaiveDate,
    pub academic_year: Option<String>,
    pub status: String,
    pub variables_snapshot: Value,
    pub template_version: Option<i32>,
    pub remarks: Option<String>,
    pub cancelled_by: Option<Uuid>,
    pub cancelled_by_name: Option<String>,
    pub cancelled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub cancellation_reason: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct TransferCertificateRepository;

impl TransferCertificateRepository {
    pub async fn ensure_default_sequences(db: &PgPool, school_id: Uuid) -> Result<(), sqlx::Error> {
        let defaults = [
            (
                "transfer_certificate",
                "Transfer Certificate",
                "TC",
                4,
                "{prefix}/{year}/{number}",
            ),
            (
                "bonafide_certificate",
                "Bonafide Certificate",
                "BC",
                4,
                "{prefix}/{year}/{number}",
            ),
            (
                "conduct_certificate",
                "Conduct Certificate",
                "CC",
                4,
                "{prefix}/{year}/{number}",
            ),
            (
                "study_certificate",
                "Study Certificate",
                "SC",
                4,
                "{prefix}/{year}/{number}",
            ),
            (
                "report_card",
                "Report Card / Marksheet",
                "RC",
                4,
                "{prefix}/{year}/{number}",
            ),
            (
                "id_card",
                "Student ID Card",
                "ID",
                4,
                "{prefix}/{year}/{number}",
            ),
        ];

        for (rtype, name, prefix, pad, pat) in defaults {
            sqlx::query(
                r#"
                INSERT INTO report_sequences (school_id, report_type, display_name, prefix, padding_length, format_pattern)
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (school_id, report_type) DO NOTHING
                "#,
            )
            .bind(school_id)
            .bind(rtype)
            .bind(name)
            .bind(prefix)
            .bind(pad)
            .bind(pat)
            .execute(db)
            .await?;
        }
        Ok(())
    }

    pub async fn list_sequences(
        db: &PgPool,
        school_id: Uuid,
    ) -> Result<Vec<ReportSequence>, sqlx::Error> {
        sqlx::query_as::<_, ReportSequence>(
            r#"
            SELECT id, school_id, report_type, display_name, prefix, suffix, current_number,
                   padding_length, format_pattern, academic_year, is_locked, reset_annually,
                   created_at, updated_at
            FROM report_sequences
            WHERE school_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(school_id)
        .fetch_all(db)
        .await
    }

    pub async fn get_sequence(
        db: &PgPool,
        school_id: Uuid,
        report_type: &str,
    ) -> Result<Option<ReportSequence>, sqlx::Error> {
        sqlx::query_as::<_, ReportSequence>(
            r#"
            SELECT id, school_id, report_type, display_name, prefix, suffix, current_number,
                   padding_length, format_pattern, academic_year, is_locked, reset_annually,
                   created_at, updated_at
            FROM report_sequences
            WHERE school_id = $1 AND report_type = $2
            "#,
        )
        .bind(school_id)
        .bind(report_type)
        .fetch_optional(db)
        .await
    }

    pub async fn get_sequence_for_update(
        tx: &mut Transaction<'_, Postgres>,
        school_id: Uuid,
        report_type: &str,
    ) -> Result<Option<ReportSequence>, sqlx::Error> {
        sqlx::query_as::<_, ReportSequence>(
            r#"
            SELECT id, school_id, report_type, display_name, prefix, suffix, current_number,
                   padding_length, format_pattern, academic_year, is_locked, reset_annually,
                   created_at, updated_at
            FROM report_sequences
            WHERE school_id = $1 AND report_type = $2
            FOR UPDATE
            "#,
        )
        .bind(school_id)
        .bind(report_type)
        .fetch_optional(&mut **tx)
        .await
    }

    pub async fn update_sequence(
        db: &PgPool,
        school_id: Uuid,
        report_type: &str,
        payload: &UpdateSequencePayload,
    ) -> Result<ReportSequence, sqlx::Error> {
        let mut qb = sqlx::QueryBuilder::new("UPDATE report_sequences SET updated_at = NOW()");
        if let Some(display_name) = &payload.display_name {
            qb.push(", display_name = ");
            qb.push_bind(display_name);
        }
        if let Some(prefix) = &payload.prefix {
            qb.push(", prefix = ");
            qb.push_bind(prefix);
        }
        if let Some(suffix) = &payload.suffix {
            qb.push(", suffix = ");
            qb.push_bind(suffix);
        }
        if let Some(current_number) = payload.current_number {
            qb.push(", current_number = ");
            qb.push_bind(current_number);
        }
        if let Some(padding_length) = payload.padding_length {
            qb.push(", padding_length = ");
            qb.push_bind(padding_length);
        }
        if let Some(format_pattern) = &payload.format_pattern {
            qb.push(", format_pattern = ");
            qb.push_bind(format_pattern);
        }
        if let Some(academic_year) = &payload.academic_year {
            qb.push(", academic_year = ");
            qb.push_bind(academic_year);
        }
        if let Some(is_locked) = payload.is_locked {
            qb.push(", is_locked = ");
            qb.push_bind(is_locked);
        }
        if let Some(reset_annually) = payload.reset_annually {
            qb.push(", reset_annually = ");
            qb.push_bind(reset_annually);
        }

        qb.push(" WHERE school_id = ");
        qb.push_bind(school_id);
        qb.push(" AND report_type = ");
        qb.push_bind(report_type);
        qb.push(
            r#" RETURNING id, school_id, report_type, display_name, prefix, suffix, current_number,
                          padding_length, format_pattern, academic_year, is_locked, reset_annually,
                          created_at, updated_at"#,
        );

        qb.build_query_as::<ReportSequence>().fetch_one(db).await
    }

    pub async fn update_sequence_counter(
        tx: &mut Transaction<'_, Postgres>,
        sequence_id: Uuid,
        new_counter: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE report_sequences
            SET current_number = $1, updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(new_counter)
        .bind(sequence_id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn get_template_value(
        db: &PgPool,
        school_id: Uuid,
        key: &str,
    ) -> Result<Option<Value>, sqlx::Error> {
        sqlx::query_scalar::<_, Value>(
            r#"
            SELECT value
            FROM school_settings
            WHERE school_id = $1 AND key = $2
            "#,
        )
        .bind(school_id)
        .bind(key)
        .fetch_optional(db)
        .await
    }

    pub async fn save_template_value(
        db: &PgPool,
        school_id: Uuid,
        key: &str,
        value: &Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO school_settings (school_id, key, value, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (school_id, key)
            DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
            "#,
        )
        .bind(school_id)
        .bind(key)
        .bind(value)
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn clear_otps_for_action(
        db: &PgPool,
        user_id: Uuid,
        action_type: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"DELETE FROM admin_action_otps
               WHERE user_id = $1 AND action_type = $2"#,
        )
        .bind(user_id)
        .bind(action_type)
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn insert_otp(
        db: &PgPool,
        user_id: Uuid,
        action_type: &str,
        otp_code: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO admin_action_otps (user_id, action_type, otp_code, expires_at)
               VALUES ($1, $2, $3, NOW() + INTERVAL '10 minutes')"#,
        )
        .bind(user_id)
        .bind(action_type)
        .bind(otp_code)
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn get_active_otp(
        db: &PgPool,
        user_id: Uuid,
        action_type: &str,
    ) -> Result<Option<(Uuid, String)>, sqlx::Error> {
        sqlx::query_as::<_, (Uuid, String)>(
            r#"SELECT id, otp_code
               FROM admin_action_otps
               WHERE user_id = $1 AND action_type = $2 AND expires_at > NOW()
               ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(user_id)
        .bind(action_type)
        .fetch_optional(db)
        .await
    }

    pub async fn delete_otp(db: &PgPool, otp_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM admin_action_otps WHERE id = $1")
            .bind(otp_id)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn fetch_student_details(
        tx: &mut Transaction<'_, Postgres>,
        student_id: Uuid,
        school_id: Uuid,
    ) -> Result<
        Option<(
            Uuid,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )>,
        sqlx::Error,
    > {
        sqlx::query_as::<
            _,
            (
                Uuid,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            SELECT u.id, u.first_name, u.last_name, u.display_name, u.admission_number,
                   sec.name as section_name, c.name as class_name
            FROM users u
            LEFT JOIN enrollments e ON e.student_id = u.id AND e.status = 'ACTIVE'
            LEFT JOIN class_sections cs ON cs.id = COALESCE(e.class_section_id, u.class_section_id)
            LEFT JOIN classes c ON c.id = cs.class_id
            LEFT JOIN sections sec ON sec.id = cs.section_id
            WHERE u.id = $1 AND u.school_id = $2
            "#,
        )
        .bind(student_id)
        .bind(school_id)
        .fetch_optional(&mut **tx)
        .await
    }

    pub async fn insert_issued_report(
        tx: &mut Transaction<'_, Postgres>,
        school_id: Uuid,
        report_type: &str,
        document_number: &str,
        serial_number: i32,
        student_id: Uuid,
        student_name: &str,
        admission_number: Option<&str>,
        class_section: Option<&str>,
        issued_by: Uuid,
        issuer_name: &str,
        academic_year: Option<&str>,
        variables_snapshot: &Value,
        remarks: Option<&str>,
    ) -> Result<(Uuid, String, i32, chrono::NaiveDate, String), sqlx::Error> {
        sqlx::query_as::<_, (Uuid, String, i32, chrono::NaiveDate, String)>(
            r#"
            INSERT INTO issued_reports (
                school_id, report_type, document_number, serial_number,
                student_id, student_name, admission_number, class_section,
                issued_by, issuer_name, issue_date, academic_year,
                status, variables_snapshot, remarks, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, $7, $8,
                $9, $10, CURRENT_DATE, $11,
                'ISSUED', $12, $13, NOW(), NOW()
            )
            RETURNING id, document_number, serial_number, issue_date, status
            "#,
        )
        .bind(school_id)
        .bind(report_type)
        .bind(document_number)
        .bind(serial_number)
        .bind(student_id)
        .bind(student_name)
        .bind(admission_number)
        .bind(class_section)
        .bind(issued_by)
        .bind(issuer_name)
        .bind(academic_year)
        .bind(variables_snapshot)
        .bind(remarks)
        .fetch_one(&mut **tx)
        .await
    }

    pub async fn list_issued_reports(
        db: &PgPool,
        school_id: Uuid,
        query: &ListIssuedReportsQuery,
    ) -> Result<(Vec<IssuedReport>, i64), sqlx::Error> {
        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(25).clamp(1, 100);
        let offset = (page - 1) * limit;
        let search_pattern = query.search.as_ref().map(|s| format!("%{}%", s.trim()));

        let mut qb = sqlx::QueryBuilder::new(
            r#"
            SELECT id, school_id, report_type, document_number, serial_number, student_id,
                   student_name, admission_number, class_section, issued_by, issuer_name,
                   issue_date, academic_year, status, variables_snapshot, template_version,
                   remarks, cancelled_by, cancelled_by_name, cancelled_at, cancellation_reason,
                   created_at, updated_at
            FROM issued_reports
            WHERE school_id = 
            "#,
        );
        qb.push_bind(school_id);

        if let Some(ref rtype) = query.report_type {
            if !rtype.is_empty() && rtype != "all" {
                qb.push(" AND report_type = ");
                qb.push_bind(rtype);
            }
        }
        if let Some(student_id) = query.student_id {
            qb.push(" AND student_id = ");
            qb.push_bind(student_id);
        }
        if let Some(ref status) = query.status {
            if !status.is_empty() && status != "all" {
                qb.push(" AND status = ");
                qb.push_bind(status);
            }
        }
        if let Some(ref pattern) = search_pattern {
            qb.push(" AND (document_number ILIKE ");
            qb.push_bind(pattern);
            qb.push(" OR student_name ILIKE ");
            qb.push_bind(pattern);
            qb.push(" OR admission_number ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        let mut count_qb =
            sqlx::QueryBuilder::new("SELECT COUNT(*) FROM issued_reports WHERE school_id = ");
        count_qb.push_bind(school_id);
        if let Some(ref rtype) = query.report_type {
            if !rtype.is_empty() && rtype != "all" {
                count_qb.push(" AND report_type = ");
                count_qb.push_bind(rtype);
            }
        }
        if let Some(student_id) = query.student_id {
            count_qb.push(" AND student_id = ");
            count_qb.push_bind(student_id);
        }
        if let Some(ref status) = query.status {
            if !status.is_empty() && status != "all" {
                count_qb.push(" AND status = ");
                count_qb.push_bind(status);
            }
        }
        if let Some(ref pattern) = search_pattern {
            count_qb.push(" AND (document_number ILIKE ");
            count_qb.push_bind(pattern);
            count_qb.push(" OR student_name ILIKE ");
            count_qb.push_bind(pattern);
            count_qb.push(" OR admission_number ILIKE ");
            count_qb.push_bind(pattern);
            count_qb.push(")");
        }

        let total: i64 = count_qb
            .build_query_scalar()
            .fetch_one(db)
            .await
            .unwrap_or(0);

        qb.push(" ORDER BY created_at DESC LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let rows = qb.build_query_as::<IssuedReport>().fetch_all(db).await?;
        Ok((rows, total))
    }

    pub async fn cancel_issued_report(
        db: &PgPool,
        school_id: Uuid,
        report_id: Uuid,
        cancelled_by: Uuid,
        cancelled_by_name: Option<String>,
        reason: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE issued_reports
            SET status = 'CANCELLED',
                cancelled_by = $1,
                cancelled_by_name = $2,
                cancelled_at = NOW(),
                cancellation_reason = $3,
                remarks = COALESCE(remarks || ' | Cancelled: ' || $3, 'Cancelled: ' || $3),
                updated_at = NOW()
            WHERE id = $4 AND school_id = $5
            "#,
        )
        .bind(cancelled_by)
        .bind(cancelled_by_name)
        .bind(reason)
        .bind(report_id)
        .bind(school_id)
        .execute(db)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn mark_student_tc_issued(
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        school_id: Uuid,
        is_super_admin: bool,
        updated_by: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"UPDATE users
               SET status = 'TC_ISSUED',
                   updated_at = NOW(),
                   updated_by_user_id = $1
               WHERE id = $2 AND (school_id = $3 OR $4)"#,
        )
        .bind(updated_by)
        .bind(user_id)
        .bind(school_id)
        .bind(is_super_admin)
        .execute(&mut **tx)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn deactivate_active_enrollments(
        tx: &mut Transaction<'_, Postgres>,
        student_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"UPDATE enrollments
               SET status = 'INACTIVE', updated_at = NOW()
               WHERE student_id = $1 AND status = 'ACTIVE'"#,
        )
        .bind(student_id)
        .execute(&mut **tx)
        .await?;

        Ok(result.rows_affected())
    }
}
