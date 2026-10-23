use eravaya::config::AppConfig;
use eravaya::db;
use eravaya::modules::academic::handler::SubjectTypeInput;
use eravaya::modules::academic::repository::AcademicRepository;
use uuid::Uuid;

#[tokio::test]
async fn test_subject_type_and_subject_full_lifecycle() {
    dotenvy::dotenv().ok();

    if std::env::var("DATABASE_URL").is_err() {
        println!("DATABASE_URL is not set. Skipping live DB test.");
        return;
    }

    let config = AppConfig::from_env();
    let pool = db::init_pool(&config)
        .await
        .expect("Failed to connect to PostgreSQL database pool");

    println!("============================================================");
    println!("🧪 TEST: Subject Type & Subject Creation and Deletion Lifecycle");
    println!("============================================================");

    // ─── Step 1: Create an isolated Test School ────────────────────────────────
    let test_suffix = Uuid::new_v4().to_string()[..8].to_string();
    let school_code = format!("TEST_SUBJ_SCH_{}", test_suffix);

    let school_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO schools (name, code, status)
        VALUES ($1, $2, 'ACTIVE')
        RETURNING id
        "#,
    )
    .bind(format!("Subject Lifecycle Test School {}", test_suffix))
    .bind(&school_code)
    .fetch_one(&pool)
    .await
    .expect("Failed to create isolated test school");

    println!("✅ 1. Created isolated test school (ID: {})", school_id);

    // ─── Step 2: Add New Subject Type ──────────────────────────────────────────
    let type_code = format!("ROBO_{}", test_suffix).to_uppercase();
    let type_name = "Robotics & Automation";
    let system_type = "Practical";
    let is_elective = true;
    let is_mandatory = false;
    let type_desc = "Hands-on robotics hardware, sensors, and microcontroller lab";

    println!(
        "▶️ 2. Adding new subject type '{}' (code: '{}')...",
        type_name, type_code
    );

    let add_type_res = AcademicRepository::upsert_subject_type(
        &pool,
        school_id,
        type_name,
        &type_code,
        system_type,
        is_elective,
        is_mandatory,
        Some(type_desc),
    )
    .await;

    assert!(
        add_type_res.is_ok(),
        "Failed to upsert new subject type: {:?}",
        add_type_res.err()
    );

    // ─── Step 3: Verify Subject Type Exists with All Fields ────────────────────
    println!("▶️ 3. Querying subject types list to verify addition...");
    let types_list = AcademicRepository::get_subject_types(&pool, school_id)
        .await
        .expect("Failed to query subject types");

    let found_type = types_list
        .iter()
        .find(|t| t.code == type_code)
        .expect("Newly created subject type not found in get_subject_types");

    assert_eq!(found_type.name, type_name);
    assert_eq!(found_type.code, type_code);
    assert_eq!(found_type.system_subject_type, system_type);
    assert_eq!(found_type.is_elective, is_elective);
    assert_eq!(found_type.is_mandatory, is_mandatory);
    assert_eq!(found_type.description.as_deref(), Some(type_desc));

    let created_type_id = found_type.id;
    println!(
        "✅ 3. Verified subject type successfully saved with ID: {}",
        created_type_id
    );

    // ─── Step 4: Create a Subject Referencing this New Subject Type ───────────
    let subj_code = format!("ROBO_{}", test_suffix).to_uppercase();
    let subj_name = "Introductory Microcontrollers";
    let subj_color = "#0284C7";
    let subj_dept = "Computer Science & IT";
    let subj_desc = "Foundations of embedded electronics and microcontroller firmware";

    println!(
        "▶️ 4. Adding subject '{}' referencing subject type ID {}...",
        subj_name, created_type_id
    );

    let add_subj_res = AcademicRepository::upsert_subject(
        &pool,
        school_id,
        subj_name,
        &subj_code,
        "ELECTIVE",
        system_type,
        Some(created_type_id),
        Some(subj_color),
        Some(subj_dept),
        Some(subj_desc),
    )
    .await;

    assert!(
        add_subj_res.is_ok(),
        "Failed to upsert subject: {:?}",
        add_subj_res.err()
    );

    // ─── Step 5: Verify Subject Exists with Type Linkage ───────────────────────
    let subjects_list = AcademicRepository::get_subjects(&pool, school_id)
        .await
        .expect("Failed to query subjects");

    let found_subject = subjects_list
        .iter()
        .find(|s| s.code == subj_code)
        .expect("Newly created subject not found in get_subjects");

    assert_eq!(found_subject.name, subj_name);
    assert_eq!(found_subject.code, subj_code);
    assert_eq!(found_subject.system_subject_type, system_type);
    assert_eq!(found_subject.subject_type_id, Some(created_type_id));
    assert_eq!(found_subject.color.as_deref(), Some(subj_color));
    assert_eq!(found_subject.department.as_deref(), Some(subj_dept));
    assert_eq!(found_subject.description.as_deref(), Some(subj_desc));

    let created_subject_id = found_subject.id;
    println!(
        "✅ 5. Verified subject exists and is linked to subject type (Subject ID: {})",
        created_subject_id
    );

    // ─── Step 6: Test Subject Update ───────────────────────────────────────────
    let updated_name = "Advanced Microcontrollers & IoT";
    let updated_color = "#059669";
    let updated_dept = "Robotics Engineering";
    let updated_desc = "Sensors, wireless networks, and IoT firmware";

    println!("▶️ 6. Updating subject via update_subject...");
    let update_res = AcademicRepository::update_subject(
        &pool,
        school_id,
        created_subject_id,
        updated_name,
        &subj_code,
        Some("ELECTIVE"),
        system_type,
        Some(updated_color),
        Some(updated_dept),
        Some(updated_desc),
    )
    .await
    .expect("Error executing update_subject");

    assert!(
        update_res,
        "update_subject returned false (subject not found)"
    );

    let updated_subjects = AcademicRepository::get_subjects(&pool, school_id)
        .await
        .expect("Failed to re-query subjects");
    let re_fetched = updated_subjects
        .iter()
        .find(|s| s.id == created_subject_id)
        .unwrap();
    assert_eq!(re_fetched.name, updated_name);
    assert_eq!(re_fetched.color.as_deref(), Some(updated_color));
    assert_eq!(re_fetched.department.as_deref(), Some(updated_dept));
    println!("✅ 6. Verified subject update was committed successfully");

    // ─── Step 7: Delete Subject and Verify Deletion ────────────────────────────
    println!("▶️ 7. Deleting subject ID: {}...", created_subject_id);
    let del_subj_res = AcademicRepository::delete_subject(&pool, school_id, created_subject_id)
        .await
        .expect("Failed to execute delete_subject");

    assert!(
        del_subj_res,
        "delete_subject should return true on successful delete"
    );

    // Re-verify subject list does NOT contain the deleted subject
    let subjects_after_del = AcademicRepository::get_subjects(&pool, school_id)
        .await
        .expect("Failed to query subjects after deletion");

    assert!(
        !subjects_after_del
            .iter()
            .any(|s| s.id == created_subject_id || s.code == subj_code),
        "Deleted subject still found in get_subjects!"
    );

    // Deleting again should return false (already deleted)
    let del_again = AcademicRepository::delete_subject(&pool, school_id, created_subject_id)
        .await
        .expect("Failed to execute second delete_subject");
    assert!(
        !del_again,
        "Deleting a non-existent subject should return false"
    );

    println!("✅ 7. Subject deleted and verified absent from repository");

    // ─── Step 8: Delete Subject Type and Verify Deletion ───────────────────────
    println!("▶️ 8. Deleting subject type ID: {}...", created_type_id);
    let del_type_res = AcademicRepository::delete_subject_type(&pool, school_id, created_type_id)
        .await
        .expect("Failed to execute delete_subject_type");

    assert!(
        del_type_res,
        "delete_subject_type should return true on successful delete"
    );

    // Re-verify subject types list does NOT contain the deleted type
    let types_after_del = AcademicRepository::get_subject_types(&pool, school_id)
        .await
        .expect("Failed to query subject types after deletion");

    assert!(
        !types_after_del
            .iter()
            .any(|t| t.id == created_type_id || t.code == type_code),
        "Deleted subject type still found in get_subject_types!"
    );

    // Deleting again should return false (already deleted)
    let del_type_again = AcademicRepository::delete_subject_type(&pool, school_id, created_type_id)
        .await
        .expect("Failed to execute second delete_subject_type");
    assert!(
        !del_type_again,
        "Deleting a non-existent subject type should return false"
    );

    println!("✅ 8. Subject type deleted and verified absent from repository");

    // ─── Step 9: Verify ON DELETE SET NULL Foreign Key Safety ─────────────────
    println!(
        "▶️ 9. Testing FK ON DELETE SET NULL: Deleting subject type while linked to subject..."
    );
    let fk_type_code = format!("AI_TYPE_{}", test_suffix);
    AcademicRepository::upsert_subject_type(
        &pool,
        school_id,
        "Artificial Intelligence",
        &fk_type_code,
        "Practical",
        true,
        false,
        Some("Machine Learning & Neural Networks"),
    )
    .await
    .unwrap();

    let fk_type = AcademicRepository::get_subject_types(&pool, school_id)
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.code == fk_type_code)
        .unwrap();

    let fk_subj_code = format!("AI_SUBJ_{}", test_suffix);
    AcademicRepository::upsert_subject(
        &pool,
        school_id,
        "Machine Learning 101",
        &fk_subj_code,
        "ELECTIVE",
        "Practical",
        Some(fk_type.id),
        Some("#7C3AED"),
        Some("Computer Science & IT"),
        Some("Deep learning with Python"),
    )
    .await
    .unwrap();

    let fk_subj = AcademicRepository::get_subjects(&pool, school_id)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.code == fk_subj_code)
        .unwrap();

    assert_eq!(fk_subj.subject_type_id, Some(fk_type.id));

    // Delete the subject type while the subject is actively referencing it
    let del_parent_type = AcademicRepository::delete_subject_type(&pool, school_id, fk_type.id)
        .await
        .expect("Deleting parent subject type failed");
    assert!(del_parent_type);

    // The subject must still exist, but its subject_type_id must be gracefully set to None (NULL)
    let fk_subj_after = AcademicRepository::get_subjects(&pool, school_id)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.code == fk_subj_code)
        .expect("Subject was unexpectedly deleted when parent type was deleted");

    assert_eq!(
        fk_subj_after.subject_type_id, None,
        "Foreign key subject_type_id was not set to NULL upon subject type deletion!"
    );

    // Clean up the test subject
    AcademicRepository::delete_subject(&pool, school_id, fk_subj.id)
        .await
        .unwrap();
    println!("✅ 9. Verified FK ON DELETE SET NULL safely sets subject_type_id to NULL");

    // ─── Step 10: Batch Upsert and Idempotency ──────────────────────────────────
    println!("▶️ 10. Testing batch_upsert_subject_types and idempotency...");
    let batch_types = vec![
        SubjectTypeInput {
            name: "Biotechnology".into(),
            code: format!("BIOTECH_{}", test_suffix),
            system_subject_type: Some("Practical".into()),
            is_elective: Some(true),
            is_mandatory: Some(false),
            description: Some("Genetics and molecular biology".into()),
        },
        SubjectTypeInput {
            name: "Aerospace Studies".into(),
            code: format!("AERO_{}", test_suffix),
            system_subject_type: Some("Vocational".into()),
            is_elective: Some(true),
            is_mandatory: Some(false),
            description: Some("Flight dynamics and aerodynamics".into()),
        },
    ];

    let batch_count = AcademicRepository::batch_upsert_subject_types(&pool, school_id, batch_types)
        .await
        .expect("batch_upsert_subject_types failed");
    assert_eq!(batch_count, 2);

    let types_after_batch = AcademicRepository::get_subject_types(&pool, school_id)
        .await
        .unwrap();
    assert!(types_after_batch
        .iter()
        .any(|t| t.code.starts_with("BIOTECH_")));
    assert!(types_after_batch
        .iter()
        .any(|t| t.code.starts_with("AERO_")));

    // Idempotent update: updating BIOTECH with new description
    let updated_batch = vec![SubjectTypeInput {
        name: "Biotechnology & Bioinformatics".into(),
        code: format!("BIOTECH_{}", test_suffix),
        system_subject_type: Some("Practical".into()),
        is_elective: Some(true),
        is_mandatory: Some(false),
        description: Some("Computational biology and genome analysis".into()),
    }];
    AcademicRepository::batch_upsert_subject_types(&pool, school_id, updated_batch)
        .await
        .unwrap();

    let types_after_rebatch = AcademicRepository::get_subject_types(&pool, school_id)
        .await
        .unwrap();
    let re_biotech = types_after_rebatch
        .iter()
        .find(|t| t.code.starts_with("BIOTECH_"))
        .unwrap();
    assert_eq!(re_biotech.name, "Biotechnology & Bioinformatics");
    assert_eq!(
        re_biotech.description.as_deref(),
        Some("Computational biology and genome analysis")
    );

    println!("✅ 10. Verified batch upsert and idempotency");

    // ─── Step 11: Final Isolation Cleanup ─────────────────────────────────────
    println!("▶️ 11. Cleaning up isolated test school...");
    let _ = sqlx::query("DELETE FROM schools WHERE id = $1")
        .bind(school_id)
        .execute(&pool)
        .await;

    println!("============================================================");
    println!("🎉 ALL 11 SUBJECT TYPE LIFECYCLE CHECKS PASSED SUCCESSFULLY!");
    println!("============================================================");
}
