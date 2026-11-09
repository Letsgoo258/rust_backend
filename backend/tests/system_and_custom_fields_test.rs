use eravaya::modules::users::repository::UserRepository;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

async fn get_db_pool() -> Option<PgPool> {
    dotenvy::dotenv().ok();
    let _db_url = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => url,
        _ => {
            println!("DATABASE_URL is not set. Skipping live DB test.");
            return None;
        }
    };
    let config = eravaya::config::AppConfig::from_env();
    match eravaya::db::init_pool(&config).await {
        Ok(pool) => Some(pool),
        Err(e) => {
            println!(
                "Failed to connect to DATABASE_URL: {}. Skipping live DB test.",
                e
            );
            None
        }
    }
}

#[tokio::test]
async fn test_system_and_custom_fields_in_db_and_api() {
    let pool = match get_db_pool().await {
        Some(p) => p,
        None => return,
    };

    println!("\n================================================================================");
    println!("🧪 STARTING SYSTEM FIELDS & CUSTOM FIELDS COMPREHENSIVE VERIFICATION TEST");
    println!("================================================================================");

    // ─────────────────────────────────────────────────────────────────────────────
    // STEP 1: Verify all 14 Strict Core & System Fields Exist in PostgreSQL Database
    // ─────────────────────────────────────────────────────────────────────────────
    println!("\n▶️ [STEP 1] Checking all Core & Demographic System Columns in 'users' table...");

    let expected_user_columns = vec![
        "id",
        "school_id",
        "username",
        "first_name",
        "last_name",
        "display_name",
        "email",
        "phone",
        "date_of_birth",
        "gender",
        "blood_group",
        "caste_category",
        "admission_number",
        "class_section_id",
        "status",
        "custom_values",
    ];

    for col in &expected_user_columns {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 
                FROM information_schema.columns 
                WHERE table_name = 'users' AND column_name = $1
            )
            "#,
        )
        .bind(col)
        .fetch_one(&pool)
        .await
        .expect("Failed to check column in information_schema");

        assert!(
            exists,
            "CRITICAL: Column '{}' is missing in 'users' table!",
            col
        );
        println!("  ├─ Column verified: users.{} [EXISTS ✅]", col);
    }

    // Verify user_custom_values table exists with proper schema
    println!("\n▶️ [STEP 2] Checking 'user_custom_values' table structure...");
    let custom_values_tbl_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 
            FROM information_schema.tables 
            WHERE table_name = 'user_custom_values'
        )
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to check user_custom_values table");

    assert!(
        custom_values_tbl_exists,
        "Table 'user_custom_values' does not exist in DB!"
    );
    println!("  ├─ Table verified: 'user_custom_values' [EXISTS ✅]");

    let custom_fields_tbl_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 
            FROM information_schema.tables 
            WHERE table_name = 'custom_fields'
        )
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to check custom_fields table");

    assert!(
        custom_fields_tbl_exists,
        "Table 'custom_fields' does not exist in DB!"
    );
    println!("  ├─ Table verified: 'custom_fields' [EXISTS ✅]");

    // ─────────────────────────────────────────────────────────────────────────────
    // STEP 3: Setup Test School and Admin User
    // ─────────────────────────────────────────────────────────────────────────────
    println!("\n▶️ [STEP 3] Setting up test school and admin user...");
    sqlx::query("DELETE FROM users WHERE username IN ('test_cf_admin', 'test_student_cf')")
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM schools WHERE code = 'TEST_CUSTOM_FIELDS_ACADEMY'")
        .execute(&pool)
        .await
        .ok();

    let school_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO schools (name, code, status)
        VALUES ('Custom Fields Test Academy', 'TEST_CUSTOM_FIELDS_ACADEMY', 'ACTIVE')
        ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name
        RETURNING id
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to setup test school");

    sqlx::query("DELETE FROM custom_fields WHERE school_id = $1")
        .bind(school_id)
        .execute(&pool)
        .await
        .ok();

    let admin_user_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (id, school_id, username, password_hash, first_name, last_name, email, phone, status)
        VALUES ($1, $2, 'test_cf_admin', 'HASH', 'CustomField', 'Admin', 'cf_admin@test.school.org', '+919999900001', 'ACTIVE')
        ON CONFLICT (school_id, username) DO UPDATE SET first_name = EXCLUDED.first_name
        RETURNING id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(school_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to insert admin user");

    // Assign admin role
    let mut conn = pool.acquire().await.expect("Failed to acquire connection");
    let role_admin_id = eravaya::utils::roles::get_or_create_role_id(
        &mut conn,
        school_id,
        "SCHOOL_ADMIN",
        "School Admin",
    )
    .await
    .expect("Failed to create admin role");
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(admin_user_id)
        .bind(role_admin_id)
        .execute(&pool)
        .await
        .expect("Failed to assign admin role");

    let role_student_id =
        eravaya::utils::roles::get_or_create_role_id(&mut conn, school_id, "STUDENT", "Student")
            .await
            .expect("Failed to create student role");
    drop(conn);

    // ─────────────────────────────────────────────────────────────────────────────
    // STEP 4: Define and Save Custom Fields (e.g. locker_number, emergency_route)
    // ─────────────────────────────────────────────────────────────────────────────
    println!("\n▶️ [STEP 4] Defining custom fields in 'custom_fields' table...");
    let cf1_key = "test_locker_num";
    let cf1_name = "Assigned Locker Number";
    let cf2_key = "test_hostel_wing";
    let cf2_name = "Hostel Building Wing";

    let cf1_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO custom_fields (school_id, entity_type, field_name, field_key, field_type, is_required)
        VALUES ($1, 'STUDENT', $2, $3, 'TEXT', false)
        RETURNING id
        "#,
    )
    .bind(school_id)
    .bind(cf1_name)
    .bind(cf1_key)
    .fetch_one(&pool)
    .await
    .expect("Failed to create custom field 1");

    let cf2_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO custom_fields (school_id, entity_type, field_name, field_key, field_type, is_required, options_json)
        VALUES ($1, 'STUDENT', $2, $3, 'SELECT', false, '["North Wing", "South Wing", "East Wing"]'::jsonb)
        RETURNING id
        "#,
    )
    .bind(school_id)
    .bind(cf2_name)
    .bind(cf2_key)
    .fetch_one(&pool)
    .await
    .expect("Failed to create custom field 2");

    println!(
        "  ├─ Created Custom Field 1: id={}, key='{}'",
        cf1_id, cf1_key
    );
    println!(
        "  ├─ Created Custom Field 2: id={}, key='{}'",
        cf2_id, cf2_key
    );

    // ─────────────────────────────────────────────────────────────────────────────
    // STEP 5: Create Student User and Save Caste + Custom Fields via Bulk / Profile
    // ─────────────────────────────────────────────────────────────────────────────
    println!("\n▶️ [STEP 5] Testing creation and saving of Caste & Custom Fields...");
    let initial_adm = "TEST-ADM-9901";

    let student_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (id, school_id, username, password_hash, first_name, last_name, admission_number, status)
        VALUES ($1, $2, 'test_student_cf', 'HASH', 'Aarav', 'Sharma', $3, 'ACTIVE')
        ON CONFLICT (school_id, username) DO UPDATE SET admission_number = EXCLUDED.admission_number
        RETURNING id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(school_id)
    .bind(initial_adm)
    .fetch_one(&pool)
    .await
    .expect("Failed to create initial student");

    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(student_id)
        .bind(role_student_id)
        .execute(&pool)
        .await
        .expect("Failed to assign student role");

    // Perform update simulating Bulk Update or Edit Profile with caste and custom values
    let mut custom_fields_map = HashMap::new();
    custom_fields_map.insert(cf1_key.to_string(), serde_json::json!("Locker #402"));
    custom_fields_map.insert(cf2_key.to_string(), serde_json::json!("North Wing"));

    let custom_values_json = serde_json::to_value(&custom_fields_map).unwrap();
    let caste_val = "OBC";

    sqlx::query(
        r#"
        UPDATE users
        SET
            caste_category = $1,
            custom_values = $2,
            updated_at = NOW()
        WHERE id = $3
        "#,
    )
    .bind(caste_val)
    .bind(&custom_values_json)
    .bind(student_id)
    .execute(&pool)
    .await
    .expect("Failed to update users caste and custom_values");

    // Upsert into user_custom_values
    for (f_key, f_val) in &custom_fields_map {
        let val_str = f_val.as_str().unwrap_or("");
        sqlx::query(
            r#"
            INSERT INTO user_custom_values (school_id, user_id, field_id, value, updated_at)
            SELECT $1, $2, id, $3, NOW()
            FROM custom_fields
            WHERE school_id = $1 AND field_key = $4
            ON CONFLICT (user_id, field_id)
            DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
            "#,
        )
        .bind(school_id)
        .bind(student_id)
        .bind(val_str)
        .bind(f_key)
        .execute(&pool)
        .await
        .expect("Failed to upsert user_custom_values");
    }

    println!(
        "  ├─ Successfully saved caste='{}' and custom_fields in database.",
        caste_val
    );

    // ─────────────────────────────────────────────────────────────────────────────
    // STEP 6: Verify Persistence and Retrieval via UserRepository::list_users
    // ─────────────────────────────────────────────────────────────────────────────
    println!("\n▶️ [STEP 6] Calling UserRepository::list_users to verify retrieval...");
    let mut filters = HashMap::new();
    filters.insert("admission_number".to_string(), initial_adm.to_string());

    let (users_found, total_count) = UserRepository::list_users(&pool, school_id, &filters, None)
        .await
        .expect("Failed to call list_users");

    assert_eq!(
        total_count, 1,
        "Should find exactly 1 user with admission number"
    );
    let fetched = &users_found[0];

    assert_eq!(fetched.id, student_id);
    assert_eq!(
        fetched.caste_category.as_deref(),
        Some("OBC"),
        "Caste category must match saved value"
    );

    let returned_custom = fetched
        .custom_fields
        .as_ref()
        .expect("custom_fields JSON must be present");
    assert_eq!(returned_custom[cf1_key], "Locker #402");
    assert_eq!(returned_custom[cf2_key], "North Wing");

    println!(
        "  ├─ Retrieved caste_category: {:?}",
        fetched.caste_category
    );
    println!("  ├─ Retrieved custom_fields: {}", returned_custom);
    println!("  └─ Retrievability check PASSED ✅");

    // ─────────────────────────────────────────────────────────────────────────────
    // STEP 7: Verify user_custom_values Table Relational Storage
    // ─────────────────────────────────────────────────────────────────────────────
    println!("\n▶️ [STEP 7] Verifying user_custom_values table relational records...");
    let stored_records: Vec<(String, String)> = sqlx::query(
        r#"
        SELECT cf.field_key, ucv.value
        FROM user_custom_values ucv
        JOIN custom_fields cf ON cf.id = ucv.field_id
        WHERE ucv.user_id = $1
        ORDER BY cf.field_key ASC
        "#,
    )
    .bind(student_id)
    .map(|row: sqlx::postgres::PgRow| (row.get(0), row.get(1)))
    .fetch_all(&pool)
    .await
    .expect("Failed to query user_custom_values");

    assert_eq!(
        stored_records.len(),
        2,
        "Both custom field relational rows must exist"
    );
    assert_eq!(stored_records[0].0, cf2_key);
    assert_eq!(stored_records[0].1, "North Wing");
    assert_eq!(stored_records[1].0, cf1_key);
    assert_eq!(stored_records[1].1, "Locker #402");

    println!(
        "  ├─ Verified relational record 1: {} = {}",
        stored_records[0].0, stored_records[0].1
    );
    println!(
        "  ├─ Verified relational record 2: {} = {}",
        stored_records[1].0, stored_records[1].1
    );
    println!("  └─ Relational integrity PASSED ✅");

    // ─────────────────────────────────────────────────────────────────────────────
    // STEP 8: Cleanup Test Data
    // ─────────────────────────────────────────────────────────────────────────────
    println!("\n▶️ [STEP 8] Cleaning up test data...");
    sqlx::query("DELETE FROM users WHERE school_id = $1")
        .bind(school_id)
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM custom_fields WHERE school_id = $1")
        .bind(school_id)
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM schools WHERE id = $1")
        .bind(school_id)
        .execute(&pool)
        .await
        .ok();

    println!("================================================================================");
    println!("🎉 ALL SYSTEM FIELDS & CUSTOM FIELDS VERIFICATION TESTS PASSED SUCCESSFULLY! ✅");
    println!("================================================================================\n");
}
