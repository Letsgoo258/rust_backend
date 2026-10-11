
    pub async fn lookup_admission_numbers(
        pool: &PgPool,
        school_id: Uuid,
        ay_id: Uuid,
        admission_numbers: &[String],
    ) -> Result<Vec<LookupStudentRow>, AppError> {
        let rows = sqlx::query_as!(
            LookupStudentRow,
            r#"
            SELECT
                s.id,
                u.display_name,
                u.username,
                s.admission_number,
                e.class_section_id as current_class_section_id,
                c.name as current_class_name,
                sec.name as current_section_name
            FROM students s
            JOIN users u ON u.id = s.user_id
            LEFT JOIN enrollments e ON e.student_id = s.id AND e.academic_year_id = $2
            LEFT JOIN class_sections cs ON cs.id = e.class_section_id
            LEFT JOIN classes c ON c.id = cs.class_id
            LEFT JOIN sections sec ON sec.id = cs.section_id
            WHERE s.school_id = $1 AND s.admission_number = ANY($3)
            "#,
            school_id,
            ay_id,
            admission_numbers
        )
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
                "SELECT id FROM students WHERE id = $1 AND school_id = $2",
                student_id,
                school_id
            )
            .fetch_optional(&mut *tx)
            .await?;

            if exists.is_none() { continue; }

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
        }

        tx.commit().await?;
        Ok(())
    }
