//! Integration tests for the new timetable slot change APIs:
//!   - GET  /api/v1/academic/timetable/slots/:id  (get_slot_by_id)
//!   - PATCH /api/v1/academic/timetable/slots/:id/change-subject
//!   - PATCH /api/v1/academic/timetable/slots/:id/change-class
//!   - POST  /api/v1/academic/timetable/slots/swap
//!
//! All tests call the TimetableService layer directly (same pattern as
//! elective_enrollment_and_clash_test.rs).  Auth/role-based 403 tests are
//! annotated but skipped here because that logic lives in the handler layer;
//! they are covered by HTTP-level tests once the server is running.

use eravaya::modules::academic::timetable::service::{
    BatchSaveSlotsPayload, ChangeSlotClassInput, ChangeSlotSubjectInput, SaveSlotInput,
    SwapSlotsInput, TimetableService,
};
use sqlx::PgPool;
use uuid::Uuid;

// ─── Shared DB setup helper ───────────────────────────────────────────────────

async fn get_pool() -> Option<PgPool> {
    dotenvy::dotenv().ok();
    let _ = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => url,
        _ => {
            println!("DATABASE_URL is not set. Skipping integration test.");
            return None;
        }
    };
    let config = eravaya::config::AppConfig::from_env();
    match eravaya::db::init_pool(&config).await {
        Ok(pool) => Some(pool),
        Err(e) => {
            println!("Could not connect to DB: {}. Skipping.", e);
            None
        }
    }
}

/// A minimal fixture with everything the timetable tests need.
struct TimetableFixture {
    pub school_id: Uuid,
    pub academic_year_id: Uuid,
    pub section_a: Uuid,
    pub section_b: Uuid,
    pub subject_a: Uuid,
    pub subject_b: Uuid,
    pub subject_c: Uuid, // third subject for change-subject duplicate tests
    pub teacher_a: Uuid,
    pub teacher_b: Uuid,
}

/// A unique sentinel day/period pair to avoid colliding with real data.
/// We use day 6 (Saturday) and periods 90–99 which should be unused.
fn test_day() -> i32 {
    6
}

async fn setup_fixture(pool: &PgPool) -> Option<TimetableFixture> {
    // Pick a school that has an active academic year and ≥ 2 class sections.
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
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let (school_id, academic_year_id) = school_opt?;

    let sections: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT cs.id
        FROM class_sections cs
        JOIN classes c ON cs.class_id = c.id
        WHERE c.school_id = $1 AND cs.academic_year_id = $2
        LIMIT 2
        "#,
    )
    .bind(school_id)
    .bind(academic_year_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if sections.len() < 2 {
        println!("Need ≥ 2 class sections, skipping.");
        return None;
    }
    let section_a = sections[0].0;
    let section_b = sections[1].0;

    // Need ≥ 3 subjects.
    let subjects: Vec<(Uuid,)> =
        sqlx::query_as("SELECT id FROM subjects WHERE school_id = $1 LIMIT 3")
            .bind(school_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    if subjects.len() < 3 {
        println!("Need ≥ 3 subjects, skipping.");
        return None;
    }
    let subject_a = subjects[0].0;
    let subject_b = subjects[1].0;
    let subject_c = subjects[2].0;

    // Need ≥ 2 active staff members.
    let staff: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT s.id FROM staff s
        JOIN users u ON s.user_id = u.id
        WHERE s.school_id = $1 AND u.status = 'ACTIVE'
        LIMIT 2
        "#,
    )
    .bind(school_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if staff.len() < 2 {
        println!("Need ≥ 2 active staff, skipping.");
        return None;
    }
    let teacher_a = staff[0].0;
    let teacher_b = staff[1].0;

    Some(TimetableFixture {
        school_id,
        academic_year_id,
        section_a,
        section_b,
        subject_a,
        subject_b,
        subject_c,
        teacher_a,
        teacher_b,
    })
}

/// Delete all test slots created in day 6, periods 90-99.
async fn cleanup(pool: &PgPool, school_id: Uuid) {
    let _ = sqlx::query(
        "DELETE FROM timetable_slots WHERE school_id = $1 AND day_of_week = $2 AND period_number >= 90 AND period_number <= 99",
    )
    .bind(school_id)
    .bind(test_day())
    .execute(pool)
    .await;
}

/// Inserts one timetable slot through the existing batch_upsert_slots path
/// and returns its UUID.
async fn insert_slot(
    pool: &PgPool,
    school_id: Uuid,
    academic_year_id: Uuid,
    class_section_id: Uuid,
    period_number: i32,
    subject_id: Uuid,
    teacher_staff_id: Uuid,
    is_elective: bool,
    elective_group_id: Option<Uuid>,
) -> Uuid {
    TimetableService::batch_upsert_slots(
        pool,
        school_id,
        BatchSaveSlotsPayload {
            class_section_id,
            academic_year_id: Some(academic_year_id),
            slots: vec![SaveSlotInput {
                id: None,
                day_of_week: test_day(),
                period_number,
                start_time: "15:00".into(),
                end_time: "15:45".into(),
                subject_id,
                teacher_staff_id,
                co_teacher_staff_ids: vec![],
                is_shared: false,
                room_id: None,
                elective_group_id,
                is_elective_slot: is_elective,
            }],
        },
    )
    .await
    .expect("insert_slot: batch_upsert_slots failed");

    // Fetch the newly inserted row's id.
    sqlx::query_scalar(
        "SELECT id FROM timetable_slots WHERE school_id = $1 AND class_section_id = $2 AND day_of_week = $3 AND period_number = $4 AND subject_id = $5",
    )
    .bind(school_id)
    .bind(class_section_id)
    .bind(test_day())
    .bind(period_number)
    .bind(subject_id)
    .fetch_one(pool)
    .await
    .expect("insert_slot: could not retrieve inserted slot id")
}

// ═════════════════════════════════════════════════════════════════════════════
// 1.  get_slot_by_id tests
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_slot_by_id_success() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    let slot_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        90,
        fix.subject_a,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    let result = TimetableService::get_slot_by_id(&pool, fix.school_id, slot_id).await;
    assert!(
        result.is_ok(),
        "test_get_slot_by_id_success: expected Ok, got {:?}",
        result.err()
    );
    let slot = result.unwrap();
    assert_eq!(slot.id, slot_id, "test_get_slot_by_id_success: id mismatch");
    assert_eq!(
        slot.subject_id, fix.subject_a,
        "test_get_slot_by_id_success: subject_id mismatch"
    );
    println!("✅ test_get_slot_by_id_success PASS");

    cleanup(&pool, fix.school_id).await;
}

