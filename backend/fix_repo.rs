
    pub async fn batch_upsert_classes(
        conn: &PgPool,
        school_id: Uuid,
        active_ay_id: Option<Uuid>,
        classes: Vec<crate::modules::academic::handler::ClassInput>,
    ) -> Result<usize, AppError> {
        let mut count = 0;
        for (idx, cls) in classes.into_iter().enumerate() {
            let seq = cls.sequence.unwrap_or((idx + 1) as i32);
            let class_id = Self::upsert_class(conn, school_id, cls.name.trim(), cls.code.trim(), seq).await?;

            for sec_name in cls.sections {
                let sec_code = sec_name.trim().to_uppercase();
                let section_id = Self::upsert_section(conn, school_id, sec_name.trim(), &sec_code).await?;

                if let Some(ay_id) = active_ay_id {
                    Self::link_class_section(conn, ay_id, class_id, section_id).await?;
                }
            }
            count += 1;
        }
        Ok(count)
    }

    pub async fn batch_upsert_subjects(
        conn: &PgPool,
        school_id: Uuid,
        subjects: Vec<crate::modules::academic::handler::SubjectInput>,
    ) -> Result<usize, AppError> {
        let mut count = 0;
        for subj in subjects {
            let stype = subj.subject_type.unwrap_or_else(|| "CORE".into());
            Self::upsert_subject(conn, school_id, subj.name.trim(), &subj.code.trim().to_uppercase(), &stype).await?;
            count += 1;
        }
        Ok(count)
    }

    pub async fn batch_assign_students(
        conn: &PgPool,
        school_id: Uuid,
        academic_year_id: Uuid,
        class_section_id: Uuid,
        admission_numbers: Vec<String>,
    ) -> Result<(usize, Vec<String>), AppError> {
        let mut count = 0;
        let mut unmatched = Vec::new();

        for raw_adm in admission_numbers {
            let adm = raw_adm.trim();
            if adm.is_empty() { continue; }

            let student_opt: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM students WHERE school_id = $1 AND UPPER(admission_number) = UPPER($2) LIMIT 1"
            )
            .bind(school_id)
            .bind(adm)
            .fetch_optional(conn)
            .await?;

            if let Some(student_id) = student_opt {
                sqlx::query(
                    r#"
                    INSERT INTO enrollments (student_id, academic_year_id, class_section_id, status)
                    VALUES ($1, $2, $3, 'ACTIVE')
                    ON CONFLICT (student_id, academic_year_id)
                    DO UPDATE SET class_section_id = EXCLUDED.class_section_id, status = 'ACTIVE'
                    "#
                )
                .bind(student_id)
                .bind(academic_year_id)
                .bind(class_section_id)
                .execute(conn)
                .await?;

                count += 1;
            } else {
                unmatched.push(adm.to_string());
            }
        }
        Ok((count, unmatched))
    }
