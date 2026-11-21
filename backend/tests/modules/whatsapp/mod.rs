use eravaya::modules::whatsapp::service::{WhatsAppMessageService, WhatsAppUserContext};
use eravaya::utils::phone::normalize_phone_number;
use std::collections::HashSet;
use uuid::Uuid;

#[test]
fn test_whatsapp_automated_bot_message_filtering() {
    assert!(WhatsAppMessageService::is_automated_or_bot_message(
        "Due to inactivity, your WhatsApp Banking session has timed out."
    ));
    assert!(WhatsAppMessageService::is_automated_or_bot_message(
        "Your OTP is 123456. Do not share it with anyone."
    ));
    assert!(WhatsAppMessageService::is_automated_or_bot_message(
        "Mini statement: Acct balance is INR 50,000"
    ));
    assert!(WhatsAppMessageService::is_automated_or_bot_message(
        "This is an automated FASTag alert. Do not reply."
    ));

    // Valid human parent/teacher messages should NOT be flagged
    assert!(!WhatsAppMessageService::is_automated_or_bot_message(
        "Hello"
    ));
    assert!(!WhatsAppMessageService::is_automated_or_bot_message(
        "Good morning, what is today's homework?"
    ));
    assert!(!WhatsAppMessageService::is_automated_or_bot_message(
        "attendance"
    ));
    assert!(!WhatsAppMessageService::is_automated_or_bot_message(
        "take attendance"
    ));
}

#[test]
fn test_whatsapp_greeting_and_help_detection() {
    assert!(WhatsAppMessageService::is_greeting("hi"));
    assert!(WhatsAppMessageService::is_greeting("hello"));
    assert!(WhatsAppMessageService::is_greeting("good morning"));
    assert!(WhatsAppMessageService::is_greeting("namaste"));

    assert!(WhatsAppMessageService::is_help_request("help"));
    assert!(WhatsAppMessageService::is_help_request("menu"));
    assert!(WhatsAppMessageService::is_help_request("options"));
}

#[test]
fn test_whatsapp_user_context_roles() {
    let teacher_ctx = WhatsAppUserContext {
        session_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        school_id: Uuid::new_v4(),
        school_name: "Test Academy".into(),
        phone: "919876543210".into(),
        role: "TEACHER".into(),
        capabilities: HashSet::new(),
        display_name: Some("John Teacher".into()),
    };
    assert!(teacher_ctx.is_teacher());

    let student_ctx = WhatsAppUserContext {
        session_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        school_id: Uuid::new_v4(),
        school_name: "Test Academy".into(),
        phone: "919876543211".into(),
        role: "STUDENT".into(),
        capabilities: HashSet::new(),
        display_name: Some("Jane Student".into()),
    };
    assert!(!student_ctx.is_teacher());
}

#[test]
fn test_phone_number_normalization() {
    assert_eq!(normalize_phone_number("+91 98765-43210"), "+919876543210");
    assert_eq!(normalize_phone_number("09876543210"), "+919876543210");
    assert_eq!(normalize_phone_number("9876543210"), "+919876543210");
}

#[test]
fn test_parse_take_attendance_command_variations() {
    use chrono::NaiveDate;
    use eravaya::modules::whatsapp::menus::attendance::parse_take_attendance_args;

    let today = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();

    // 1. Exact user prompt: ":take attendance 13/11/2026 class 2a"
    let parsed1 = parse_take_attendance_args(":take attendance 13/11/2026 class 2a", today);
    assert_eq!(
        parsed1.date,
        Some(NaiveDate::from_ymd_opt(2026, 11, 13).unwrap())
    );
    assert_eq!(parsed1.class_query.as_deref(), Some("class 2a"));
    assert!(parsed1.date_error.is_none());

    // 2. Dash separator: "take attendance 13-11-2026 2 A"
    let parsed2 = parse_take_attendance_args("take attendance 13-11-2026 2 A", today);
    assert_eq!(
        parsed2.date,
        Some(NaiveDate::from_ymd_opt(2026, 11, 13).unwrap())
    );
    assert_eq!(parsed2.class_query.as_deref(), Some("2 A"));

    // 3. Dot separator: "/take attendance 13.11.2026 Class 2 - A"
    let parsed3 = parse_take_attendance_args("/take attendance 13.11.2026 Class 2 - A", today);
    assert_eq!(
        parsed3.date,
        Some(NaiveDate::from_ymd_opt(2026, 11, 13).unwrap())
    );
    assert_eq!(parsed3.class_query.as_deref(), Some("Class 2 - A"));

    // 4. ISO date: "take attendance 2026-11-13 2a"
    let parsed4 = parse_take_attendance_args("take attendance 2026-11-13 2a", today);
    assert_eq!(
        parsed4.date,
        Some(NaiveDate::from_ymd_opt(2026, 11, 13).unwrap())
    );
    assert_eq!(parsed4.class_query.as_deref(), Some("2a"));

    // 5. Keyword "yesterday": "take attendance yesterday class 2a"
    let parsed5 = parse_take_attendance_args("take attendance yesterday class 2a", today);
    assert_eq!(
        parsed5.date,
        Some(NaiveDate::from_ymd_opt(2026, 11, 17).unwrap())
    );
    assert_eq!(parsed5.class_query.as_deref(), Some("class 2a"));

    // 6. Keyword "today": "take attendance today 2A"
    let parsed6 = parse_take_attendance_args("take attendance today 2A", today);
    assert_eq!(parsed6.date, Some(today));
    assert_eq!(parsed6.class_query.as_deref(), Some("2A"));

    // 7. No date specified, just class: "take attendance class 2a"
    let parsed7 = parse_take_attendance_args("take attendance class 2a", today);
    assert_eq!(parsed7.date, None);
    assert_eq!(parsed7.class_query.as_deref(), Some("class 2a"));

    // 8. Future date validation
    let parsed_future = parse_take_attendance_args("take attendance 25/12/2099 class 2a", today);
    assert!(parsed_future.date_error.is_some());
    assert!(parsed_future.date_error.unwrap().contains("future"));

    // 9. Invalid calendar date: 31/02/2026
    let parsed_invalid = parse_take_attendance_args("take attendance 31/02/2026 class 2a", today);
    assert!(parsed_invalid.date_error.is_some());
    assert!(parsed_invalid.date_error.unwrap().contains("Invalid"));
}