#[tokio::test]
async fn test_get_slot_by_id_wrong_school() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    let slot_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        91,
        fix.subject_a,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    // Use a random (different) school_id — should return NotFound.
    let wrong_school = Uuid::new_v4();
    let result = TimetableService::get_slot_by_id(&pool, wrong_school, slot_id).await;
    assert!(
        result.is_err(),
        "test_get_slot_by_id_wrong_school: expected Err (NotFound), got Ok"
    );
    println!("✅ test_get_slot_by_id_wrong_school PASS");

    cleanup(&pool, fix.school_id).await;
}

#[tokio::test]
async fn test_get_slot_by_id_not_found() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };

    let nonexistent_id = Uuid::new_v4();
    let result = TimetableService::get_slot_by_id(&pool, fix.school_id, nonexistent_id).await;
    assert!(
        result.is_err(),
        "test_get_slot_by_id_not_found: expected Err (NotFound), got Ok"
    );
    println!("✅ test_get_slot_by_id_not_found PASS");
}

// ═════════════════════════════════════════════════════════════════════════════
// 2.  change_slot_subject tests
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_change_subject_success() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    // Insert a slot with subject_a / teacher_a.
    let slot_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        92,
        fix.subject_a,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    // Change to subject_b (teacher_b has no other slot at day 6 period 92).
    let result = TimetableService::change_slot_subject(
        &pool,
        fix.school_id,
        slot_id,
        ChangeSlotSubjectInput {
            new_subject_id: fix.subject_b,
            new_teacher_staff_id: Some(fix.teacher_b),
            new_room_id: None,
            new_elective_group_id: None,
            is_elective_slot: false,
            co_teacher_staff_ids: vec![],
            is_shared: false,
        },
    )
    .await;

    assert!(
        result.is_ok(),
        "test_change_subject_success: expected Ok, got {:?}",
        result.err()
    );
    let updated = result.unwrap();
    assert_eq!(
        updated.subject_id, fix.subject_b,
        "test_change_subject_success: subject not updated"
    );
    assert_eq!(
        updated.teacher_staff_id, fix.teacher_b,
        "test_change_subject_success: teacher not updated"
    );
    println!("✅ test_change_subject_success PASS");

    cleanup(&pool, fix.school_id).await;
}

#[tokio::test]
async fn test_change_subject_teacher_clash() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    // teacher_b is already busy at day 6 period 93 (section_b).
    insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_b,
        93,
        fix.subject_a,
        fix.teacher_b,
        false,
        None,
    )
    .await;

    // Insert target slot in section_a at the same day/period with teacher_a.
    let slot_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        93,
        fix.subject_b,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    // Attempting to reassign teacher_b to section_a at the same period must fail with Conflict.
    let result = TimetableService::change_slot_subject(
        &pool,
        fix.school_id,
        slot_id,
        ChangeSlotSubjectInput {
            new_subject_id: fix.subject_b,
            new_teacher_staff_id: Some(fix.teacher_b), // already booked
            new_room_id: None,
            new_elective_group_id: None,
            is_elective_slot: false,
            co_teacher_staff_ids: vec![],
            is_shared: false,
        },
    )
    .await;

    assert!(
        result.is_err(),
        "test_change_subject_teacher_clash: expected Err (Conflict), got Ok"
    );
    println!("✅ test_change_subject_teacher_clash PASS");

    cleanup(&pool, fix.school_id).await;
}

