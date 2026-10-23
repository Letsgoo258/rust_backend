use crate::error::AppError;
use sqlx::{PgPool, Postgres, Row, Transaction};
use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, serde::Serialize)]
pub struct ClassCapacityRow {
    pub class_section_id: Uuid,
    pub class_id: Uuid,
    pub class_name: String,
    pub class_code: String,
    pub section_id: Uuid,
    pub section_name: String,
    pub section_code: String,
    pub capacity: Option<i32>,
    pub enrolled_count: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct SchoolStatsRow {
    pub total_students: i64,
    pub active_classes: i64,
    pub pending_enrollments: i64,
    pub total_staff: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct SubjectRow {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub subject_type: Option<String>,
    pub system_subject_type: String,
    pub subject_type_id: Option<Uuid>,
    pub color: Option<String>,
    pub department: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct SubjectTypeRow {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub system_subject_type: String,
    pub is_elective: bool,
    pub is_mandatory: bool,
    pub description: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ClassSectionHistoryRow {
    pub id: Uuid,
    pub change_type: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_by_id: Option<Uuid>,
    pub updated_by_name: Option<String>,
}

#[allow(dead_code)]
pub struct ClassSectionDetail {
    pub capacity: Option<i32>,
    pub room_id: Option<Uuid>,
    pub class_teacher_staff_id: Option<Uuid>,
    pub status: Option<String>,
    pub school_id: Uuid,
}

pub struct AcademicRepository;

#[derive(Debug, serde::Serialize, sqlx::FromRow, Clone)]
pub struct LookupStudentRow {
    pub id: Uuid,
    pub display_name: Option<String>,
    pub username: String,
    pub admission_number: String,
    pub current_class_section_id: Option<Uuid>,
    pub current_class_name: Option<String>,
    pub current_section_name: Option<String>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow, Clone)]
pub struct SectionStudentRow {
    pub id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub admission_number: Option<String>,
    pub roll_number: Option<String>,
    pub status: Option<String>,
    pub gender: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

impl AcademicRepository {
    #[allow(dead_code)]
    pub async fn upsert_class_conn(
        conn: &mut sqlx::PgConnection,
        school_id: Uuid,
        name: &str,
        code: &str,
        sequence: i32,
    ) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO classes (school_id, name, code, sequence)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (school_id, code) DO UPDATE SET name = EXCLUDED.name, sequence = EXCLUDED.sequence
            RETURNING id
            "#
        )
        .bind(school_id)
        .bind(name)
        .bind(code)
        .bind(sequence)
        .fetch_one(&mut *conn)
        .await?;

        Ok(id)
    }

    #[allow(dead_code)]
    pub async fn upsert_section_conn(
        conn: &mut sqlx::PgConnection,
        school_id: Uuid,
        name: &str,
        code: &str,
    ) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO sections (school_id, name, code)
            VALUES ($1, $2, $3)
            ON CONFLICT (school_id, code) DO UPDATE SET name = EXCLUDED.name
            RETURNING id
            "#,
        )
        .bind(school_id)
        .bind(name)
        .bind(code)
        .fetch_one(&mut *conn)
        .await?;

        Ok(id)
    }

