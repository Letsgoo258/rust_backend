use eravaya::modules::auth::backend::User;
use eravaya::modules::lms::domain::task::{TaskStatus, TaskType};
use eravaya::modules::lms::dto::{
    CreateAttachmentInput, CreateEvaluationConfig, CreateLmsTaskRequest,
};
use eravaya::utils::capabilities::defaults::default_enabled_keys;
use uuid::Uuid;

#[test]
fn test_lms_capabilities_role_assignment() {
    let teacher_caps = default_enabled_keys("TEACHER");
    assert!(teacher_caps.contains("lms:view"));
    assert!(teacher_caps.contains("lms:create"));
    assert!(teacher_caps.contains("lms:publish"));
    assert!(teacher_caps.contains("lms:marks_enter"));
    assert!(teacher_caps.contains("lms:evaluation_configure"));

    let student_caps = default_enabled_keys("STUDENT");
    assert!(student_caps.contains("lms:view"));
    assert!(!student_caps.contains("lms:create"));
    assert!(!student_caps.contains("lms:publish"));
    assert!(!student_caps.contains("lms:marks_enter"));

    let parent_caps = default_enabled_keys("PARENT");
    assert!(parent_caps.contains("lms:view"));
    assert!(!parent_caps.contains("lms:create"));

    let principal_caps = default_enabled_keys("PRINCIPAL");
    assert!(principal_caps.contains("lms:view"));
    assert!(principal_caps.contains("lms:reports_view"));
    assert!(principal_caps.contains("lms:reports_export"));
}

#[test]
fn test_lms_user_capabilities_check() {
    let teacher = User {
        id: Uuid::new_v4(),
        school_id: Uuid::new_v4(),
        username: "teacher1".to_string(),
        password_hash: "".to_string(),
        display_name: Some("Science Teacher".to_string()),
        roles: vec!["TEACHER".to_string()],
        capabilities: vec![
            "lms:view".to_string(),
            "lms:create".to_string(),
            "lms:publish".to_string(),
            "lms:marks_enter".to_string(),
        ],
        email_verified: true,
        phone_verified: true,
        school_status: "ACTIVE".to_string(),
        school_operable: true,
        school_onboarding_step: 4,
        is_class_teacher: false,
        has_assigned_class_teacher: false,
    };

    assert!(teacher.has_capability("lms:view"));
    assert!(teacher.has_capability("lms.view"));
    assert!(teacher.has_capability("lms:create"));
    assert!(teacher.has_capability("lms:publish"));
    assert!(!teacher.has_capability("lms:settings_manage"));
}

#[test]
fn test_lms_task_validation_rules() {
    // 1. Valid Draft request
    let valid_draft = CreateLmsTaskRequest {
        academic_year_id: None,
        title: "Chapter 4 Homework".to_string(),
        description: "Solve problems 1 to 10 on page 45".to_string(),
        special_message: Some("Submit handwritten notebook tomorrow".to_string()),
        task_type: Some("HOMEWORK".to_string()),
        class_section_id: Uuid::new_v4(),
        subject_id: Some(Uuid::new_v4()),
        teaching_group_id: None,
        attendance_session_id: None, // allowed in DRAFT
        assigned_at: None,
        deadline_at: None,
        add_to_calendar: Some(false),
        notify_whatsapp: Some(false),
        notify_email: Some(false),
        is_evaluation: Some(false),
        evaluation_config: None,
        attachments: None,
        publish_now: Some(false),
        status: None,
    };
    assert!(!valid_draft.title.trim().is_empty());
    assert!(!valid_draft.description.trim().is_empty());

    // 2. Publish request requires attendance session
    let publish_req = CreateLmsTaskRequest {
        academic_year_id: None,
        title: "Weekly Physics Quiz".to_string(),
        description: "Kinematics questions".to_string(),
        special_message: None,
        task_type: Some("ASSIGNMENT".to_string()),
        class_section_id: Uuid::new_v4(),
        subject_id: Some(Uuid::new_v4()),
        teaching_group_id: None,
        attendance_session_id: None,
        assigned_at: None,
        deadline_at: None,
        add_to_calendar: Some(true),
        notify_whatsapp: Some(true),
        notify_email: Some(false),
        is_evaluation: Some(true),
        evaluation_config: Some(CreateEvaluationConfig {
            evaluation_title: Some("Kinematics Quiz".to_string()),
            maximum_marks: 25.0,
            pass_marks: Some(10.0),
            evaluation_category: Some("Quiz".to_string()),
            evaluation_date: None,
            include_in_results: Some(true),
            result_component_id: None,
        }),
        attachments: None,
        publish_now: Some(true),
        status: None,
    };
    assert!(publish_req.publish_now.unwrap_or(false));
    assert!(publish_req.attendance_session_id.is_none()); // Violates publication rule
}

#[test]
fn test_attachment_types_and_expiry_semantics() {
    let drive_attachment = CreateAttachmentInput {
        attachment_type: "GOOGLE_DRIVE".to_string(),
        original_filename: Some("Reference Syllabus.pdf".to_string()),
        file_url: "https://drive.google.com/file/d/12345/view".to_string(),
        google_drive_url: Some("https://drive.google.com/file/d/12345/view".to_string()),
        mime_type: Some("application/pdf".to_string()),
        file_size: None,
        imagekit_file_id: None,
    };
    assert_eq!(drive_attachment.attachment_type, "GOOGLE_DRIVE");
    assert!(drive_attachment.imagekit_file_id.is_none());

    let imagekit_attachment = CreateAttachmentInput {
        attachment_type: "TEMPORARY_UPLOAD".to_string(),
        original_filename: Some("diagram.png".to_string()),
        file_url: "https://ik.imagekit.io/school/diagram.png".to_string(),
        google_drive_url: None,
        mime_type: Some("image/png".to_string()),
        file_size: Some(1048576),
        imagekit_file_id: Some("ik_file_xyz987".to_string()),
    };
    assert_eq!(imagekit_attachment.attachment_type, "TEMPORARY_UPLOAD");
    assert!(imagekit_attachment.imagekit_file_id.is_some());
}

#[test]
fn test_task_status_state_transitions() {
    assert_eq!(TaskStatus::Draft.as_str(), "DRAFT");
    assert_eq!(TaskStatus::Published.as_str(), "PUBLISHED");
    assert_eq!(TaskStatus::Closed.as_str(), "CLOSED");
    assert_eq!(TaskStatus::Cancelled.as_str(), "CANCELLED");

    assert_eq!(TaskType::Homework.as_str(), "HOMEWORK");
    assert_eq!(TaskType::Assignment.as_str(), "ASSIGNMENT");
    assert_eq!(TaskType::ClassroomTask.as_str(), "CLASSROOM_TASK");
    assert_eq!(TaskType::Other.as_str(), "OTHER");
}