#[tokio::test]
async fn test_change_subject_duplicate_subject_at_slot() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    // section_a already has subject_b at day 6 period 94.
    insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        94,
        fix.subject_b,
        fix.teacher_b,
        false,
        None,
    )
    .await;

    // Insert a second slot for subject_c in section_a at the same period.
    let slot_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        94,
        fix.subject_c,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    // Changing subject_c → subject_b for the same class/day/period must 409.
    let result = TimetableService::change_slot_subject(
        &pool,
        fix.school_id,
        slot_id,
        ChangeSlotSubjectInput {
            new_subject_id: fix.subject_b, // already present at same slot
            new_teacher_staff_id: Some(fix.teacher_a),
            new_room_id: None,
            new_elective_group_id: None,
            is_elective_slot: false,
            co_teacher_staff_ids: vec![],
            is_shared: false,
        },
    )
    .await;

    assert!(
        result.is_err(),
        "test_change_subject_duplicate_subject_at_slot: expected Err (Conflict), got Ok"
    );
    println!("✅ test_change_subject_duplicate_subject_at_slot PASS");

    cleanup(&pool, fix.school_id).await;
}

#[tokio::test]
async fn test_change_subject_elective_with_group() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    // Check if any elective_group exists for this school; skip gracefully if not.
    let elective_group_opt: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM elective_groups WHERE school_id = $1 LIMIT 1")
            .bind(fix.school_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);

    let elective_group_id = match elective_group_opt {
        Some((id,)) => id,
        None => {
            println!(
                "No elective group found for school, skipping test_change_subject_elective_with_group"
            );
            return;
        }
    };

    let slot_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        95,
        fix.subject_a,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    let result = TimetableService::change_slot_subject(
        &pool,
        fix.school_id,
        slot_id,
        ChangeSlotSubjectInput {
            new_subject_id: fix.subject_b,
            new_teacher_staff_id: Some(fix.teacher_b),
            new_room_id: None,
            new_elective_group_id: Some(elective_group_id),
            is_elective_slot: true,
            co_teacher_staff_ids: vec![],
            is_shared: false,
        },
    )
    .await;

    assert!(
        result.is_ok(),
        "test_change_subject_elective_with_group: expected Ok, got {:?}",
        result.err()
    );
    let updated = result.unwrap();
    assert!(
        updated.is_elective_slot,
        "test_change_subject_elective_with_group: is_elective_slot should be true"
    );
    assert_eq!(
        updated.elective_group_id,
        Some(elective_group_id),
        "test_change_subject_elective_with_group: elective_group_id mismatch"
    );
    println!("✅ test_change_subject_elective_with_group PASS");

    cleanup(&pool, fix.school_id).await;
}

/// NOTE: test_change_subject_unauthorized (403 Forbidden for teacher role)
/// requires the full Axum app with auth session middleware and cannot be tested
/// at the service layer. This test is a placeholder documenting the coverage gap;
/// it will be implemented as an HTTP-level test once the server is running in a
/// test harness with real session cookies.
#[tokio::test]
async fn test_change_subject_unauthorized_documented() {
    // Service layer has no concept of roles — skipping with an informational note.
    println!("ℹ️  test_change_subject_unauthorized: auth/role check lives in the handler layer.");
    println!("   Will be covered by HTTP integration tests (reqwest against a live test server).");
    println!("   SKIP (not a failure)");
}

// ═════════════════════════════════════════════════════════════════════════════
// 3.  change_slot_class tests
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_change_class_success() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    let slot_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        96,
        fix.subject_a,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    // section_b has no slot at day 6 period 96 — reassignment should succeed.
    let result = TimetableService::change_slot_class(
        &pool,
        fix.school_id,
        slot_id,
        ChangeSlotClassInput {
            new_class_section_id: fix.section_b,
            new_teacher_staff_id: Some(fix.teacher_a),
            new_room_id: None,
            is_shared: false,
        },
    )
    .await;

    assert!(
        result.is_ok(),
        "test_change_class_success: expected Ok, got {:?}",
        result.err()
    );
    let updated = result.unwrap();
    assert_eq!(
        updated.class_section_id, fix.section_b,
        "test_change_class_success: class_section_id not updated"
    );
    println!("✅ test_change_class_success PASS");

    cleanup(&pool, fix.school_id).await;
}

