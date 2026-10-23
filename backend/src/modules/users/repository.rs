use crate::error::AppError;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

#[derive(Debug, serde::Serialize)]
pub struct UserListItem {
    pub id: Uuid,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub display_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub status: Option<String>,
    pub roles: Vec<String>,
    pub admission_number: Option<String>,
    pub employee_id: Option<String>,
    pub gender: Option<String>,
    pub blood_group: Option<String>,
    pub date_of_birth: Option<String>,
    pub class_name: Option<String>,
    pub section_name: Option<String>,
    pub roll_number: Option<String>,
    pub guardian_name: Option<String>,
    pub guardian_phone: Option<String>,
    pub guardian_relationship: Option<String>,
    pub created_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, serde::Serialize)]
pub struct AdmissionApplicantRow {
    pub id: Uuid,
    pub admission_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub dob: time::Date,
    pub gender: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

pub struct UserRepository;

#[allow(dead_code)]
impl UserRepository {
    pub async fn check_user_exists_by_email_or_phone(
        pool: &PgPool,
        email: &str,
        phone: &str,
    ) -> Result<bool, AppError> {
        let exists: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM users WHERE email = $1 OR phone = $2 LIMIT 1")
                .bind(email)
                .bind(phone)
                .fetch_optional(pool)
                .await?;

        Ok(exists.is_some())
    }

    pub async fn create_admin_user(
        pool: &PgPool,
        school_id: Uuid,
        first_name: &str,
        last_name: &str,
        email: &str,
        phone: &str,
        username: &str,
        password_hash: &str,
        creator_user_id: Uuid,
    ) -> Result<Uuid, AppError> {
        let mut tx_guard = crate::utils::write_controller::DbWriteController::begin(pool).await?;
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status, email_verified, phone_verified, updated_by_user_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'INACTIVE', false, false, $8)
            RETURNING id
            "#
        )
        .bind(school_id)
        .bind(first_name)
        .bind(last_name)
        .bind(email)
        .bind(phone)
        .bind(username)
        .bind(password_hash)
        .bind(creator_user_id)
        .fetch_one(tx_guard.conn())
        .await?;

        crate::utils::roles::assign_user_role(tx_guard.conn(), school_id, id, "SCHOOL_ADMIN")
            .await?;