    #[allow(dead_code)]
    pub async fn link_class_section_conn(
        conn: &mut sqlx::PgConnection,
        academic_year_id: Uuid,
        class_id: Uuid,
        section_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO class_sections (academic_year_id, class_id, section_id, status)
            VALUES ($1, $2, $3, 'ACTIVE')
            ON CONFLICT (academic_year_id, class_id, section_id) DO NOTHING
            "#,
        )
        .bind(academic_year_id)
        .bind(class_id)
        .bind(section_id)
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn upsert_subject_conn(
        conn: &mut sqlx::PgConnection,
        school_id: Uuid,
        name: &str,
        code: &str,
        subject_type: &str,
    ) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO subjects (school_id, name, code, subject_type)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (school_id, code) DO UPDATE SET name = EXCLUDED.name, subject_type = EXCLUDED.subject_type
            RETURNING id
            "#
        )
        .bind(school_id)
        .bind(name)
        .bind(code)
        .bind(subject_type)
        .fetch_one(&mut *conn)
        .await?;

        Ok(id)
    }

    pub async fn batch_upsert_classes(
        pool: &PgPool,
        school_id: Uuid,
        active_ay_id: Option<Uuid>,
        classes: Vec<crate::modules::academic::handler::ClassInput>,
    ) -> Result<usize, AppError> {
        let mut count = 0;
        for (idx, cls) in classes.into_iter().enumerate() {
            let seq = cls.sequence.unwrap_or((idx + 1) as i32);
            let class_id =
                Self::upsert_class(pool, school_id, cls.name.trim(), cls.code.trim(), seq).await?;

            for sec_name in cls.sections {
                let sec_code = sec_name.trim().to_uppercase();
                let section_id =
                    Self::upsert_section(pool, school_id, sec_name.trim(), &sec_code).await?;

                if let Some(ay_id) = active_ay_id {
                    Self::link_class_section(pool, ay_id, class_id, section_id).await?;
                }
            }
            count += 1;
        }
        Ok(count)
    }

    pub async fn batch_upsert_subjects(
        pool: &PgPool,
        school_id: Uuid,
        subjects: Vec<crate::modules::academic::handler::SubjectInput>,
    ) -> Result<usize, AppError> {
        let mut count = 0;
        for subj in subjects {
            let stype = subj.subject_type.unwrap_or_else(|| "CORE".into());
            let sys_type = subj.system_subject_type.unwrap_or_else(|| "Core".into());
            let color = subj.color.unwrap_or_else(|| "#4F46E5".into());
            Self::upsert_subject(
                pool,
                school_id,
                subj.name.trim(),
                &subj.code.trim().to_uppercase(),
                &stype,
                &sys_type,
                subj.subject_type_id,
                Some(&color),
                subj.department.as_deref(),
                subj.description.as_deref(),
            )
            .await?;
            count += 1;
        }
        Ok(count)
    }

    pub async fn batch_upsert_subject_types(
        pool: &PgPool,
        school_id: Uuid,
        types: Vec<crate::modules::academic::handler::SubjectTypeInput>,
    ) -> Result<usize, AppError> {
        let mut count = 0;
        for t in types {
            let sys_type = t.system_subject_type.unwrap_or_else(|| "Core".into());
            let is_elec = t.is_elective.unwrap_or(false);
            let is_mand = t.is_mandatory.unwrap_or(true);
            Self::upsert_subject_type(
                pool,
                school_id,
                t.name.trim(),
                &t.code.trim().to_uppercase(),
                &sys_type,
                is_elec,
                is_mand,
                t.description.as_deref(),
            )
            .await?;
            count += 1;
        }
        Ok(count)
    }

    pub async fn batch_assign_students(
        pool: &PgPool,
        school_id: Uuid,
        academic_year_id: Uuid,
        class_section_id: Uuid,
        admission_numbers: Vec<String>,
    ) -> Result<(usize, Vec<String>), AppError> {
        let mut count = 0;
        let mut unmatched = Vec::new();

        for raw_adm in admission_numbers {
            let adm = raw_adm.trim();
            if adm.is_empty() {
                continue;
            }

            let student_opt: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM users WHERE school_id = $1 AND UPPER(admission_number) = UPPER($2) LIMIT 1"
            )
            .bind(school_id)
            .bind(adm)
            .fetch_optional(pool)
            .await?;

            if let Some(student_id) = student_opt {
                sqlx::query(
                    r#"
                    INSERT INTO enrollments (student_id, academic_year_id, class_section_id, status)
                    VALUES ($1, $2, $3, 'ACTIVE')
                    ON CONFLICT (student_id, academic_year_id)
                    DO UPDATE SET class_section_id = EXCLUDED.class_section_id, status = 'ACTIVE'
                    "#,
                )
                .bind(student_id)
                .bind(academic_year_id)
                .bind(class_section_id)
                .execute(pool)
                .await?;

                sqlx::query("UPDATE users SET class_section_id = $1 WHERE id = $2")
                    .bind(class_section_id)
                    .bind(student_id)
                    .execute(pool)
                    .await?;

                count += 1;
            } else {
                unmatched.push(adm.to_string());
            }
        }
        Ok((count, unmatched))
    }

    pub async fn lookup_admission_numbers(
        pool: &PgPool,
        school_id: Uuid,
        ay_id: Uuid,
        admission_numbers: &[String],
    ) -> Result<Vec<LookupStudentRow>, AppError> {
        let rows = sqlx::query_as::<_, LookupStudentRow>(
            r#"
            SELECT
                u.id,
                COALESCE(u.display_name, u.first_name || ' ' || u.last_name) as display_name,
                u.username,
                u.admission_number,
                COALESCE(e.class_section_id, u.class_section_id) as current_class_section_id,
                c.name as current_class_name,
                sec.name as current_section_name
            FROM users u
            LEFT JOIN enrollments e ON e.student_id = u.id AND e.academic_year_id = $2
            LEFT JOIN class_sections cs ON (cs.id = e.class_section_id OR (e.class_section_id IS NULL AND cs.id = u.class_section_id))
            LEFT JOIN classes c ON c.id = cs.class_id
            LEFT JOIN sections sec ON sec.id = cs.section_id
            WHERE u.school_id = $1 AND UPPER(u.admission_number) = ANY(SELECT UPPER(x) FROM unnest($3::text[]) x)
            "#,
        )
        .bind(school_id)
        .bind(ay_id)
        .bind(admission_numbers)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    pub async fn assign_students_bulk_by_id(
        pool: &PgPool,
        school_id: Uuid,
        ay_id: Uuid,
        class_section_id: Uuid,
        student_ids: &[Uuid],
    ) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;

        for student_id in student_ids {
            let exists = sqlx::query_scalar!(
                "SELECT id FROM users WHERE id = $1 AND school_id = $2",
                student_id,
                school_id
            )
            .fetch_optional(&mut *tx)
            .await?;

            if exists.is_none() {
                continue;
            }

            sqlx::query!(
                r#"
                INSERT INTO enrollments (student_id, academic_year_id, class_section_id)
                VALUES ($1, $2, $3)
                ON CONFLICT (student_id, academic_year_id)
                DO UPDATE SET
                    class_section_id = EXCLUDED.class_section_id,
                    updated_at = now()
                "#,
                student_id,
                ay_id,
                class_section_id
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query!(
                "UPDATE users SET class_section_id = $1 WHERE id = $2",
                class_section_id,
                student_id
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
    pub async fn create_academic_year(
        pool: &PgPool,
        school_id: Uuid,
        name: &str,
        start_date: Date,
        end_date: Date,
    ) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO academic_years (school_id, name, start_date, end_date, status)
            VALUES ($1, $2, $3, $4, 'ACTIVE')
            ON CONFLICT (school_id, name) DO UPDATE SET start_date = EXCLUDED.start_date, end_date = EXCLUDED.end_date
            RETURNING id
            "#
        )
        .bind(school_id)
        .bind(name)
        .bind(start_date)
        .bind(end_date)
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    pub async fn get_active_academic_year_id(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        let ay_id = sqlx::query_scalar(
            "SELECT id FROM academic_years WHERE school_id = $1 AND status = 'ACTIVE' ORDER BY created_at DESC LIMIT 1"
        )
        .bind(school_id)
        .fetch_optional(pool)
        .await?;

        Ok(ay_id)
    }

    pub async fn upsert_class(
        pool: &PgPool,
        school_id: Uuid,
        name: &str,
        code: &str,
        sequence: i32,
    ) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO classes (school_id, name, code, sequence)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (school_id, code) DO UPDATE SET name = EXCLUDED.name, sequence = EXCLUDED.sequence
            RETURNING id
            "#
        )
        .bind(school_id)
        .bind(name)
        .bind(code)
        .bind(sequence)
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    pub async fn upsert_section(
        pool: &PgPool,
        school_id: Uuid,
        name: &str,
        code: &str,
    ) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO sections (school_id, name, code)
            VALUES ($1, $2, $3)
            ON CONFLICT (school_id, code) DO UPDATE SET name = EXCLUDED.name
            RETURNING id
            "#,
        )
        .bind(school_id)
        .bind(name)
        .bind(code)
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    pub async fn link_class_section(
        pool: &PgPool,
        academic_year_id: Uuid,
        class_id: Uuid,
        section_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO class_sections (academic_year_id, class_id, section_id, status)
            VALUES ($1, $2, $3, 'ACTIVE')
            ON CONFLICT (academic_year_id, class_id, section_id) DO NOTHING
            "#,
        )
        .bind(academic_year_id)
        .bind(class_id)
        .bind(section_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn upsert_subject(
        pool: &PgPool,
        school_id: Uuid,
        name: &str,
        code: &str,
        subject_type: &str,
        system_subject_type: &str,
        subject_type_id: Option<Uuid>,
        color: Option<&str>,
        department: Option<&str>,
        description: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO subjects (school_id, name, code, subject_type, system_subject_type, subject_type_id, color, department, description, status)
            VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, '#4F46E5'), $8, $9, 'ACTIVE')
            ON CONFLICT (school_id, code) DO UPDATE SET
                name = EXCLUDED.name,
                subject_type = EXCLUDED.subject_type,
                system_subject_type = EXCLUDED.system_subject_type,
                subject_type_id = EXCLUDED.subject_type_id,
                color = COALESCE(EXCLUDED.color, subjects.color, '#4F46E5'),
                department = COALESCE(EXCLUDED.department, subjects.department),
                description = COALESCE(EXCLUDED.description, subjects.description),
                status = 'ACTIVE'
            "#
        )
        .bind(school_id)
        .bind(name)
        .bind(code)
        .bind(subject_type)
        .bind(system_subject_type)
        .bind(subject_type_id)
        .bind(color)
        .bind(department)
        .bind(description)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn upsert_subject_type(
        pool: &PgPool,
        school_id: Uuid,
        name: &str,
        code: &str,
        system_subject_type: &str,
        is_elective: bool,
        is_mandatory: bool,
        description: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO subject_types (school_id, name, code, system_subject_type, is_elective, is_mandatory, description)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (school_id, code) DO UPDATE SET
                name = EXCLUDED.name,
                system_subject_type = EXCLUDED.system_subject_type,
                is_elective = EXCLUDED.is_elective,
                is_mandatory = EXCLUDED.is_mandatory,
                description = EXCLUDED.description
            "#
        )
        .bind(school_id)
        .bind(name)
        .bind(code)
        .bind(system_subject_type)
        .bind(is_elective)
        .bind(is_mandatory)
        .bind(description)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn get_class_capacities(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<Vec<ClassCapacityRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                cs.id as class_section_id,
                c.id as class_id,
                c.name as class_name,
                c.code as class_code,
                s.id as section_id,
                s.name as section_name,
                s.code as section_code,
                cs.capacity,
                COUNT(DISTINCT e.student_id)::BIGINT as enrolled_count
            FROM class_sections cs
            JOIN classes c ON cs.class_id = c.id
            JOIN sections s ON cs.section_id = s.id
            JOIN academic_years ay ON cs.academic_year_id = ay.id
            LEFT JOIN enrollments e ON e.class_section_id = cs.id AND e.status = 'ACTIVE'
            WHERE c.school_id = $1 AND ay.status = 'ACTIVE'
            GROUP BY cs.id, c.id, c.name, c.code, s.id, s.name, s.code, cs.capacity
            ORDER BY c.sequence, c.name, s.name
            "#,
        )
        .bind(school_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| ClassCapacityRow {
                class_section_id: r.get("class_section_id"),
                class_id: r.get("class_id"),
                class_name: r.get("class_name"),
                class_code: r.get("class_code"),
                section_id: r.get("section_id"),
                section_name: r.get("section_name"),
                section_code: r.get("section_code"),
                capacity: r.get("capacity"),
                enrolled_count: r.get("enrolled_count"),
            })
            .collect();

        Ok(list)
    }

    pub async fn get_school_stats(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<SchoolStatsRow, AppError> {
        let total_students: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE school_id = $1 AND admission_number IS NOT NULL",
        )
        .bind(school_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let active_classes: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM class_sections cs JOIN classes c ON cs.class_id = c.id WHERE c.school_id = $1"
        )
        .bind(school_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let pending_enrollments: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM users u
            WHERE u.school_id = $1 AND u.admission_number IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM enrollments e WHERE e.student_id = u.id AND e.status = 'ACTIVE'
            )
            "#,
        )
        .bind(school_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let total_staff: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE school_id = $1 AND ('TEACHER' = ANY(roles) OR 'SCHOOL_ADMIN' = ANY(roles) OR 'STAFF' = ANY(roles))"
        )
        .bind(school_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        Ok(SchoolStatsRow {
            total_students,
            active_classes,
            pending_enrollments,
            total_staff,
        })
    }

    pub async fn get_subjects(pool: &PgPool, school_id: Uuid) -> Result<Vec<SubjectRow>, AppError> {
        let rows = sqlx::query(
            "SELECT id, name, code, subject_type, COALESCE(system_subject_type, 'Core') as system_subject_type, subject_type_id, color, department, description FROM subjects WHERE school_id = $1 AND status != 'DELETED' ORDER BY name ASC"
        )
        .bind(school_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| SubjectRow {
                id: r.get("id"),
                name: r.get("name"),
                code: r.get("code"),
                subject_type: r.get("subject_type"),
                system_subject_type: r.get("system_subject_type"),
                subject_type_id: r.get("subject_type_id"),
                color: r.get("color"),
                department: r.get("department"),
                description: r.get("description"),
            })
            .collect();

        Ok(list)
    }

    pub async fn update_subject(
        pool: &PgPool,
        school_id: Uuid,
        subject_id: Uuid,
        name: &str,
        code: &str,
        subject_type: Option<&str>,
        system_subject_type: &str,
        color: Option<&str>,
        department: Option<&str>,
        description: Option<&str>,
    ) -> Result<bool, AppError> {
        let res = sqlx::query(
            r#"
            UPDATE subjects
            SET name = $1,
                code = $2,
                subject_type = COALESCE($3, subject_type),
                system_subject_type = $4,
                color = COALESCE($5, color, '#4F46E5'),
                department = $6,
                description = $7,
                updated_at = NOW()
            WHERE id = $8 AND school_id = $9
            "#,
        )
        .bind(name)
        .bind(code)
        .bind(subject_type)
        .bind(system_subject_type)
        .bind(color)
        .bind(department)
        .bind(description)
        .bind(subject_id)
        .bind(school_id)
        .execute(pool)
        .await?;

        Ok(res.rows_affected() > 0)
    }

    pub async fn delete_subject(
        pool: &PgPool,
        school_id: Uuid,
        subject_id: Uuid,
    ) -> Result<bool, AppError> {
        let res = sqlx::query("DELETE FROM subjects WHERE id = $1 AND school_id = $2")
            .bind(subject_id)
            .bind(school_id)
            .execute(pool)
            .await?;

        Ok(res.rows_affected() > 0)
    }

    pub async fn delete_subject_type(
        pool: &PgPool,
        school_id: Uuid,
        type_id: Uuid,
    ) -> Result<bool, AppError> {
        let res = sqlx::query("DELETE FROM subject_types WHERE id = $1 AND school_id = $2")
            .bind(type_id)
            .bind(school_id)
            .execute(pool)
            .await?;

        Ok(res.rows_affected() > 0)
    }

    pub async fn get_subject_types(
        pool: &PgPool,
        school_id: Uuid,
    ) -> Result<Vec<SubjectTypeRow>, AppError> {
        let rows = sqlx::query(
            "SELECT id, name, code, system_subject_type, is_elective, is_mandatory, description FROM subject_types WHERE school_id = $1 ORDER BY name ASC"
        )
        .bind(school_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| SubjectTypeRow {
                id: r.get("id"),
                name: r.get("name"),
                code: r.get("code"),
                system_subject_type: r.get("system_subject_type"),
                is_elective: r.get("is_elective"),
                is_mandatory: r.get("is_mandatory"),
                description: r.get("description"),
            })
            .collect();

        Ok(list)
    }

    pub async fn get_or_create_class_section(
        pool: &PgPool,
        academic_year_id: Uuid,
        class_id: Uuid,
        section_id: Uuid,
    ) -> Result<Uuid, AppError> {
        let cs_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM class_sections WHERE academic_year_id = $1 AND class_id = $2 AND section_id = $3 LIMIT 1"
        )
        .bind(academic_year_id)
        .bind(class_id)
        .bind(section_id)
        .fetch_optional(pool)
        .await?;

        match cs_id {
            Some(id) => Ok(id),
            None => {
                let new_id: Uuid = sqlx::query_scalar(
                    "INSERT INTO class_sections (academic_year_id, class_id, section_id, status) VALUES ($1, $2, $3, 'ACTIVE') RETURNING id"
                )
                .bind(academic_year_id)
                .bind(class_id)
                .bind(section_id)
                .fetch_one(pool)
                .await?;
                Ok(new_id)
            }
        }
    }

    #[allow(dead_code)]
    pub async fn find_student_id_by_admission_number(
        pool: &PgPool,
        school_id: Uuid,
        admission_number: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM users WHERE school_id = $1 AND UPPER(admission_number) = UPPER($2) LIMIT 1"
        )
        .bind(school_id)
        .bind(admission_number)
        .fetch_optional(pool)
        .await?;

        Ok(id)
    }

    #[allow(dead_code)]
    pub async fn enroll_student(
        pool: &PgPool,
        student_id: Uuid,
        academic_year_id: Uuid,
        class_section_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO enrollments (student_id, academic_year_id, class_section_id, status)
            VALUES ($1, $2, $3, 'ACTIVE')
            ON CONFLICT (student_id, academic_year_id)
            DO UPDATE SET class_section_id = EXCLUDED.class_section_id, status = 'ACTIVE'
            "#,
        )
        .bind(student_id)
        .bind(academic_year_id)
        .bind(class_section_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn get_class_section_detail(
        pool: &PgPool,
        class_section_id: Uuid,
        school_id: Uuid,
    ) -> Result<Option<ClassSectionDetail>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT cs.capacity, cs.room_id, cs.class_teacher_staff_id, cs.status, c.school_id
            FROM class_sections cs
            JOIN classes c ON cs.class_id = c.id
            WHERE cs.id = $1 AND c.school_id = $2
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| ClassSectionDetail {
            capacity: r.get("capacity"),
            room_id: r.get("room_id"),
            class_teacher_staff_id: r.get("class_teacher_staff_id"),
            status: r.get("status"),
            school_id: r.get("school_id"),
        }))
    }

    pub async fn update_class_section(
        tx: &mut Transaction<'_, Postgres>,
        class_section_id: Uuid,
        capacity: Option<i32>,
        room_id: Option<Uuid>,
        class_teacher_staff_id: Option<Uuid>,
        status: Option<&str>,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE class_sections
            SET capacity = COALESCE($2, capacity),
                room_id = COALESCE($3, room_id),
                class_teacher_staff_id = COALESCE($4, class_teacher_staff_id),
                status = COALESCE($5, status),
                updated_by_user_id = $6,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(class_section_id)
        .bind(capacity)
        .bind(room_id)
        .bind(class_teacher_staff_id)
        .bind(status)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    pub async fn record_class_section_history(
        tx: &mut Transaction<'_, Postgres>,
        school_id: Uuid,
        class_section_id: Uuid,
        user_id: Uuid,
        change_type: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO class_section_history (school_id, class_section_id, updated_by_user_id, change_type, old_value, new_value)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(school_id)
        .bind(class_section_id)
        .bind(user_id)
        .bind(change_type)
        .bind(old_value)
        .bind(new_value)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    pub async fn set_class_section_last_history_id(
        tx: &mut Transaction<'_, Postgres>,
        class_section_id: Uuid,
        history_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE class_sections SET last_history_id = $1 WHERE id = $2")
            .bind(history_id)
            .bind(class_section_id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    pub async fn get_class_section_history(
        pool: &PgPool,
        class_section_id: Uuid,
        school_id: Uuid,
    ) -> Result<Vec<ClassSectionHistoryRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                h.id,
                h.change_type,
                h.old_value,
                h.new_value,
                h.created_at,
                u.id as updated_by_id,
                COALESCE(u.display_name, u.username) as updated_by_name
            FROM class_section_history h
            LEFT JOIN users u ON h.updated_by_user_id = u.id
            WHERE h.class_section_id = $1 AND h.school_id = $2
            ORDER BY h.created_at DESC
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| ClassSectionHistoryRow {
                id: r.get("id"),
                change_type: r.get("change_type"),
                old_value: r.get("old_value"),
                new_value: r.get("new_value"),
                created_at: r.get("created_at"),
                updated_by_id: r.get("updated_by_id"),
                updated_by_name: r.get("updated_by_name"),
            })
            .collect();

        Ok(list)
    }

    pub async fn get_class_section_students(
        pool: &PgPool,
        class_section_id: Uuid,
        school_id: Uuid,
    ) -> Result<Vec<SectionStudentRow>, AppError> {
        let rows = sqlx::query_as::<_, SectionStudentRow>(
            r#"
            SELECT
                u.id,
                u.username,
                COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username) as display_name,
                u.admission_number,
                e.roll_number,
                u.status::TEXT as status,
                u.gender,
                u.email,
                u.phone
            FROM enrollments e
            JOIN users u ON u.id = e.student_id
            WHERE e.class_section_id = $1 AND e.status = 'ACTIVE' AND u.school_id = $2
            ORDER BY COALESCE(NULLIF(e.roll_number, ''), '999999'), u.first_name, u.last_name
            "#,
        )
        .bind(class_section_id)
        .bind(school_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
