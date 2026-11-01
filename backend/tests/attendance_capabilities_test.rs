use eravaya::modules::auth::backend::User;
use eravaya::modules::whatsapp::menus::attendance::get_teacher_available_classes;
use eravaya::modules::whatsapp::repository::WhatsAppRepository;
use eravaya::modules::whatsapp::service::WhatsAppMessageService;
use eravaya::utils::capabilities::CapabilityController;
use sqlx::PgPool;
use uuid::Uuid;

#[test]
fn test_capabilities_dot_and_colon_aliases_and_admin_block() {
    let admin_user = User {
        id: Uuid::new_v4(),
        school_id: Uuid::new_v4(),
        username: "admin_test".to_string(),
        password_hash: "".to_string(),
        display_name: Some("Principal".to_string()),
        roles: vec!["SCHOOL_ADMIN".to_string()],
        capabilities: vec![
            "attendance:view".to_string(),
            "attendance:bulk_edit".to_string(),
            "attendance:delete".to_string(),
        ],
        email_verified: true,
        phone_verified: true,
        school_status: "ACTIVE".to_string(),
        school_operable: true,
        school_onboarding_step: 4,
        is_class_teacher: false,
        has_assigned_class_teacher: false,
    };

    assert!(admin_user.is_school_admin());
    // Admin must NEVER have initial attendance marking
    assert!(!admin_user.has_capability("attendance:record"));
    assert!(!admin_user.has_capability("attendance:mark"));
    assert!(!admin_user.has_capability("attendance.mark"));

    // Admin has bulk edit and delete (both colon and dot notations)
    assert!(admin_user.has_capability("attendance:bulk_edit"));
    assert!(admin_user.has_capability("attendance.bulk_edit"));
    assert!(admin_user.has_capability("attendance:delete"));
    assert!(admin_user.has_capability("attendance.delete"));
    assert!(admin_user.has_capability("attendance:view"));
    assert!(admin_user.has_capability("attendance.view"));

    let teacher_user = User {
        id: Uuid::new_v4(),
        school_id: Uuid::new_v4(),
        username: "teacher_test".to_string(),
        password_hash: "".to_string(),
        display_name: Some("Teacher Anita".to_string()),
        roles: vec!["TEACHER".to_string()],
        capabilities: vec![
            "attendance:view".to_string(),
            "attendance:record".to_string(),
            "attendance:edit_subject".to_string(),
        ],
        email_verified: true,
        phone_verified: true,
        school_status: "ACTIVE".to_string(),
        school_operable: true,
        school_onboarding_step: 4,
        is_class_teacher: false,
        has_assigned_class_teacher: false,
    };

    assert!(!teacher_user.is_school_admin());
    // Teacher has mark capabilities in both colon and dot formats
    assert!(teacher_user.has_capability("attendance:record"));
    assert!(teacher_user.has_capability("attendance:mark"));
    assert!(teacher_user.has_capability("attendance.mark"));

    // Teacher has edit capability alias (maps to edit_subject/edit_class)
    assert!(teacher_user.has_capability("attendance:edit"));
    assert!(teacher_user.has_capability("attendance.edit"));
    assert!(teacher_user.has_capability("attendance:edit_subject"));
    assert!(teacher_user.has_capability("attendance.edit_subject"));

    // Teacher cannot bulk edit or delete
    assert!(!teacher_user.has_capability("attendance:bulk_edit"));
    assert!(!teacher_user.has_capability("attendance.bulk_edit"));
    assert!(!teacher_user.has_capability("attendance:delete"));
    assert!(!teacher_user.has_capability("attendance.delete"));
}

#[tokio::test]
async fn test_whatsapp_admin_blocked_and_teacher_subjects() {
    let _ = dotenvy::dotenv();
    let db_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => return,
    };

    let pool = match PgPool::connect(&db_url).await {
        Ok(p) => p,
        Err(_) => return,
    };

    let school_id_opt: Option<Uuid> = sqlx::query_scalar("SELECT id FROM schools LIMIT 1")
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

    let school_id = match school_id_opt {
        Some(s) => s,
        None => return,
    };

    // 1. Test Admin WhatsApp Session
    let admin_user_id_opt: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT u.id FROM users u
        JOIN user_roles ur ON ur.user_id = u.id
        JOIN roles r ON r.id = ur.role_id
        WHERE u.school_id = $1 AND r.code IN ('SUPER_ADMIN', 'SCHOOL_ADMIN')
        LIMIT 1
        "#,
    )
    .bind(school_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if let Some(admin_user_id) = admin_user_id_opt {
        let admin_jid = format!("admin_test_{}@s.whatsapp.net", Uuid::new_v4().simple());
        let admin_caps = CapabilityController::resolve_user_capabilities(
            &pool,
            school_id,
            Some(admin_user_id),
            &["SCHOOL_ADMIN".to_string()],
        )
        .await;

        let _admin_session = WhatsAppRepository::create_or_renew_session_with_meta(
            &pool,
            admin_user_id,
            school_id,
            &admin_jid,
            "919000000001",
            "SCHOOL_ADMIN",
            &admin_caps,
            1,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Admin asks "take attendance"
        let replies = WhatsAppMessageService::generate_reply_messages(
            &pool,
            &admin_jid,
            None,
            "take attendance",
        )
        .await;

        assert!(!replies.is_empty());
        let admin_reply = &replies[0];
        assert!(
            admin_reply.contains("School Administrators Cannot Mark Attendance"),
            "Admin must be strictly blocked from taking attendance! Got: {admin_reply}"
        );

        let _ = WhatsAppRepository::terminate_session(&pool, &admin_jid).await;
    }

    // 2. Test Teacher with assignments (meeraemp-103)
    let teacher_info = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        r#"
        SELECT u.id, s.id, u.username
        FROM users u
        JOIN staff s ON s.user_id = u.id
        WHERE u.username = 'meeraemp-103' AND u.school_id = $1
        LIMIT 1
        "#,
    )
    .bind(school_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if let Some((teacher_user_id, staff_id, _username)) = teacher_info {
        let teacher_jid = format!("teacher_test_{}@s.whatsapp.net", Uuid::new_v4().simple());
        let teacher_caps = CapabilityController::resolve_user_capabilities(
            &pool,
            school_id,
            Some(teacher_user_id),
            &["TEACHER".to_string(), "CLASS_TEACHER".to_string()],
        )
        .await;

        let _teacher_session = WhatsAppRepository::create_or_renew_session_with_meta(
            &pool,
            teacher_user_id,
            school_id,
            &teacher_jid,
            "919000000002",
            "TEACHER",
            &teacher_caps,
            1,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Check options directly from get_teacher_available_classes
        let options =
            get_teacher_available_classes(&pool, school_id, staff_id, teacher_user_id).await;
        assert!(
            !options.is_empty(),
            "Teacher meeraemp-103 must have assigned classes/subjects"
        );

        for opt in &options {
            assert!(
                !opt.subject_name.is_empty(),
                "Option display must include subject! Got: {}",
                opt.display
            );
            assert!(
                opt.display.contains(&opt.class_name),
                "Display must include class name"
            );
            assert!(
                opt.display.contains(&opt.subject_name),
                "Display must include subject name"
            );
        }

        let _ = WhatsAppRepository::terminate_session(&pool, &teacher_jid).await;
    }
}
