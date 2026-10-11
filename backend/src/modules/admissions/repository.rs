use super::dto::{AdmissionApplicantDto, AdmissionGuardianDto, AdmissionItemDto};
use crate::error::AppError;
use sqlx::{PgConnection, PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

pub struct AdmissionRepository;

impl AdmissionRepository {
    pub async fn create_admission_submission(
        conn: &mut PgConnection,
        school_id: Uuid,
        first_name: &str,
        middle_name: Option<&str>,
        last_name: &str,
        dob: time::Date,
        gender: Option<&str>,
        phone: Option<&str>,
        email: Option<&str>,
        guardian_name: &str,
        guardian_relationship: &str,
        guardian_phone: &str,
        guardian_email: Option<&str>,
        guardian_occupation: Option<&str>,
        custom_fields: &HashMap<String, String>,
        creator_user_id: Uuid,
    ) -> Result<Uuid, AppError> {
        let admission_id: Uuid = sqlx::query_scalar(
            "INSERT INTO admissions (school_id, status, submitted_at) VALUES ($1, 'PENDING', now()) RETURNING id",
        )
        .bind(school_id)
        .fetch_one(&mut *conn)
        .await?;

        sqlx::query(
            r#"INSERT INTO admission_applicants
               (admission_id, first_name, middle_name, last_name, date_of_birth, gender, phone, email)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(admission_id)
        .bind(first_name)
        .bind(middle_name)
        .bind(last_name)
        .bind(dob)
        .bind(gender)
        .bind(phone)
        .bind(email)
        .execute(&mut *conn)
        .await?;

        sqlx::query(
            r#"INSERT INTO admission_guardians
               (admission_id, name, relationship, phone, email, occupation)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(admission_id)
        .bind(guardian_name)
        .bind(guardian_relationship)
        .bind(guardian_phone)
        .bind(guardian_email)
        .bind(guardian_occupation)
        .execute(&mut *conn)
        .await?;

        sqlx::query(
            r#"INSERT INTO admission_status_history (admission_id, to_status, changed_by, reason)
               VALUES ($1, 'PENDING', $2, 'Initial Submission')"#,
        )
        .bind(admission_id)
        .bind(creator_user_id)
        .execute(&mut *conn)
        .await?;

        for (key, value) in custom_fields {
            let field_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM custom_fields WHERE school_id = $1 AND entity_type = 'ADMISSION' AND field_key = $2",
            )
            .bind(school_id)
            .bind(key)
            .fetch_optional(&mut *conn)
            .await?;

            if let Some(fid) = field_id {
                sqlx::query(
                    "INSERT INTO admission_custom_values (admission_id, field_id, value) VALUES ($1, $2, $3)",
                )
                .bind(admission_id)
                .bind(fid)
                .bind(value)
                .execute(&mut *conn)
                .await?;
            }
        }

        Ok(admission_id)
    }

    pub async fn enroll_admission(
        conn: &mut PgConnection,
        school_id: Uuid,
        admission_id: Uuid,
        user_id: Uuid,
    ) -> Result<String, AppError> {
        let admission = sqlx::query(
            "SELECT status::text FROM admissions WHERE id = $1 AND school_id = $2 FOR UPDATE",
        )
        .bind(admission_id)
        .bind(school_id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| AppError::NotFound("Admission not found".into()))?;

        let status: String = admission.try_get("status").unwrap();
        if status != "APPROVED" && status != "PENDING" {
            return Err(AppError::BadRequest(
                "Admission cannot be enrolled. It is either already enrolled or rejected.".into(),
            ));
        }

        let applicant = sqlx::query(
            "SELECT first_name, middle_name, last_name, date_of_birth FROM admission_applicants WHERE admission_id = $1",
        )
        .bind(admission_id)
        .fetch_one(&mut *conn)
        .await?;

        let applicant_first_name: String = applicant.try_get("first_name").unwrap();
        let applicant_last_name: String = applicant.try_get("last_name").unwrap();
        let date_of_birth: time::Date = applicant.try_get("date_of_birth").unwrap();

        let guardian = sqlx::query(
            "SELECT name, phone, email, relationship FROM admission_guardians WHERE admission_id = $1 LIMIT 1",
        )
        .bind(admission_id)
        .fetch_one(&mut *conn)
        .await?;

        let guardian_name: String = guardian.try_get("name").unwrap();
        let guardian_phone: String = guardian.try_get("phone").unwrap();
        let guardian_email: Option<String> = guardian.try_get("email").ok();

        let default_pass = crate::utils::password::hash_password("Welcome123!").unwrap();

        let s_username = format!(
            "{}.{}",
            applicant_first_name.to_lowercase(),
            admission_id.to_string().chars().take(5).collect::<String>()
        );
        let admission_number = format!(
            "ADM-{}",
            admission_id
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
                .to_uppercase()
        );

        let s_user_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO users (school_id, first_name, last_name, username, password_hash, status, imported_via_csv, updated_by_user_id, admission_number, date_of_birth, updated_at)
               VALUES ($1, $2, $3, $4, $5, 'ACTIVE', false, $6, $7, $8, NOW()) RETURNING id"#,
        )
        .bind(school_id)
        .bind(&applicant_first_name)
        .bind(&applicant_last_name)
        .bind(&s_username)
        .bind(&default_pass)
        .bind(user_id)
        .bind(&admission_number)
        .bind(date_of_birth)
        .fetch_one(&mut *conn)
        .await?;

        crate::utils::roles::assign_user_role(&mut *conn, school_id, s_user_id, "STUDENT").await?;

        let names: Vec<&str> = guardian_name.split_whitespace().collect();
        let g_first = names.first().unwrap_or(&"Guardian");
        let g_last = names.get(1).unwrap_or(&"");

        let g_username = format!("{}_{}", g_first.to_lowercase(), guardian_phone);
        let g_user_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status, imported_via_csv, updated_by_user_id, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE', false, $8, NOW()) RETURNING id"#,
        )
        .bind(school_id)
        .bind(g_first)
        .bind(g_last)
        .bind(&guardian_email)
        .bind(&guardian_phone)
        .bind(&g_username)
        .bind(&default_pass)
        .bind(user_id)
        .fetch_one(&mut *conn)
        .await?;

        crate::utils::roles::assign_user_role(&mut *conn, school_id, g_user_id, "PARENT").await?;

        let guardian_record_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO guardians (school_id, user_id, first_name, last_name, email, phone, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, NOW()) RETURNING id"#,
        )
        .bind(school_id)
        .bind(g_user_id)
        .bind(g_first)
        .bind(g_last)
        .bind(&guardian_email)
        .bind(&guardian_phone)
        .fetch_one(&mut *conn)
        .await?;

        sqlx::query(
            r#"INSERT INTO student_guardians (student_id, guardian_id, relationship) VALUES ($1, $2, 'PARENT')"#,
        )
        .bind(s_user_id)
        .bind(guardian_record_id)
        .execute(&mut *conn)
        .await?;

        sqlx::query(
            "UPDATE admissions SET status = 'ENROLLED', application_number = $1, approved_at = now(), approved_by = $2, updated_at = now() WHERE id = $3",
        )
        .bind(&admission_number)
        .bind(user_id)
        .bind(admission_id)
        .execute(&mut *conn)
        .await?;

        let active_ay_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM academic_years WHERE school_id = $1 AND status = 'ACTIVE' LIMIT 1",
        )
        .bind(school_id)
        .fetch_optional(&mut *conn)
        .await
        .ok()
        .flatten();

        if let Some(ay_id) = active_ay_id {
            sqlx::query(
                r#"INSERT INTO enrollments (school_id, academic_year_id, student_id, status)
                   VALUES ($1, $2, $3, 'ACTIVE')
                   ON CONFLICT (student_id, academic_year_id) DO UPDATE SET status = 'ACTIVE'"#,
            )
            .bind(school_id)
            .bind(ay_id)
            .bind(s_user_id)
            .execute(&mut *conn)
            .await?;
        }

        sqlx::query(
            r#"INSERT INTO admission_status_history (admission_id, from_status, to_status, changed_by, reason)
               VALUES ($1, 'APPROVED', 'ENROLLED', $2, 'System Enrollment')"#,
        )
        .bind(admission_id)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;

        Ok(admission_number)
    }

    pub async fn list_admissions(
        pool: &PgPool,
        school_id: Uuid,
        status_filter: Option<&str>,
    ) -> Result<Vec<AdmissionItemDto>, AppError> {
        let rows = sqlx::query(
            r#"SELECT
                a.id,
                a.school_id,
                a.application_number,
                a.status::text as status,
                a.submitted_at::text as submitted_at,
                a.created_at::text as created_at,
                ap.first_name,
                ap.middle_name,
                ap.last_name,
                ap.date_of_birth::text as date_of_birth,
                ap.gender,
                ap.phone as applicant_phone,
                ap.email as applicant_email,
                g.name as guardian_name,
                g.relationship as guardian_relationship,
                g.phone as guardian_phone,
                g.email as guardian_email,
                g.occupation as guardian_occupation
            FROM admissions a
            JOIN admission_applicants ap ON ap.admission_id = a.id
            JOIN admission_guardians g ON g.admission_id = a.id
            WHERE a.school_id = $1
              AND ($2::text IS NULL OR a.status::text = $2)
            ORDER BY a.created_at DESC"#,
        )
        .bind(school_id)
        .bind(status_filter)
        .fetch_all(pool)
        .await?;

        let mut admissions = Vec::new();
        for r in rows {
            let id: Uuid = r.try_get("id")?;
            let school_id: Uuid = r.try_get("school_id")?;
            let application_number: Option<String> = r.try_get("application_number").ok();
            let status: String = r.try_get("status")?;
            let submitted_at: Option<String> = r.try_get("submitted_at").ok();
            let created_at: String = r.try_get("created_at")?;

            let applicant = AdmissionApplicantDto {
                first_name: r.try_get("first_name")?,
                middle_name: r.try_get("middle_name").ok(),
                last_name: r.try_get("last_name")?,
                date_of_birth: r.try_get("date_of_birth")?,
                gender: r.try_get("gender").ok(),
                phone: r.try_get("applicant_phone").ok(),
                email: r.try_get("applicant_email").ok(),
            };

            let guardian = AdmissionGuardianDto {
                name: r.try_get("guardian_name")?,
                relationship: r.try_get("guardian_relationship")?,
                phone: r.try_get("guardian_phone")?,
                email: r.try_get("guardian_email").ok(),
                occupation: r.try_get("guardian_occupation").ok(),
            };

            admissions.push(AdmissionItemDto {
                id,
                school_id,
                application_number,
                status,
                submitted_at,
                applicant,
                guardian,
                custom_fields: HashMap::new(),
                created_at,
            });
        }

        Ok(admissions)
    }

    pub async fn update_status(
        conn: &mut PgConnection,
        school_id: Uuid,
        admission_id: Uuid,
        new_status: &str,
        changed_by: Uuid,
        reason: Option<&str>,
    ) -> Result<(), AppError> {
        let current_status: String = sqlx::query_scalar(
            "SELECT status::text FROM admissions WHERE id = $1 AND school_id = $2 FOR UPDATE",
        )
        .bind(admission_id)
        .bind(school_id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| AppError::NotFound("Admission record not found".into()))?;

        sqlx::query(
            "UPDATE admissions SET status = $1::admission_status, updated_at = now() WHERE id = $2 AND school_id = $3",
        )
        .bind(new_status)
        .bind(admission_id)
        .bind(school_id)
        .execute(&mut *conn)
        .await?;

        sqlx::query(
            r#"INSERT INTO admission_status_history (admission_id, from_status, to_status, changed_by, reason)
               VALUES ($1, $2::admission_status, $3::admission_status, $4, $5)"#,
        )
        .bind(admission_id)
        .bind(&current_status)
        .bind(new_status)
        .bind(changed_by)
        .bind(reason)
        .execute(&mut *conn)
        .await?;

        Ok(())
    }
}