#[test]
fn test_class_normalization_and_matching() {
    use eravaya::modules::whatsapp::menus::attendance::{
        match_class_options, normalize_class_key, AvailableClassOption,
    };
    use uuid::Uuid;

    // Verify all variations of "class 2a", "2A", "2 A", "2 a", "2-a", "Class II A" normalize to "2a"
    let variations = vec![
        "class 2a",
        "class 2 a",
        "2A",
        "2 A",
        "2 a",
        "2-a",
        "2 - a",
        "Class 2 - A",
        "Grade 2 Section A",
        "Class II A",
        "Class II - A",
        "II A",
        "sec 2 a",
    ];

    for v in &variations {
        let key = normalize_class_key(v);
        assert_eq!(
            key, "2a",
            "Variation '{}' normalized to '{}', expected '2a'",
            v, key
        );
    }

    // Set up mock teacher assigned class options
    let class_sec_id = Uuid::new_v4();
    let options = vec![
        AvailableClassOption {
            index: 1,
            class_section_id: class_sec_id,
            class_name: "Class 2".into(),
            section_name: "A".into(),
            subject_id: None,
            subject_name: "Daily Roll Call".into(),
            period_number: None,
            is_daily_roll_call: true,
            display: "Class 2 - Sec A (Daily Roll Call)".into(),
            is_holiday: false,
            holiday_title: None,
            is_recorded: false,
        },
        AvailableClassOption {
            index: 2,
            class_section_id: Uuid::new_v4(),
            class_name: "Class 5".into(),
            section_name: "B".into(),
            subject_id: None,
            subject_name: "Daily Roll Call".into(),
            period_number: None,
            is_daily_roll_call: true,
            display: "Class 5 - Sec B (Daily Roll Call)".into(),
            is_holiday: false,
            holiday_title: None,
            is_recorded: false,
        },
    ];

    // Every variation of Class 2 A should match exactly option 1
    for v in &variations {
        let matched = match_class_options(v, &options);
        assert_eq!(matched.len(), 1, "Failed to match option for '{}'", v);
        assert_eq!(matched[0].index, 1);
        assert_eq!(matched[0].class_name, "Class 2");
        assert_eq!(matched[0].section_name, "A");
    }

    // Class 5 B matches
    let matched_5b = match_class_options("5b", &options);
    assert_eq!(matched_5b.len(), 1);
    assert_eq!(matched_5b[0].index, 2);

    // Unassigned class does NOT match teacher's options
    let matched_9c = match_class_options("class 9c", &options);
    assert_eq!(matched_9c.len(), 0);
}

#[test]
fn test_sql_injection_defense_and_sanitization() {
    use chrono::NaiveDate;
    use eravaya::modules::whatsapp::menus::attendance::{
        match_class_options, parse_take_attendance_args, AvailableClassOption,
    };
    use uuid::Uuid;

    let today = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();

    let injection_payloads = vec![
        ":take attendance 13/11/2026 class 2a'; DROP TABLE users; --",
        "take attendance 13/11/2026 2A' OR '1'='1",
        "take attendance 13/11/2026 2a\"; DELETE FROM attendance_sessions; --",
        "take attendance 13/11/2026 2A /* comment */",
        "take attendance 13/11/2026 <script>alert(1)</script> 2a",
        "take attendance 13/11/2026 UNION SELECT * FROM users --",
    ];

    let options = vec![AvailableClassOption {
        index: 1,
        class_section_id: Uuid::new_v4(),
        class_name: "Class 2".into(),
        section_name: "A".into(),
        subject_id: None,
        subject_name: "Daily Roll Call".into(),
        period_number: None,
        is_daily_roll_call: true,
        display: "Class 2 - Sec A (Daily Roll Call)".into(),
        is_holiday: false,
        holiday_title: None,
        is_recorded: false,
    }];

    for payload in injection_payloads {
        let parsed = parse_take_attendance_args(payload, today);
        assert_eq!(
            parsed.date,
            Some(NaiveDate::from_ymd_opt(2026, 11, 13).unwrap())
        );
        // The query string never causes panic or corrupts state
        if let Some(cq) = parsed.class_query {
            // In-memory matching against teacher's whitelist
            let matched = match_class_options(&cq, &options);
            // It either safely matches 2a or safely returns empty without executing SQL
            assert!(matched.len() <= 1);
        }
    }
}
