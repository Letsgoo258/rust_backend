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
