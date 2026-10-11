use eravaya::config::AppConfig;
use eravaya::db;

#[tokio::test]
async fn test_apply_cascade_migration_and_verify_deletion() {
    dotenvy::dotenv().ok();
    if std::env::var("DATABASE_URL").is_err() {
        println!("DATABASE_URL is not set. Skipping live DB test.");
        return;
    }
    let config = AppConfig::from_env();
    let pool = db::init_pool(&config)
        .await
        .expect("Failed to connect to db");

    // Apply migration SQL
    let migration_sql =
        include_str!("../migrations/20260925000001_cascade_delete_staff_students_guardians.sql");
    sqlx::raw_sql(migration_sql)
        .execute(&pool)
        .await
        .expect("Failed to execute cascade migration");

    println!("Migration executed successfully!");

    // Check staff count (orphaned ones should now be 0)
    let orphaned_staff_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM staff WHERE user_id IS NULL OR school_id NOT IN (SELECT id FROM schools)"
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    println!(
        "Orphaned staff count after cleanup: {}",
        orphaned_staff_count
    );
    assert_eq!(orphaned_staff_count, 0);

    let unique_suffix = uuid::Uuid::new_v4()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>();
    let unique_code = format!("CAS_{}", unique_suffix);
    let unique_email = format!("cascade_{}@test.com", unique_suffix);
    let unique_username = format!("user_{}", unique_suffix);

    // Test creating a temporary test school and adding a teacher to verify ON DELETE CASCADE
    let test_school_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO schools (name, code, status) VALUES ($1, $2, 'ACTIVE') RETURNING id",
    )
    .bind(format!("Cascade Test School {}", unique_suffix))
    .bind(&unique_code)
    .fetch_one(&pool)
    .await
    .unwrap();

    let password_hash = eravaya::utils::password::hash_password("Welcome123!").unwrap();

    // Create user in this school
    let test_user_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (school_id, first_name, last_name, email, phone, username, password_hash, status)
         VALUES ($1, 'Cascade', 'Teacher', $2, '+919999988888', $3, $4, 'ACTIVE')
         RETURNING id"
    )
    .bind(test_school_id)
    .bind(&unique_email)
    .bind(&unique_username)
    .bind(password_hash)
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let _ =
        eravaya::utils::roles::assign_user_role(&mut conn, test_school_id, test_user_id, "TEACHER")
            .await;

    // Create staff profile
    sqlx::query(
        "INSERT INTO staff (school_id, user_id, employee_id, department) VALUES ($1, $2, 'TCH_CAS_01', 'Science')"
    )
    .bind(test_school_id)
    .bind(test_user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Verify staff exists
    let staff_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM staff WHERE school_id = $1")
        .bind(test_school_id)
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    assert_eq!(staff_before, 1);

    let users_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE school_id = $1")
        .bind(test_school_id)
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    assert_eq!(users_before, 1);

    // Now DELETE the school
    let deleted_rows = sqlx::query("DELETE FROM schools WHERE id = $1")
        .bind(test_school_id)
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(deleted_rows, 1);

    // Verify that the user AND the staff profile were automatically deleted by CASCADE!
    let staff_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM staff WHERE school_id = $1")
        .bind(test_school_id)
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    assert_eq!(staff_after, 0);

    let users_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE school_id = $1")
        .bind(test_school_id)
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    assert_eq!(users_after, 0);

    println!("✅ Verified: Deleting school cascades and deletes all associated teachers/users/staff cleanly!");
}
