use crate::error::AppError;
use dashmap::DashMap;
use sqlx::{PgPool, QueryBuilder, Row};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone)]
struct CachedUsersList {
    users: Vec<UserListItem>,
    total_count: i64,
    cached_at: Instant,
}

static USERS_LIST_CACHE: LazyLock<DashMap<String, CachedUsersList>> = LazyLock::new(DashMap::new);

#[derive(Debug, Clone, serde::Serialize)]
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
    pub class_section_id: Option<Uuid>,
    pub roll_number: Option<String>,
    pub caste_category: Option<String>,
    pub custom_fields: Option<serde_json::Value>,
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
        student_ids: &str,
        class_section_id: Option<Uuid>,
        requester_teacher_user_id: Option<Uuid>,
    ) {
        // Teacher / Class Teacher Scope: If requester is a non-admin teacher, restrict student queries
        // to students in sections they teach or where they are the assigned class teacher.
        if let Some(teacher_uid) = requester_teacher_user_id {
            qb.push(
                " AND (
                    -- If user is NOT a student, allow viewing (e.g. colleagues/staff/admins)
                    (NOT (EXISTS (SELECT 1 FROM user_roles ur_chk JOIN roles r_chk ON ur_chk.role_id = r_chk.id WHERE ur_chk.user_id = u.id AND r_chk.code = 'STUDENT') OR u.admission_number IS NOT NULL))
                    OR
                    -- If user IS a student, they must belong to a section taught or managed by this teacher
                    EXISTS (
                        SELECT 1 FROM class_sections cs_scope
                        WHERE (cs_scope.id = u.class_section_id OR EXISTS (
                            SELECT 1 FROM enrollments e_scope 
                            WHERE e_scope.student_id = u.id AND e_scope.status = 'ACTIVE' AND e_scope.class_section_id = cs_scope.id
                        ))
                        AND (
                            -- Assigned as Class Teacher (by staff.id or direct user_id)
                            cs_scope.class_teacher_staff_id = (SELECT st_scope.id FROM staff st_scope WHERE st_scope.user_id = "
            );
            qb.push_bind(teacher_uid);
            qb.push(
                " LIMIT 1)
                            OR cs_scope.class_teacher_staff_id = ",
            );
            qb.push_bind(teacher_uid);
            qb.push(
                "
                            OR cs_scope.id IN (
                                SELECT ts_scope.class_section_id 
                                FROM timetable_slots ts_scope 
                                WHERE (
                                    ts_scope.teacher_staff_id = (SELECT st_scope2.id FROM staff st_scope2 WHERE st_scope2.user_id = "
            );
            qb.push_bind(teacher_uid);
            qb.push(
                " LIMIT 1)
                                    OR ts_scope.teacher_staff_id = ",
            );
            qb.push_bind(teacher_uid);
            qb.push(
                "
                                    OR ts_scope.co_teacher_staff_ids @> ARRAY[(SELECT st_scope3.id FROM staff st_scope3 WHERE st_scope3.user_id = "
            );
            qb.push_bind(teacher_uid);
            qb.push(
                " LIMIT 1)]
                                )
                                AND ts_scope.status = 'ACTIVE'
                            )
                        )
                    )
                )",
            );
        }

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

        if let Some(sec_uuid) = class_section_id {
            qb.push(" AND (u.class_section_id = ");
            qb.push_bind(sec_uuid);
            qb.push(" OR EXISTS (SELECT 1 FROM enrollments e_cs WHERE e_cs.student_id = u.id AND e_cs.status = 'ACTIVE' AND e_cs.class_section_id = ");
            qb.push_bind(sec_uuid);
            qb.push("))");
        }

        if !search.is_empty() {
            let clean_search = search.trim();
            let pat = format!("%{}%", clean_search);
            let prefix_pat = format!("{}%", clean_search);
            let raw_alphanumeric: String = clean_search
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect();
            let alpha_pat =
                if !raw_alphanumeric.is_empty() && raw_alphanumeric.len() != clean_search.len() {
                    Some(format!("%{}%", raw_alphanumeric))
                } else {
                    None
                };

            qb.push(" AND (");

            // --- LEVEL 1: EXACT MATCH & DIRECT PREFIX ---
            qb.push("(u.first_name ILIKE ");
            qb.push_bind(prefix_pat.clone());
            qb.push(" OR u.last_name ILIKE ");
            qb.push_bind(prefix_pat.clone());
            qb.push(" OR (u.admission_number IS NOT NULL AND u.admission_number ILIKE ");
            qb.push_bind(prefix_pat.clone());
            qb.push(") OR u.username ILIKE ");
            qb.push_bind(prefix_pat.clone());
            qb.push(" OR u.display_name ILIKE ");
            qb.push_bind(prefix_pat.clone());
            qb.push(")");

            // --- LEVEL 2: SUBSTRING & WORD BOUNDARY (\"SOMEWHERE IN BETWEEN\") ---
            qb.push(" OR (u.first_name ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR u.last_name ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR u.display_name ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR (u.first_name || ' ' || u.last_name) ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR u.username ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR u.email ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR u.phone ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(" OR (u.admission_number IS NOT NULL AND u.admission_number ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(")");

            // Alphanumeric sanitized match (ignores hyphens and spaces in admission # or phone)
            if let Some(ref a_pat) = alpha_pat {
                qb.push(" OR REGEXP_REPLACE(COALESCE(u.admission_number, ''), '[^a-zA-Z0-9]', '', 'g') ILIKE ");
                qb.push_bind(a_pat.clone());
                qb.push(" OR REGEXP_REPLACE(COALESCE(u.phone, ''), '[^0-9]', '', 'g') ILIKE ");
                qb.push_bind(a_pat.clone());
            }

            // Class name match
            qb.push(" OR EXISTS (SELECT 1 FROM class_sections cs3 JOIN classes c3 ON cs3.class_id = c3.id WHERE (cs3.id = u.class_section_id OR EXISTS (SELECT 1 FROM enrollments e3 WHERE e3.student_id = u.id AND e3.status = 'ACTIVE' AND e3.class_section_id = cs3.id)) AND c3.name ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(")");

            // Guardian name match
            qb.push(" OR EXISTS (SELECT 1 FROM student_guardians sg3 JOIN guardians g3 ON sg3.guardian_id = g3.id WHERE sg3.student_id = u.id AND (g3.first_name || ' ' || COALESCE(g3.last_name, '')) ILIKE ");
            qb.push_bind(pat.clone());
            qb.push(")");
            qb.push(")");

            // --- LEVEL 3: TYPO-TOLERANT, TRANSLITERATION & TRIGRAM SIMILARITY ---
            if clean_search.len() >= 3 {
                // Aspirated consonant phonetic normalization: dh<->d, sh<->s, th<->t, kh<->k, gh<->g, ph<->f
                qb.push(" OR (");
                qb.push("REGEXP_REPLACE(LOWER(COALESCE(u.first_name, '')), '([dtkspbg])h', '\\1', 'g') ILIKE '%' || REGEXP_REPLACE(LOWER(");
                qb.push_bind(clean_search.to_string());
                qb.push("), '([dtkspbg])h', '\\1', 'g') || '%'");
                qb.push(" OR REGEXP_REPLACE(LOWER(COALESCE(u.last_name, '')), '([dtkspbg])h', '\\1', 'g') ILIKE '%' || REGEXP_REPLACE(LOWER(");
                qb.push_bind(clean_search.to_string());
                qb.push("), '([dtkspbg])h', '\\1', 'g') || '%'");

                // Trigram similarity for typo tolerance
                qb.push(" OR similarity(COALESCE(u.first_name, ''), ");
                qb.push_bind(clean_search.to_string());
                qb.push(") >= 0.25");
                qb.push(" OR similarity(COALESCE(u.last_name, ''), ");
                qb.push_bind(clean_search.to_string());
                qb.push(") >= 0.25");
                qb.push(" OR word_similarity(");
                qb.push_bind(clean_search.to_string());
                qb.push(
                    ", COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')) >= 0.55",
                );
                qb.push(")");
            }

            qb.push(")");
        }

        if !student_ids.is_empty() {
            let uids: Vec<Uuid> = student_ids
                .split(',')
                .filter_map(|s| Uuid::parse_str(s.trim()).ok())
                .collect();
            if !uids.is_empty() {
                qb.push(" AND u.id = ANY(");
                qb.push_bind(uids);
                qb.push(")");
            }
        }
    }

    pub async fn list_users(
        pool: &PgPool,
        school_id: Uuid,
        filters: &std::collections::HashMap<String, String>,
        requester_teacher_user_id: Option<Uuid>,
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
        let student_ids = filters
            .get("student_ids")
            .or_else(|| filters.get("user_ids"))
            .map(|s| s.trim())
            .unwrap_or("");
        let class_section_id = filters
            .get("class_section_id")
            .and_then(|s| Uuid::parse_str(s.trim()).ok());
        let limit: i64 = filters
            .get("limit")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(300);

        // Fast In-Memory Cache Lookup (5-Second TTL)
        let cache_key = format!(
            "{}:{}:{:?}:{}",
            school_id,
            limit,
            filters,
            requester_teacher_user_id
                .map(|u| u.to_string())
                .unwrap_or_default()
        );

        if let Some(entry) = USERS_LIST_CACHE.get(&cache_key) {
            if entry.cached_at.elapsed() < Duration::from_secs(5) {
                return Ok((entry.users.clone(), entry.total_count));
            }
        }

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
                cs.id as class_section_id,
                e.roll_number,
                u.caste_category,
                COALESCE(u.custom_values, '{}'::jsonb) as custom_fields,
                g.guardian_name,
                g.guardian_phone,
                g.guardian_relationship,
                u.created_at::TEXT as created_at
            FROM users u
            LEFT JOIN enrollments e ON e.student_id = u.id AND e.status = 'ACTIVE'
            LEFT JOIN class_sections cs ON cs.id = COALESCE(e.class_section_id, u.class_section_id)
            LEFT JOIN classes c ON cs.class_id = c.id
            LEFT JOIN sections sec ON cs.section_id = sec.id
            LEFT JOIN staff st ON st.user_id = u.id
            LEFT JOIN LATERAL (
                SELECT
                    TRIM(CONCAT(g2.first_name, ' ', g2.last_name)) as guardian_name,
                    g2.phone as guardian_phone,
                    sg2.relationship as guardian_relationship
                FROM student_guardians sg2
                JOIN guardians g2 ON sg2.guardian_id = g2.id
                WHERE sg2.student_id = u.id
                ORDER BY sg2.is_primary DESC, sg2.is_emergency_contact DESC
                LIMIT 1
            ) g ON true
            LEFT JOIN LATERAL (
                SELECT COALESCE(array_agg(DISTINCT ro.code::TEXT) FILTER (WHERE ro.code IS NOT NULL), ARRAY[]::TEXT[]) AS roles
                FROM user_roles ur
                JOIN roles ro ON ro.id = ur.role_id
                WHERE ur.user_id = u.id
            ) r ON true
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
            student_ids,
            class_section_id,
            requester_teacher_user_id,
        );

        if !search.is_empty() {
            let clean_search = search.trim();
            qb.push(" ORDER BY CASE ");
            // Tier 1a: Exact match on identity, username, or name
            qb.push(" WHEN u.admission_number ILIKE ");
            qb.push_bind(clean_search.to_string());
            qb.push(" OR u.username ILIKE ");
            qb.push_bind(clean_search.to_string());
            qb.push(" OR u.first_name ILIKE ");
            qb.push_bind(clean_search.to_string());
            qb.push(" OR (u.first_name || ' ' || u.last_name) ILIKE ");
            qb.push_bind(clean_search.to_string());
            qb.push(" THEN 1 ");

            // Tier 1b: Prefix match
            qb.push(" WHEN u.first_name ILIKE (");
            qb.push_bind(format!("{}%", clean_search));
            qb.push(") OR u.last_name ILIKE (");
            qb.push_bind(format!("{}%", clean_search));
            qb.push(") OR u.admission_number ILIKE (");
            qb.push_bind(format!("{}%", clean_search));
            qb.push(") THEN 2 ");

            // Tier 2: Substring match (\"somewhere in between\")
            qb.push(" WHEN u.first_name ILIKE (");
            qb.push_bind(format!("%{}%", clean_search));
            qb.push(") OR u.last_name ILIKE (");
            qb.push_bind(format!("%{}%", clean_search));
            qb.push(") OR (u.first_name || ' ' || u.last_name) ILIKE (");
            qb.push_bind(format!("%{}%", clean_search));
            qb.push(") THEN 3 ");

            // Tier 3: Fuzzy / Typo / Transliteration
            qb.push(" ELSE 4 END ASC, ");
            qb.push(" GREATEST(similarity(COALESCE(u.first_name, ''), ");
            qb.push_bind(clean_search.to_string());
            qb.push("), word_similarity(");
            qb.push_bind(clean_search.to_string());
            qb.push(", COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, ''))) DESC, ");
            qb.push(" u.created_at DESC");
        } else if class_section_id.is_some() {
            qb.push(" ORDER BY COALESCE(NULLIF(e.roll_number, ''), NULLIF(u.admission_number, ''), '999999'), u.first_name, u.last_name");
        } else {
            qb.push(" ORDER BY u.created_at DESC");
        }

        if limit > 0 {
            qb.push(" LIMIT ");
            qb.push_bind(limit);
        }

        let rows = qb.build().fetch_all(pool).await?;

        let total_count = if limit > 0 && rows.len() < limit as usize {
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
                student_ids,
                class_section_id,
                requester_teacher_user_id,
            );
            let count_row = count_qb.build().fetch_one(pool).await?;
            count_row.get::<i64, _>(0)
        };

        let list: Vec<UserListItem> = rows
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
                class_section_id: r.try_get("class_section_id").ok(),
                roll_number: r.try_get("roll_number").ok(),
                caste_category: r.try_get("caste_category").ok(),
                custom_fields: r.try_get("custom_fields").ok(),
                guardian_name: r.try_get("guardian_name").ok(),
                guardian_phone: r.try_get("guardian_phone").ok(),
                guardian_relationship: r.try_get("guardian_relationship").ok(),
                created_at: r.try_get("created_at").ok(),
            })
            .collect();

        // Populate Cache
        if USERS_LIST_CACHE.len() > 500 {
            USERS_LIST_CACHE.retain(|_, v| v.cached_at.elapsed() < Duration::from_secs(30));
        }
        USERS_LIST_CACHE.insert(
            cache_key,
            CachedUsersList {
                users: list.clone(),
                total_count,
                cached_at: Instant::now(),
            },
        );

        Ok((list, total_count))
    }

    /// Invalidate users list cache for a specific school or globally
    pub fn invalidate_users_cache(school_id: Option<Uuid>) {
        if let Some(sid) = school_id {
            let prefix = format!("{sid}:");
            USERS_LIST_CACHE.retain(|k, _| !k.starts_with(&prefix));
        } else {
            USERS_LIST_CACHE.clear();
        }
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
        roll_number: Option<&str>,
    ) -> Result<(Uuid, Option<Uuid>), AppError> {
        let s_username = format!(
            "{}_{}",
            adm.to_lowercase().replace(' ', "_"),
            school_id.to_string().chars().take(6).collect::<String>()
        );

        let s_user_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO users (school_id, first_name, last_name, username, password_hash, status, imported_via_csv, updated_by_user_id, phone, admission_number, date_of_birth, class_section_id, updated_at)
               VALUES ($1, $2, $3, $4, $5, 'ACTIVE', true, $6, $7, $8, $9, $10, NOW())
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
                       VALUES ($1, $2, $3, $4, $5, $6, $7, 'ACTIVE', true, $8, NOW())
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

        if let Some(ay_id) = active_ay_id {
            let _ = sqlx::query(
                r#"INSERT INTO enrollments (student_id, academic_year_id, class_section_id, roll_number, status, enrollment_date, created_at, updated_at)
                   VALUES ($1, $2, $3, $4, 'ACTIVE', CURRENT_DATE, NOW(), NOW())
                   ON CONFLICT (student_id, academic_year_id) DO UPDATE SET
                       class_section_id = COALESCE(EXCLUDED.class_section_id, enrollments.class_section_id),
                       roll_number = COALESCE(EXCLUDED.roll_number, enrollments.roll_number),
                       status = 'ACTIVE',
                       updated_at = NOW()"#
            )
            .bind(s_user_id)
            .bind(ay_id)
            .bind(final_cs_id)
            .bind(roll_number)
            .execute(&mut *conn)
            .await
            .ok();
        }

        Ok((s_user_id, final_cs_id))
    }
}
