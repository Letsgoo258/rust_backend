use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// Helper function to seed all 29 standard system roles into the `roles` table for a given school.
pub async fn seed_all_school_roles(
    conn: &mut PgConnection,
    school_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO roles (school_id, name, code, description, is_system_role)
        SELECT
            $1,
            r.name,
            r.code,
            r.description,
            true
        FROM (
            VALUES
                ('Super Admin', 'SUPER_ADMIN', 'Full system access across all school settings'),
                ('School Admin', 'SCHOOL_ADMIN', 'School level administrator'),
                ('Teacher', 'TEACHER', 'Academic faculty member'),
                ('Student', 'STUDENT', 'Enrolled student'),
                ('Parent', 'PARENT', 'Guardian or parent of a student'),
                ('Vice Principal', 'VICE_PRINCIPAL', 'Deputy head of school'),
                ('Headmaster', 'HEADMASTER', 'Headmaster or principal'),
                ('Class Teacher', 'CLASS_TEACHER', 'Assigned class teacher'),
                ('Subject Coordinator', 'SUBJECT_COORDINATOR', 'Head of subject department'),
                ('Exam Coordinator', 'EXAM_COORDINATOR', 'Controller of examinations'),
                ('Librarian', 'Librarian', 'Library supervisor'),
                ('Lab Assistant', 'Lab Assistant', 'Laboratory assistant'),
                ('Counselor', 'Counselor', 'Student guidance counselor'),
                ('PT Teacher', 'PT Teacher', 'Physical education teacher'),
                ('Receptionist', 'Receptionist', 'Front desk officer'),
                ('HR Manager', 'HR Manager', 'Human resources manager'),
                ('Accountant', 'Accountant', 'Finance and fee manager'),
                ('Clerk', 'Clerk', 'Administrative clerk'),
                ('Transport Manager', 'Transport Manager', 'School transport supervisor'),
                ('Driver', 'Driver', 'School bus driver'),
                ('Bus Attendant', 'Bus Attendant', 'Bus attendant/conductor'),
                ('Canteen Manager', 'Canteen Manager', 'Cafeteria supervisor'),
                ('Canteen Staff', 'Canteen Staff', 'Cafeteria staff'),
                ('Security Head', 'Security Head', 'Chief security officer'),
                ('Security Guard', 'Security Guard', 'Campus security guard'),
                ('Peon', 'Peon', 'Office assistant/peon'),
                ('Housekeeping', 'Housekeeping', 'Maintenance and cleaning staff'),
                ('Support Staff', 'Support Staff', 'General support staff'),
                ('Alumni', 'Alumni', 'Former student alumni')
        ) AS r(name, code, description)
        ON CONFLICT (school_id, code) DO NOTHING
        "#,
        school_id
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Helper function to retrieve or create a role ID in the `roles` table for a given school.
pub async fn get_or_create_role_id(
    conn: &mut PgConnection,
    school_id: Uuid,
    role_code: &str,
    role_name: &str,
) -> Result<Uuid, sqlx::Error> {
    let code_upper = role_code.trim().to_uppercase();
    let name_str = if role_name.trim().is_empty() {
        &code_upper
    } else {
        role_name.trim()
    };

    let role_id = sqlx::query_scalar!(
        r#"
        INSERT INTO roles (school_id, name, code, is_system_role)
        VALUES ($1, $2, $3, true)
        ON CONFLICT (school_id, code) DO UPDATE SET name = EXCLUDED.name
        RETURNING id
        "#,
        school_id,
        name_str,
        code_upper
    )
    .fetch_one(&mut *conn)
    .await?;

    Ok(role_id)
}

/// Helper function to assign a role to a user in `user_roles` table.
pub async fn assign_user_role(
    conn: &mut PgConnection,
    school_id: Uuid,
    user_id: Uuid,
    role_code: &str,
) -> Result<Uuid, sqlx::Error> {
    let code_upper = role_code.trim().to_uppercase();
    let role_id = get_or_create_role_id(&mut *conn, school_id, &code_upper, &code_upper).await?;

    // Insert into user_roles join table (Granular RBAC)
    sqlx::query!(
        r#"
        INSERT INTO user_roles (user_id, role_id)
        VALUES ($1, $2)
        ON CONFLICT (user_id, role_id) DO NOTHING
        "#,
        user_id,
        role_id
    )
    .execute(&mut *conn)
    .await?;

    crate::modules::auth::backend::invalidate_user_cache(&user_id);

    Ok(role_id)
}

/// Helper function to retrieve all role codes assigned to a user from user_roles & roles tables.
#[allow(dead_code)]
pub async fn get_user_roles(
    pool: &PgPool,
    school_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    let roles = sqlx::query_scalar!(
        r#"
        SELECT DISTINCT r.code
        FROM user_roles ur
        JOIN roles r ON ur.role_id = r.id
        WHERE ur.user_id = $1 AND r.school_id = $2
        "#,
        user_id,
        school_id
    )
    .fetch_all(pool)
    .await?;

    Ok(roles)
}

/// Helper function to remove a role from a user in `user_roles` table.
pub async fn remove_user_role(
    conn: &mut PgConnection,
    school_id: Uuid,
    user_id: Uuid,
    role_code: &str,
) -> Result<(), sqlx::Error> {
    let code_upper = role_code.trim().to_uppercase();
    sqlx::query(
        r#"
        DELETE FROM user_roles
        WHERE user_id = $1
          AND role_id IN (
              SELECT id FROM roles WHERE school_id = $2 AND code = $3
          )
        "#,
    )
    .bind(user_id)
    .bind(school_id)
    .bind(code_upper)
    .execute(&mut *conn)
    .await?;

    crate::modules::auth::backend::invalidate_user_cache(&user_id);

    Ok(())
}

/// Helper function to resolve a staff or user ID to a `users.id`.
pub async fn resolve_staff_to_user_id(
    conn: &mut PgConnection,
    school_id: Uuid,
    staff_or_user_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let user_id_from_staff: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT user_id FROM staff WHERE id = $1 AND school_id = $2
        "#,
    )
    .bind(staff_or_user_id)
    .bind(school_id)
    .fetch_optional(&mut *conn)
    .await?;

    if let Some(uid) = user_id_from_staff {
        return Ok(Some(uid));
    }

    let direct_user_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM users WHERE id = $1 AND school_id = $2
        "#,
    )
    .bind(staff_or_user_id)
    .bind(school_id)
    .fetch_optional(&mut *conn)
    .await?;

    Ok(direct_user_id)
}

