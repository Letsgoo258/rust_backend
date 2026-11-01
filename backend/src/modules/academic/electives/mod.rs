use crate::error::AppError;
use crate::modules::academic::repository::AcademicRepository;
use crate::modules::auth::backend::AuthSession;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectiveSubjectBrief {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectiveGroupItem {
    pub id: Uuid,
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub class_id: Option<Uuid>,
    pub class_name: Option<String>,
    pub name: String,
    pub code: Option<String>,
    pub description: Option<String>,
    pub subjects: Vec<ElectiveSubjectBrief>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateElectiveGroupInput {
    pub class_id: Option<Uuid>,
    pub name: String,
    pub code: Option<String>,
    pub description: Option<String>,
    pub subject_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateElectiveGroupInput {
    pub class_id: Option<Uuid>,
    pub name: Option<String>,
    pub code: Option<String>,
    pub description: Option<String>,
    pub subject_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkAssignElectiveInput {
    pub class_section_id: Uuid,
    pub subject_id: Uuid,
    pub elective_group_id: Option<Uuid>,
    pub student_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentElectiveEnrollmentItem {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub admission_number: Option<String>,
    pub roll_number: Option<String>,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub elective_group_id: Option<Uuid>,
    pub class_section_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentElectiveMatrixRow {
    pub student_id: Uuid,
    pub student_name: String,
    pub admission_number: Option<String>,
    pub roll_number: Option<String>,
    pub enrolled_subject_ids: Vec<Uuid>,
}

pub struct ElectiveService;

impl ElectiveService {
    /// List all elective groups for a school & academic year
    pub async fn list_groups(
        pool: &PgPool,
        school_id: Uuid,
        academic_year_id: Uuid,
        class_id: Option<Uuid>,
    ) -> Result<Vec<ElectiveGroupItem>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                eg.id, eg.school_id, eg.academic_year_id, eg.class_id,
                c.name as class_name, eg.name, eg.code, eg.description,
                eg.created_at::text as created_at,
                COALESCE(
                    (SELECT json_agg(json_build_object(
                        'id', s.id,
                        'name', s.name,
                        'code', s.code,
                        'color', s.color
                    )) FROM subjects s WHERE s.elective_group_id = eg.id),
                    '[]'::json
                ) as subjects_json
            FROM elective_groups eg
            LEFT JOIN classes c ON eg.class_id = c.id
            WHERE eg.school_id = $1 AND eg.academic_year_id = $2
              AND ($3::uuid IS NULL OR eg.class_id = $3)
            ORDER BY eg.name ASC
            "#,
        )
        .bind(school_id)
        .bind(academic_year_id)
        .bind(class_id)
        .fetch_all(pool)
        .await?;

        let mut groups = Vec::new();
        for r in rows {
            let subjects_val: serde_json::Value = r.get("subjects_json");
            let subjects: Vec<ElectiveSubjectBrief> =
                serde_json::from_value(subjects_val).unwrap_or_default();

            groups.push(ElectiveGroupItem {
                id: r.get("id"),
                school_id: r.get("school_id"),
                academic_year_id: r.get("academic_year_id"),
                class_id: r.get("class_id"),
                class_name: r.get("class_name"),
                name: r.get("name"),
                code: r.get("code"),
                description: r.get("description"),
                subjects,
                created_at: r.get("created_at"),
            });
        }

        Ok(groups)
    }

    /// Create an elective group and optionally associate subjects
    pub async fn create_group(
        pool: &PgPool,
        school_id: Uuid,
        academic_year_id: Uuid,
        input: CreateElectiveGroupInput,
    ) -> Result<Uuid, AppError> {
        let mut tx = pool.begin().await?;

        let group_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO elective_groups (
                school_id, academic_year_id, class_id, name, code, description
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(school_id)
        .bind(academic_year_id)
        .bind(input.class_id)
        .bind(&input.name)
        .bind(&input.code)
        .bind(&input.description)
        .fetch_one(&mut *tx)
        .await?;

        // Link specified subjects
        if let Some(sub_ids) = input.subject_ids {
            if !sub_ids.is_empty() {
                sqlx::query(
                    r#"
                    UPDATE subjects
                    SET elective_group_id = $1
                    WHERE school_id = $2 AND id = ANY($3)
                    "#,
                )
                .bind(group_id)
                .bind(school_id)
                .bind(&sub_ids)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(group_id)
    }

    /// Update an elective group
    pub async fn update_group(
        pool: &PgPool,
        school_id: Uuid,
        group_id: Uuid,
        input: UpdateElectiveGroupInput,
    ) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;

        sqlx::query(
            r#"
            UPDATE elective_groups
            SET
                class_id = COALESCE($1, class_id),
                name = COALESCE($2, name),
                code = COALESCE($3, code),
                description = COALESCE($4, description),
                updated_at = NOW()
            WHERE id = $5 AND school_id = $6
            "#,
        )
        .bind(input.class_id)
        .bind(&input.name)
        .bind(&input.code)
        .bind(&input.description)
        .bind(group_id)
        .bind(school_id)
        .execute(&mut *tx)
        .await?;

        // If subject_ids provided, re-sync subject links
        if let Some(sub_ids) = input.subject_ids {
            // Detach previous subjects from this group
            sqlx::query(
                "UPDATE subjects SET elective_group_id = NULL WHERE school_id = $1 AND elective_group_id = $2",
            )
            .bind(school_id)
            .bind(group_id)
            .execute(&mut *tx)
            .await?;

            if !sub_ids.is_empty() {
                sqlx::query(
                    r#"
                    UPDATE subjects
                    SET elective_group_id = $1
                    WHERE school_id = $2 AND id = ANY($3)
                    "#,
                )
                .bind(group_id)
                .bind(school_id)
                .bind(&sub_ids)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }

    /// Delete an elective group
    pub async fn delete_group(
        pool: &PgPool,
        school_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), AppError> {
        let res = sqlx::query("DELETE FROM elective_groups WHERE id = $1 AND school_id = $2")
            .bind(group_id)
            .bind(school_id)
            .execute(pool)
            .await?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Elective group not found".into()));
        }

        Ok(())
    }

    /// Get students enrolled in a specific elective subject for a class section
    pub async fn get_subject_enrollments(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Vec<StudentElectiveEnrollmentItem>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                sse.id, sse.student_id,
                COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username) as student_name,
                u.admission_number,
                COALESCE((
                    SELECT e.roll_number FROM enrollments e
                    WHERE e.student_id = u.id AND e.class_section_id = $2
                    ORDER BY e.created_at DESC LIMIT 1
                ), u.admission_number, '') as roll_number,
                sse.subject_id, s.name as subject_name,
                sse.elective_group_id, sse.class_section_id, sse.status
            FROM student_subject_enrollments sse
            JOIN users u ON sse.student_id = u.id
            JOIN subjects s ON sse.subject_id = s.id
            WHERE sse.school_id = $1 AND sse.class_section_id = $2 AND sse.subject_id = $3
              AND sse.status = 'ACTIVE'
            ORDER BY COALESCE(NULLIF(u.admission_number, ''), '999999'), student_name ASC
            "#,
        )
        .bind(school_id)
        .bind(class_section_id)
        .bind(subject_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| StudentElectiveEnrollmentItem {
                id: r.get("id"),
                student_id: r.get("student_id"),
                student_name: r.get("student_name"),
                admission_number: r.get("admission_number"),
                roll_number: r.get("roll_number"),
                subject_id: r.get("subject_id"),
                subject_name: r.get("subject_name"),
                elective_group_id: r.get("elective_group_id"),
                class_section_id: r.get("class_section_id"),
                status: r.get("status"),
            })
            .collect();

        Ok(list)
    }

    /// Bulk assign students to an elective subject for a class section
    pub async fn bulk_assign_students(
        pool: &PgPool,
        school_id: Uuid,
        input: BulkAssignElectiveInput,
    ) -> Result<usize, AppError> {
        let academic_year_id =
            match AcademicRepository::get_active_academic_year_id(pool, school_id).await? {
                Some(id) => id,
                None => {
                    return Err(AppError::BadRequest(
                        "No active academic year found for school".into(),
                    ))
                }
            };

        let mut tx = pool.begin().await?;

        // 1. Mark existing active enrollments for this subject and class section as DROPPED
        // if they are NOT in the incoming student_ids list
        if input.student_ids.is_empty() {
            sqlx::query(
                r#"
                UPDATE student_subject_enrollments
                SET status = 'DROPPED', updated_at = NOW()
                WHERE school_id = $1 AND class_section_id = $2 AND subject_id = $3 AND status = 'ACTIVE'
                "#,
            )
            .bind(school_id)
            .bind(input.class_section_id)
            .bind(input.subject_id)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;
            return Ok(0);
        }

        sqlx::query(
            r#"
            UPDATE student_subject_enrollments
            SET status = 'DROPPED', updated_at = NOW()
            WHERE school_id = $1 AND class_section_id = $2 AND subject_id = $3
              AND status = 'ACTIVE' AND NOT (student_id = ANY($4))
            "#,
        )
        .bind(school_id)
        .bind(input.class_section_id)
        .bind(input.subject_id)
        .bind(&input.student_ids)
        .execute(&mut *tx)
        .await?;

        // 2. Upsert each selected student into student_subject_enrollments
        let mut count = 0;
        for student_id in &input.student_ids {
            sqlx::query(
                r#"
                INSERT INTO student_subject_enrollments (
                    school_id, academic_year_id, student_id, subject_id,
                    class_section_id, elective_group_id, status, enrolled_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, 'ACTIVE', NOW())
                ON CONFLICT (student_id, subject_id, academic_year_id) DO UPDATE SET
                    class_section_id = EXCLUDED.class_section_id,
                    elective_group_id = COALESCE(EXCLUDED.elective_group_id, student_subject_enrollments.elective_group_id),
                    status = 'ACTIVE',
                    updated_at = NOW()
                "#,
            )
            .bind(school_id)
            .bind(academic_year_id)
            .bind(student_id)
            .bind(input.subject_id)
            .bind(input.class_section_id)
            .bind(input.elective_group_id)
            .execute(&mut *tx)
            .await?;

            count += 1;
        }

        tx.commit().await?;
        Ok(count)
    }

    /// Retrieve the student elective matrix for a class section
    pub async fn get_class_section_matrix(
        pool: &PgPool,
        school_id: Uuid,
        class_section_id: Uuid,
    ) -> Result<Vec<StudentElectiveMatrixRow>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                u.id as student_id,
                COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username) as student_name,
                u.admission_number,
                COALESCE((
                    SELECT e.roll_number FROM enrollments e
                    WHERE e.student_id = u.id AND (e.class_section_id = $2 OR e.class_section_id = u.class_section_id)
                    ORDER BY e.created_at DESC LIMIT 1
                ), u.admission_number, '') as roll_number,
                COALESCE(
                    (SELECT array_agg(sse.subject_id)
                     FROM student_subject_enrollments sse
                     WHERE sse.student_id = u.id AND sse.class_section_id = $2 AND sse.status = 'ACTIVE'),
                    '{}'::uuid[]
                ) as enrolled_subject_ids
            FROM users u
            WHERE (u.class_section_id = $2 OR EXISTS (
                SELECT 1 FROM enrollments e WHERE e.student_id = u.id AND e.class_section_id = $2
            ))
              AND u.school_id = $1
              AND u.status = 'ACTIVE'
            ORDER BY COALESCE(NULLIF(u.admission_number, ''), '999999'), student_name ASC
            "#,
        )
        .bind(school_id)
        .bind(class_section_id)
        .fetch_all(pool)
        .await?;

        let list = rows
            .into_iter()
            .map(|r| StudentElectiveMatrixRow {
                student_id: r.get("student_id"),
                student_name: r.get("student_name"),
                admission_number: r.get("admission_number"),
                roll_number: r.get("roll_number"),
                enrolled_subject_ids: r.try_get("enrolled_subject_ids").unwrap_or_default(),
            })
            .collect();

        Ok(list)
    }
}

pub struct ElectiveHandler;

#[derive(Debug, Deserialize)]
pub struct ListGroupsQuery {
    pub class_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct EnrollmentsQuery {
    pub class_section_id: Uuid,
    pub subject_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct MatrixQuery {
    pub class_section_id: Uuid,
}

impl ElectiveHandler {
    pub async fn list_groups(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<ListGroupsQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let academic_year_id =
            match AcademicRepository::get_active_academic_year_id(&state.db, user.school_id).await?
            {
                Some(id) => id,
                None => return Err(AppError::BadRequest("No active academic year found".into())),
            };

        let groups =
            ElectiveService::list_groups(&state.db, user.school_id, academic_year_id, q.class_id)
                .await?;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "groups": groups })),
        ))
    }

    pub async fn create_group(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(input): Json<CreateElectiveGroupInput>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let academic_year_id =
            match AcademicRepository::get_active_academic_year_id(&state.db, user.school_id).await?
            {
                Some(id) => id,
                None => return Err(AppError::BadRequest("No active academic year found".into())),
            };

        let id = ElectiveService::create_group(&state.db, user.school_id, academic_year_id, input)
            .await?;
        Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id, "message": "Elective group created successfully" })),
        ))
    }

    pub async fn update_group(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
        Json(input): Json<UpdateElectiveGroupInput>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        ElectiveService::update_group(&state.db, user.school_id, id, input).await?;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "message": "Elective group updated successfully" })),
        ))
    }

    pub async fn delete_group(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Path(id): Path<Uuid>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        ElectiveService::delete_group(&state.db, user.school_id, id).await?;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "message": "Elective group deleted successfully" })),
        ))
    }

    pub async fn get_enrollments(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<EnrollmentsQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let enrollments = ElectiveService::get_subject_enrollments(
            &state.db,
            user.school_id,
            q.class_section_id,
            q.subject_id,
        )
        .await?;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "enrollments": enrollments, "count": enrollments.len() })),
        ))
    }

    pub async fn bulk_assign(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Json(input): Json<BulkAssignElectiveInput>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let count = ElectiveService::bulk_assign_students(&state.db, user.school_id, input).await?;
        Ok((
            StatusCode::OK,
            Json(
                serde_json::json!({ "assigned_count": count, "message": format!("Successfully updated {} student enrollments", count) }),
            ),
        ))
    }

    pub async fn get_matrix(
        auth_session: AuthSession,
        State(state): State<AppState>,
        Query(q): Query<MatrixQuery>,
    ) -> Result<impl IntoResponse, AppError> {
        let user = auth_session
            .user
            .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

        let matrix = ElectiveService::get_class_section_matrix(
            &state.db,
            user.school_id,
            q.class_section_id,
        )
        .await?;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "matrix": matrix, "count": matrix.len() })),
        ))
    }
}