        tx_guard.commit().await?;
        Ok(id)
    }

    fn append_user_filter_clauses(
        qb: &mut QueryBuilder<sqlx::Postgres>,
        filter_role: &str,
        status: &str,
        admitted_from: &str,
        admitted_to: &str,
        admission_number: &str,
        class_name: &str,
        search: &str,
    ) {
        if !filter_role.is_empty() {
            let upper = filter_role.to_uppercase();
            if upper == "STUDENT" {
                qb.push(" AND (EXISTS (SELECT 1 FROM user_roles ur JOIN roles r ON ur.role_id = r.id WHERE ur.user_id = u.id AND r.code = 'STUDENT') OR u.admission_number IS NOT NULL)");
            } else if upper == "TEACHER" {
                qb.push(" AND EXISTS (SELECT 1 FROM user_roles ur JOIN roles r ON ur.role_id = r.id WHERE ur.user_id = u.id AND r.code IN ('TEACHER', 'STAFF'))");
            } else if upper == "SCHOOL_ADMIN" {
                qb.push(" AND EXISTS (SELECT 1 FROM user_roles ur JOIN roles r ON ur.role_id = r.id WHERE ur.user_id = u.id AND r.code IN ('SCHOOL_ADMIN', 'SUPER_ADMIN'))");
            } else {
                qb.push(" AND EXISTS (SELECT 1 FROM user_roles ur JOIN roles r ON ur.role_id = r.id WHERE ur.user_id = u.id AND (r.code = ");
                qb.push_bind(filter_role.to_string());
                qb.push(" OR r.code = ");
                qb.push_bind(upper);
                qb.push("))");
            }
        }

        if !status.is_empty() {
            qb.push(" AND u.status::TEXT ILIKE ");
            qb.push_bind(status.to_string());
        }

        if !admitted_from.is_empty() {
            qb.push(" AND u.created_at >= ");
            qb.push_bind(admitted_from.to_string());
            qb.push("::timestamptz");
        }

        if !admitted_to.is_empty() {
            qb.push(" AND u.created_at <= (");
            qb.push_bind(admitted_to.to_string());
            qb.push("::timestamptz + interval '1 day')");
        }

        if !admission_number.is_empty() {
            qb.push(" AND u.admission_number IS NOT NULL AND u.admission_number ILIKE ");
            qb.push_bind(format!("%{}%", admission_number));
        }

        if !class_name.is_empty() {
            qb.push(" AND EXISTS (SELECT 1 FROM class_sections cs2 JOIN classes c2 ON cs2.class_id = c2.id WHERE (cs2.id = u.class_section_id OR EXISTS (SELECT 1 FROM enrollments e2 WHERE e2.student_id = u.id AND e2.status = 'ACTIVE' AND e2.class_section_id = cs2.id)) AND c2.name ILIKE ");
            qb.push_bind(format!("%{}%", class_name));
            qb.push(")");
        }

        if !search.is_empty() {
            let pat = format!("%{}%", search);
            qb.push(" AND (u.username ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR u.display_name ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR u.first_name ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR u.last_name ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR u.email ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR u.phone ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR (u.admission_number IS NOT NULL AND u.admission_number ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(") OR EXISTS (SELECT 1 FROM class_sections cs3 JOIN classes c3 ON cs3.class_id = c3.id WHERE (cs3.id = u.class_section_id OR EXISTS (SELECT 1 FROM enrollments e3 WHERE e3.student_id = u.id AND e3.status = 'ACTIVE' AND e3.class_section_id = cs3.id)) AND c3.name ILIKE ");
            qb.push_bind(pat);
            qb.push("))");
        }
    }

    pub async fn list_users(
        pool: &PgPool,
        school_id: Uuid,
        filters: &std::collections::HashMap<String, String>,
    ) -> Result<(Vec<UserListItem>, i64), AppError> {
        let filter_role = filters.get("role").map(|s| s.trim()).unwrap_or("");
        let status = filters.get("status").map(|s| s.trim()).unwrap_or("");
        let admitted_from = filters.get("admitted_from").map(|s| s.trim()).unwrap_or("");
        let admitted_to = filters.get("admitted_to").map(|s| s.trim()).unwrap_or("");
        let admission_number = filters
            .get("admission_number")
            .map(|s| s.trim())
            .unwrap_or("");
        let class_name = filters.get("class_name").map(|s| s.trim()).unwrap_or("");
        let search = filters.get("search").map(|s| s.trim()).unwrap_or("");

        let mut qb = QueryBuilder::<sqlx::Postgres>::new(
            "SELECT
                u.id,
                u.username,
                u.first_name,
                u.last_name,
                COALESCE(
                    NULLIF(u.display_name, ''),
                    NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''),
                    u.username
                ) as display_name,
                u.email,
                u.phone,
                u.status::TEXT as user_status,
                COALESCE(r.roles, ARRAY[]::TEXT[]) as roles,
                u.admission_number,
                st.employee_id,
                u.gender,
                u.blood_group,
                u.date_of_birth::TEXT as date_of_birth,
                c.name as class_name,
                sec.name as section_name,
                e.roll_number,
                TRIM(CONCAT(g.first_name, ' ', g.last_name)) as guardian_name,
                g.phone as guardian_phone,
                sg.relationship as guardian_relationship,
                u.created_at::TEXT as created_at
            FROM users u
            LEFT JOIN enrollments e ON e.student_id = u.id AND e.status = 'ACTIVE'
            LEFT JOIN class_sections cs ON (cs.id = e.class_section_id OR (e.class_section_id IS NULL AND cs.id = u.class_section_id))
            LEFT JOIN classes c ON cs.class_id = c.id
            LEFT JOIN sections sec ON cs.section_id = sec.id
            LEFT JOIN staff st ON st.user_id = u.id
            LEFT JOIN student_guardians sg ON sg.student_id = u.id
            LEFT JOIN guardians g ON sg.guardian_id = g.id
            LEFT JOIN LATERAL (
                SELECT COALESCE(array_agg(DISTINCT roles.code::TEXT) FILTER (WHERE roles.code IS NOT NULL), ARRAY[]::TEXT[]) as roles
                FROM user_roles ur2
                JOIN roles ON roles.id = ur2.role_id
                WHERE ur2.user_id = u.id
            ) r ON TRUE
            WHERE u.school_id = ",
        );
        qb.push_bind(school_id);
        Self::append_user_filter_clauses(
            &mut qb,
            filter_role,
            status,
            admitted_from,
            admitted_to,
            admission_number,
            class_name,
            search,
        );
        qb.push(" ORDER BY u.created_at DESC LIMIT 300");

        let rows = qb.build().fetch_all(pool).await?;

        let total_count = if rows.len() < 300 {
            rows.len() as i64
        } else {
            let mut count_qb = QueryBuilder::<sqlx::Postgres>::new(
                "SELECT COUNT(*) FROM users u WHERE u.school_id = ",
            );
            count_qb.push_bind(school_id);
            Self::append_user_filter_clauses(
                &mut count_qb,
                filter_role,
                status,
                admitted_from,
                admitted_to,
                admission_number,
                class_name,
                search,
            );
            let count_row = count_qb.build().fetch_one(pool).await?;
            count_row.get::<i64, _>(0)
        };

        let list = rows
            .into_iter()
            .map(|r| UserListItem {
                id: r.get("id"),
                username: r.get("username"),
                first_name: r.try_get("first_name").ok(),
                last_name: r.try_get("last_name").ok(),
                display_name: r.get("display_name"),
                email: r.try_get("email").ok(),
                phone: r.try_get("phone").ok(),
                status: r.try_get("user_status").ok(),
                roles: r.get("roles"),
                admission_number: r.try_get("admission_number").ok(),
                employee_id: r.try_get("employee_id").ok(),
                gender: r.try_get("gender").ok(),
                blood_group: r.try_get("blood_group").ok(),
                date_of_birth: r.try_get("date_of_birth").ok(),
                class_name: r.try_get("class_name").ok(),
                section_name: r.try_get("section_name").ok(),
                roll_number: r.try_get("roll_number").ok(),
                guardian_name: r.try_get("guardian_name").ok(),
                guardian_phone: r.try_get("guardian_phone").ok(),
                guardian_relationship: r.try_get("guardian_relationship").ok(),
                created_at: r.try_get("created_at").ok(),
            })
            .collect();

        Ok((list, total_count))
    }

    pub async fn get_existing_emails_and_phones(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<
        (
            std::collections::HashSet<String>,
            std::collections::HashSet<String>,
        ),
        AppError,
    > {
        let emails: std::collections::HashSet<String> = sqlx::query_scalar(
            "SELECT LOWER(email) FROM users WHERE school_id = $1 AND email IS NOT NULL",
        )
        .bind(school_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        let phones: std::collections::HashSet<String> = sqlx::query_scalar(
            "SELECT phone FROM users WHERE school_id = $1 AND phone IS NOT NULL",
        )
        .bind(school_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        Ok((emails, phones))
    }

    pub async fn get_existing_admission_numbers(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<std::collections::HashSet<String>, AppError> {
        let adms: std::collections::HashSet<String> = sqlx::query_scalar(
            "SELECT UPPER(admission_number) FROM users WHERE school_id = $1 AND admission_number IS NOT NULL"
        )
        .bind(school_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        Ok(adms)
    }

    pub async fn check_phone_exists_in_school(
        pool: &PgPool,
        school_id: Uuid,
        phone: &str,
    ) -> Result<bool, AppError> {
        let exists: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE school_id = $1 AND phone = $2)",
        )
        .bind(school_id)
        .bind(phone)
        .fetch_one(pool)
        .await?;

        Ok(exists.unwrap_or(false))
    }

    pub async fn create_staff_user(
        conn: &mut sqlx::PgConnection,
        school_id: Uuid,
        first_name: &str,
        last_name: &str,
        email: &str,
        phone: &str,
        username: &str,
        password_hash: &str,
        emp_id: &str,
        joining_date: time::Date,
        department: Option<String>,
        role: &str,
        creator_user_id: Uuid,
        imported_via_csv: bool,
    ) -> Result<Uuid, AppError> {
        if role.trim().eq_ignore_ascii_case("CLASS_TEACHER") {
            return Err(AppError::BadRequest(
                "CLASS_TEACHER is an internal system role and cannot be assigned manually. It is automatically granted when designating a class teacher for a section.".into(),
            ));
        }

        let user_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status, imported_via_csv, updated_by_user_id, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, 'INACTIVE', $8, $9, NOW())
               RETURNING id"#
        )
        .bind(school_id)
        .bind(first_name)
        .bind(last_name)
        .bind(email)
        .bind(phone)
        .bind(username)
        .bind(password_hash)
        .bind(imported_via_csv)
        .bind(creator_user_id)
        .fetch_one(&mut *conn)
        .await?;

        crate::utils::roles::assign_user_role(&mut *conn, school_id, user_id, role).await?;

        sqlx::query(
            "INSERT INTO staff (school_id, user_id, employee_id, department, joining_date)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(school_id)
        .bind(user_id)
        .bind(emp_id)
        .bind(department)
        .bind(joining_date)
        .execute(&mut *conn)
        .await?;

        Ok(user_id)
    }

    pub async fn create_bulk_student_item(
        conn: &mut sqlx::PgConnection,
        school_id: Uuid,
        user_id: Uuid,
        default_pass: &str,
        first: &str,
        last: &str,
        adm: &str,
        dob: time::Date,
        g_first: &str,
        g_last: &str,
        g_email: &str,
        g_phone: &str,
        s_phone: Option<&str>,
        cls_name: Option<&str>,
        sec_name: Option<&str>,
        active_ay_id: Option<Uuid>,
        pre_resolved_cs_id: Option<Uuid>,
    ) -> Result<(Uuid, Option<Uuid>), AppError> {
        let s_username = format!(
            "{}_{}",
            adm.to_lowercase().replace(' ', "_"),
            school_id.to_string().chars().take(6).collect::<String>()
        );

        let s_user_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO users (school_id, first_name, last_name, username, password_hash, status, imported_via_csv, updated_by_user_id, phone, admission_number, date_of_birth, class_section_id, updated_at)
               VALUES ($1, $2, $3, $4, $5, 'INACTIVE', true, $6, $7, $8, $9, $10, NOW())
               ON CONFLICT (school_id, username) DO UPDATE SET
                   first_name = EXCLUDED.first_name,
                   last_name = EXCLUDED.last_name,
                   phone = COALESCE(EXCLUDED.phone, users.phone),
                   admission_number = COALESCE(EXCLUDED.admission_number, users.admission_number),
                   date_of_birth = COALESCE(EXCLUDED.date_of_birth, users.date_of_birth),
                   class_section_id = COALESCE(EXCLUDED.class_section_id, users.class_section_id),
                   updated_at = NOW(),
                   updated_by_user_id = EXCLUDED.updated_by_user_id
               RETURNING id"#
        )
        .bind(school_id)
        .bind(first)
        .bind(last)
        .bind(&s_username)
        .bind(default_pass)
        .bind(user_id)
        .bind(s_phone)
        .bind(adm)
        .bind(dob)
        .bind(pre_resolved_cs_id)
        .fetch_one(&mut *conn)
        .await?;

        let _ = crate::utils::roles::assign_user_role(&mut *conn, school_id, s_user_id, "STUDENT")
            .await;

        let existing_g_user: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM users WHERE school_id = $1 AND (phone = $2 OR (email IS NOT NULL AND LOWER(email) = LOWER($3))) LIMIT 1"
        )
        .bind(school_id)
        .bind(g_phone)
        .bind(g_email)
        .fetch_optional(&mut *conn)
        .await
        .ok()
        .flatten();

        let g_user_id = match existing_g_user {
            Some(uid) => uid,
            None => {
                let g_username = format!(
                    "g_{}_{}",
                    g_first.to_lowercase().replace(' ', "_"),
                    g_phone
                        .replace('+', "")
                        .chars()
                        .take(10)
                        .collect::<String>()
                );
                let g_uid: Uuid = sqlx::query_scalar(
                    r#"INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status, imported_via_csv, updated_by_user_id, updated_at)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, 'INACTIVE', true, $8, NOW())
                       ON CONFLICT (school_id, username) DO UPDATE SET
                           email = COALESCE(EXCLUDED.email, users.email),
                           phone = COALESCE(EXCLUDED.phone, users.phone),
                           updated_at = NOW(),
                           updated_by_user_id = EXCLUDED.updated_by_user_id
                       RETURNING id"#
                )
                .bind(school_id)
                .bind(g_first)
                .bind(g_last)
                .bind(g_email)
                .bind(g_phone)
                .bind(&g_username)
                .bind(default_pass)
                .bind(user_id)
                .fetch_one(&mut *conn)
                .await?;

                g_uid
            }
        };

        let _ =
            crate::utils::roles::assign_user_role(&mut *conn, school_id, g_user_id, "PARENT").await;

        let existing_g_record: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM guardians WHERE school_id = $1 AND (user_id = $2 OR phone = $3) LIMIT 1"
        )
        .bind(school_id)
        .bind(g_user_id)
        .bind(g_phone)
        .fetch_optional(&mut *conn)
        .await
        .ok()
        .flatten();

        let g_record_id = match existing_g_record {
            Some(gid) => gid,
            None => sqlx::query_scalar(
                r#"INSERT INTO guardians (school_id, user_id, first_name, last_name, email, phone)
                       VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#,
            )
            .bind(school_id)
            .bind(g_user_id)
            .bind(g_first)
            .bind(g_last)
            .bind(g_email)
            .bind(g_phone)
            .fetch_one(&mut *conn)
            .await?,
        };

        sqlx::query(
            "INSERT INTO student_guardians (student_id, guardian_id, relationship) VALUES ($1, $2, 'PARENT') ON CONFLICT DO NOTHING"
        )
        .bind(s_user_id)
        .bind(g_record_id)
        .execute(&mut *conn)
        .await?;

        let mut final_cs_id: Option<Uuid> = pre_resolved_cs_id;

        if final_cs_id.is_none() {
            if let (Some(c_name), Some(s_name), Some(ay_id)) = (cls_name, sec_name, active_ay_id) {
                let c_code = c_name.trim().to_uppercase();
                let s_code = s_name.trim().to_uppercase();

                let mut class_id: Option<Uuid> =
                    sqlx::query_scalar("SELECT id FROM classes WHERE school_id = $1 AND code = $2")
                        .bind(school_id)
                        .bind(&c_code)
                        .fetch_optional(&mut *conn)
                        .await
                        .ok()
                        .flatten();

                if class_id.is_none() {
                    class_id = sqlx::query_scalar(
                        "INSERT INTO classes (school_id, name, code, sequence) VALUES ($1, $2, $3, 1) ON CONFLICT (school_id, code) DO UPDATE SET name = EXCLUDED.name RETURNING id"
                    )
                    .bind(school_id)
                    .bind(c_name.trim())
                    .bind(&c_code)
                    .fetch_one(&mut *conn)
                    .await
                    .ok();
                }

                let mut section_id: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM sections WHERE school_id = $1 AND code = $2",
                )
                .bind(school_id)
                .bind(&s_code)
                .fetch_optional(&mut *conn)
                .await
                .ok()
                .flatten();

                if section_id.is_none() {
                    section_id = sqlx::query_scalar(
                        "INSERT INTO sections (school_id, name, code) VALUES ($1, $2, $3) ON CONFLICT (school_id, code) DO UPDATE SET name = EXCLUDED.name RETURNING id"
                    )
                    .bind(school_id)
                    .bind(s_name.trim())
                    .bind(&s_code)
                    .fetch_one(&mut *conn)
                    .await
                    .ok();
                }

                if let (Some(cid), Some(sid)) = (class_id, section_id) {
                    final_cs_id = sqlx::query_scalar(
                        "SELECT id FROM class_sections WHERE academic_year_id = $1 AND class_id = $2 AND section_id = $3"
                    )
                    .bind(ay_id)
                    .bind(cid)
                    .bind(sid)
                    .fetch_optional(&mut *conn)
                    .await
                    .ok()
                    .flatten();

                    if final_cs_id.is_none() {
                        final_cs_id = sqlx::query_scalar(
                            "INSERT INTO class_sections (academic_year_id, class_id, section_id, status) VALUES ($1, $2, $3, 'ACTIVE') ON CONFLICT (academic_year_id, class_id, section_id) DO UPDATE SET status = 'ACTIVE' RETURNING id"
                        )
                        .bind(ay_id)
                        .bind(cid)
                        .bind(sid)
                        .fetch_one(&mut *conn)
                        .await
                        .ok();
                    }
                }
            }
        }

        if let Some(csid) = final_cs_id {
            let _ = sqlx::query("UPDATE users SET class_section_id = $1 WHERE id = $2")
                .bind(csid)
                .bind(s_user_id)
                .execute(&mut *conn)
                .await;
        }

        if let (Some(ay_id), Some(csid)) = (active_ay_id, final_cs_id) {
            let _ = sqlx::query(
                "INSERT INTO enrollments (student_id, academic_year_id, class_section_id, status) VALUES ($1, $2, $3, 'ACTIVE') ON CONFLICT (student_id, academic_year_id) DO UPDATE SET class_section_id = EXCLUDED.class_section_id"
            )
            .bind(s_user_id)
            .bind(ay_id)
            .bind(csid)
            .execute(&mut *conn)
            .await
            .ok();
        }

        Ok((s_user_id, final_cs_id))
    }
}