/// Automatically synchronizes the internal `CLASS_TEACHER` role for a staff member.
/// - If `is_assigned == true`: assigns `CLASS_TEACHER` role to the user.
/// - If `is_assigned == false`: checks if the user is a class teacher of any other active class section
///   in this school; if not, revokes `CLASS_TEACHER` role.
pub async fn sync_class_teacher_role(
    conn: &mut PgConnection,
    school_id: Uuid,
    staff_or_user_id: Uuid,
    section_id: Uuid,
    is_assigned: bool,
) -> Result<(), sqlx::Error> {
    let user_id = match resolve_staff_to_user_id(&mut *conn, school_id, staff_or_user_id).await? {
        Some(uid) => uid,
        None => return Ok(()),
    };

    if is_assigned {
        let _ = assign_user_role(&mut *conn, school_id, user_id, "CLASS_TEACHER").await?;
    } else {
        let has_other_sections: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM class_sections cs
                JOIN classes c ON cs.class_id = c.id
                WHERE c.school_id = $1
                  AND cs.id != $2
                  AND (cs.status IS NULL OR cs.status = 'ACTIVE')
                  AND (
                      cs.class_teacher_staff_id = $3
                      OR cs.class_teacher_staff_id IN (
                          SELECT id FROM staff WHERE user_id = $3 AND school_id = $1
                      )
                  )
            )
            "#,
        )
        .bind(school_id)
        .bind(section_id)
        .bind(user_id)
        .fetch_one(&mut *conn)
        .await
        .unwrap_or(false);

        if !has_other_sections {
            let _ = remove_user_role(&mut *conn, school_id, user_id, "CLASS_TEACHER").await?;
        }
    }

    Ok(())
}