#[tokio::test]
async fn test_change_class_slot_exists_in_target() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    // section_b already has subject_a at day 6 period 97.
    insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_b,
        97,
        fix.subject_a,
        fix.teacher_b,
        false,
        None,
    )
    .await;

    // Insert the same subject_a into section_a at the same period.
    let slot_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        97,
        fix.subject_a,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    // Moving section_a's slot to section_b at same day/period/subject must 409.
    let result = TimetableService::change_slot_class(
        &pool,
        fix.school_id,
        slot_id,
        ChangeSlotClassInput {
            new_class_section_id: fix.section_b,
            new_teacher_staff_id: Some(fix.teacher_a),
            new_room_id: None,
            is_shared: false,
        },
    )
    .await;

    assert!(
        result.is_err(),
        "test_change_class_slot_exists_in_target: expected Err (Conflict), got Ok"
    );
    println!("✅ test_change_class_slot_exists_in_target PASS");

    cleanup(&pool, fix.school_id).await;
}

/// NOTE: test_change_class_unauthorized — same rationale as change_subject_unauthorized.
#[tokio::test]
async fn test_change_class_unauthorized_documented() {
    println!("ℹ️  test_change_class_unauthorized: auth/role check lives in the handler layer.");
    println!("   SKIP (not a failure)");
}

// ═════════════════════════════════════════════════════════════════════════════
// 4.  swap_slots tests
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_swap_slots_success() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    // Slot A: section_a, day 6 period 97, subject_a / teacher_a
    let slot_a_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        97,
        fix.subject_a,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    // Slot B: section_a, day 6 period 98, subject_b / teacher_b
    // (use teacher_b so there is no teacher clash after the swap)
    let slot_b_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        98,
        fix.subject_b,
        fix.teacher_b,
        false,
        None,
    )
    .await;

    let result = TimetableService::swap_slots(
        &pool,
        fix.school_id,
        SwapSlotsInput {
            slot_a_id,
            slot_b_id,
        },
    )
    .await;

    assert!(
        result.is_ok(),
        "test_swap_slots_success: expected Ok, got {:?}",
        result.err()
    );
    let (new_a, new_b) = result.unwrap();

    // After swap: slot A should be at period 98, slot B at period 97.
    assert_eq!(
        new_a.period_number, 98,
        "test_swap_slots_success: slot_a period_number should be 98 after swap"
    );
    assert_eq!(
        new_b.period_number, 97,
        "test_swap_slots_success: slot_b period_number should be 97 after swap"
    );
    // Subjects and teachers must NOT have changed.
    assert_eq!(
        new_a.subject_id, fix.subject_a,
        "test_swap_slots_success: slot_a subject_id must stay subject_a"
    );
    assert_eq!(
        new_b.subject_id, fix.subject_b,
        "test_swap_slots_success: slot_b subject_id must stay subject_b"
    );
    println!("✅ test_swap_slots_success PASS");

    cleanup(&pool, fix.school_id).await;
}

#[tokio::test]
async fn test_swap_slots_different_school() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    let slot_a_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        98,
        fix.subject_a,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    // slot_b_id is a random UUID that belongs to no school.
    let fake_slot_b = Uuid::new_v4();

    let result = TimetableService::swap_slots(
        &pool,
        fix.school_id,
        SwapSlotsInput {
            slot_a_id,
            slot_b_id: fake_slot_b,
        },
    )
    .await;

    assert!(
        result.is_err(),
        "test_swap_slots_different_school: expected Err (NotFound/Forbidden), got Ok"
    );
    println!("✅ test_swap_slots_different_school PASS");

    cleanup(&pool, fix.school_id).await;
}

#[tokio::test]
async fn test_swap_slots_same_slot() {
    let pool = match get_pool().await {
        Some(p) => p,
        None => return,
    };
    let fix = match setup_fixture(&pool).await {
        Some(f) => f,
        None => return,
    };
    cleanup(&pool, fix.school_id).await;

    let slot_id = insert_slot(
        &pool,
        fix.school_id,
        fix.academic_year_id,
        fix.section_a,
        99,
        fix.subject_a,
        fix.teacher_a,
        false,
        None,
    )
    .await;

    // Swapping a slot with itself must be rejected (BadRequest / 400).
    let result = TimetableService::swap_slots(
        &pool,
        fix.school_id,
        SwapSlotsInput {
            slot_a_id: slot_id,
            slot_b_id: slot_id,
        },
    )
    .await;

    assert!(
        result.is_err(),
        "test_swap_slots_same_slot: expected Err (BadRequest), got Ok"
    );
    println!("✅ test_swap_slots_same_slot PASS");

    cleanup(&pool, fix.school_id).await;
}
