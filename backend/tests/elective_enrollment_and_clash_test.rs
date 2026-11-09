use chrono::NaiveDate;
use eravaya::modules::academic::electives::{BulkAssignElectiveInput, ElectiveService};
use eravaya::modules::academic::timetable::service::{
    BatchSaveSlotsPayload, CheckClashRequest, SaveSlotInput, TimetableService,
};
use eravaya::modules::attendance::repository::AttendanceRepository;
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::test]
async fn test_elective_parallel_slots_clash_and_attendance_segregation() {
    let _ = dotenvy::dotenv();
    let db_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            println!("Skipping integration test: DATABASE_URL not set");
            return;
        }
    };

    let pool = match PgPool::connect(&db_url).await {
        Ok(p) => p,
        Err(err) => {
            println!(
                "Skipping integration test: could not connect to DB: {}",
                err
            );
            return;
        }
    };

    // 1. Fetch an existing active school with active academic year and >= 2 sections
    let school_opt: Option<(Uuid, Uuid)> = sqlx::query_as(
        r#"
        SELECT c.school_id, cs.academic_year_id
        FROM class_sections cs
        JOIN classes c ON cs.class_id = c.id
        JOIN academic_years ay ON cs.academic_year_id = ay.id AND ay.status = 'ACTIVE'
        GROUP BY c.school_id, cs.academic_year_id
        HAVING COUNT(cs.id) >= 2
        LIMIT 1
        "#,
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let (school_id, academic_year_id) = match school_opt {
        Some(pair) => pair,
        None => {
            println!("No active school with active academic year found, skipping DB test");
            return;
        }
    };

    // 2. Pick two class sections for testing clashes & parallel slots
    let sections: Vec<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT cs.id, CONCAT(c.name, ' - ', s.name)
        FROM class_sections cs
        JOIN classes c ON cs.class_id = c.id
        JOIN sections s ON cs.section_id = s.id
        WHERE c.school_id = $1 AND cs.academic_year_id = $2
        LIMIT 2
        "#,
    )
    .bind(school_id)
    .bind(academic_year_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if sections.len() < 2 {
        println!("Fewer than 2 class sections found, skipping clash test");
        return;
    }

    let section_a = sections[0].0;
    let section_b = sections[1].0;

    // 3. Find or pick 2 subjects
    let subjects: Vec<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT id, name
        FROM subjects
        WHERE school_id = $1
        LIMIT 2
        "#,
    )
    .bind(school_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if subjects.len() < 2 {
        println!("Fewer than 2 subjects found, skipping test");
        return;
    }

    let subject_a = subjects[0].0;
    let subject_b = subjects[1].0;

    // 4. Find or pick a teacher staff record
    let staff_opt: Option<(Uuid, Uuid)> = sqlx::query_as(
        r#"
        SELECT s.id, s.user_id
        FROM staff s
        JOIN users u ON s.user_id = u.id
        WHERE s.school_id = $1 AND u.status = 'ACTIVE'
        LIMIT 1
        "#,
    )
    .bind(school_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let (teacher_staff_id, teacher_user_id) = match staff_opt {
        Some(pair) => pair,
        None => {
            println!("No staff found for school, skipping clash test");
            return;
        }
    };

    println!(
        "Testing with school: {}, section_a: {}, section_b: {}, staff_id: {}, user_id: {}",
        school_id, section_a, section_b, teacher_staff_id, teacher_user_id
    );

    // ──────────────────────────────────────────────────────────────────────────
    // TEST 1: Parallel Slots in the same section, day, period (different subjects)
    // ──────────────────────────────────────────────────────────────────────────
    let test_day = 6; // Saturday
    let test_period = 8; // Valid period number within range 1..=12

    // Clean up test slots first
    let _ = sqlx::query(
        "DELETE FROM timetable_slots WHERE school_id = $1 AND day_of_week = $2 AND period_number = $3",
    )
    .bind(school_id)
    .bind(test_day)
    .bind(test_period)
    .execute(&pool)
    .await;

    // Slot 1: Section A, Day 6, Period 99, Subject A (Elective 1)
    let slot_input_1 = SaveSlotInput {
        id: None,
        day_of_week: test_day,
        period_number: test_period,
        start_time: "15:00".to_string(),
        end_time: "15:45".to_string(),
        subject_id: subject_a,
        teacher_staff_id,
        co_teacher_staff_ids: vec![],
        is_shared: false,
        room_id: None,
        elective_group_id: None,
        is_elective_slot: true,
    };

    // Slot 2: Section A, Day 6, Period 99, Subject B (Elective 2)
    let slot_input_2 = SaveSlotInput {
        id: None,
        day_of_week: test_day,
        period_number: test_period,
        start_time: "15:00".to_string(),
        end_time: "15:45".to_string(),
        subject_id: subject_b,
        teacher_staff_id,
        co_teacher_staff_ids: vec![],
        is_shared: true,
        room_id: None,
        elective_group_id: None,
        is_elective_slot: true,
    };

    let upsert_res = TimetableService::batch_upsert_slots(
        &pool,
        school_id,
        BatchSaveSlotsPayload {
            class_section_id: section_a,
            academic_year_id: Some(academic_year_id),
            slots: vec![slot_input_1, slot_input_2],
        },
    )
    .await;

    assert!(
        upsert_res.is_ok(),
        "Parallel slots upsert failed: {:?}",
        upsert_res.err()
    );

    // Verify both slots exist for Section A, Day 6, Period 99
    let slots = TimetableService::get_section_timetable(&pool, school_id, section_a)
        .await
        .expect("Failed to get section timetable");

    let parallel_slots: Vec<_> = slots
        .iter()
        .filter(|s| s.day_of_week == test_day && s.period_number == test_period)
        .collect();

    assert_eq!(
        parallel_slots.len(),
        2,
        "Expected 2 parallel slots in period 99"
    );
    assert!(parallel_slots.iter().any(|s| s.subject_id == subject_a));
    assert!(parallel_slots.iter().any(|s| s.subject_id == subject_b));
    println!("✓ Successfully verified parallel elective slots in the same timetable period!");

    // ──────────────────────────────────────────────────────────────────────────
    // TEST 2: Clash Detection across sections
    // ──────────────────────────────────────────────────────────────────────────
    // Slot 1 had teacher_staff_id in Section A, Day 6, Period 8.
    // Checking Section B, Day 6, Period 8 with the same teacher_staff_id should flag a clash!
    let clash_check = TimetableService::check_clashes(
        &pool,
        school_id,
        &CheckClashRequest {
            day_of_week: test_day,
            period_number: test_period,
            teacher_staff_id,
            room_id: None,
            exclude_slot_id: None,
            class_section_id: Some(section_b),
        },
    )
    .await
    .expect("Check clashes failed");

    assert!(
        clash_check.has_clash,
        "Expected clash for teacher double-booking across sections"
    );
    assert!(
        !clash_check.clashes.is_empty(),
        "Expected at least one clash detail"
    );
    println!("✓ Successfully verified cross-section teacher clash detection!");

    // ──────────────────────────────────────────────────────────────────────────
    // TEST 3: Student Segregation & Elective Roster Filtering
    // ──────────────────────────────────────────────────────────────────────────
    // Get students in Section A
    let section_students =
        AttendanceRepository::get_enrolled_roster(&pool, school_id, section_a, None)
            .await
            .expect("Failed to get full section roster");

    if section_students.len() >= 2 {
        let segregated_student = section_students[0].student_id;
        println!(
            "Segregating student {} for subject {}",
            segregated_student, subject_a
        );

        // Bulk assign only student 0 to Subject A
        let assign_count = ElectiveService::bulk_assign_students(
            &pool,
            school_id,
            BulkAssignElectiveInput {
                class_section_id: section_a,
                subject_id: subject_a,
                elective_group_id: None,
                student_ids: vec![segregated_student],
            },
        )
        .await
        .expect("Bulk assign failed");

        assert_eq!(assign_count, 1);

        // Fetch roster for Subject A -> MUST only contain the segregated student!
        let elective_roster =
            AttendanceRepository::get_enrolled_roster(&pool, school_id, section_a, Some(subject_a))
                .await
                .expect("Failed to get elective roster");

        assert_eq!(
            elective_roster.len(),
            1,
            "Elective roster should only return the 1 segregated student"
        );
        assert_eq!(elective_roster[0].student_id, segregated_student);
        println!("✓ Successfully verified elective roster filtering for segregated subject!");

        // Fetch roster for Subject B (which has no custom enrollments) -> MUST return full class!
        let core_roster =
            AttendanceRepository::get_enrolled_roster(&pool, school_id, section_a, Some(subject_b))
                .await
                .expect("Failed to get core roster");

        assert_eq!(
            core_roster.len(),
            section_students.len(),
            "Unsegregated subject must return full class section"
        );
        println!("✓ Successfully verified fallback to full class roster for unsegregated subject!");
    } else {
        println!("Class section has fewer than 2 students, skipping enrollment segregation check");
    }

    // ──────────────────────────────────────────────────────────────────────────
    // TEST 4: Parallel Attendance Sessions on the same period
    // ──────────────────────────────────────────────────────────────────────────
    let test_date = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();

    let mut tx = pool.begin().await.expect("Failed to begin transaction");
    let session_1_uuid = Uuid::new_v4();
    let session_1_id = AttendanceRepository::upsert_attendance_session(
        &mut tx,
        session_1_uuid,
        school_id,
        academic_year_id,
        section_a,
        test_date,
        Some(test_period),
        "PERIOD_WISE",
        Some(subject_a),
        teacher_user_id,
        "TEACHER",
        true,
    )
    .await
    .expect("Failed to upsert parallel session 1");

    let session_2_uuid = Uuid::new_v4();
    let session_2_id = AttendanceRepository::upsert_attendance_session(
        &mut tx,
        session_2_uuid,
        school_id,
        academic_year_id,
        section_a,
        test_date,
        Some(test_period),
        "PERIOD_WISE",
        Some(subject_b),
        teacher_user_id,
        "TEACHER",
        true,
    )
    .await
    .expect("Failed to upsert parallel session 2");

    assert_ne!(
        session_1_id, session_2_id,
        "Parallel attendance sessions must have unique IDs"
    );
    tx.commit()
        .await
        .expect("Failed to commit test transaction");
    println!(
        "✓ Successfully verified parallel attendance sessions in the same period without conflict!"
    );

    // Clean up test data
    let _ = sqlx::query("DELETE FROM attendance_sessions WHERE id IN ($1, $2)")
        .bind(session_1_id)
        .bind(session_2_id)
        .execute(&pool)
        .await;

    let _ = sqlx::query(
        "DELETE FROM timetable_slots WHERE school_id = $1 AND day_of_week = $2 AND period_number = $3",
    )
    .bind(school_id)
    .bind(test_day)
    .bind(test_period)
    .execute(&pool)
    .await;

    println!("All elective enrollment, clash detection, and parallel slot tests passed!");
}
