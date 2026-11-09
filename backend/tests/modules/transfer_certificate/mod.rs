use eravaya::modules::transfer_certificate::dto::{
    IssueReportPayload, IssueTransferCertificatesRequest, SaveTemplatePayload,
    UpdateSequencePayload,
};
use eravaya::modules::transfer_certificate::service::format_document_number;
use eravaya::utils::student_lifecycle::StudentLifecycleStatus;
use serde_json::json;
use uuid::Uuid;

#[test]
fn test_format_document_number_standard_tokens() {
    let current_year = chrono::Local::now().format("%Y").to_string();
    let formatted = format_document_number("{prefix}/{year}/{number}", "TC", "", 1, 4, None);
    assert_eq!(formatted, format!("TC/{current_year}/0001"));
}

#[test]
fn test_format_document_number_custom_academic_year() {
    let formatted = format_document_number(
        "{prefix}/{academic_year}/{number}",
        "TC",
        "",
        42,
        4,
        Some("2026-27"),
    );
    assert_eq!(formatted, "TC/2026-27/0042");
}

#[test]
fn test_format_document_number_padding_variations() {
    // 2-digit padding
    assert_eq!(
        format_document_number("{prefix}-{number}", "CERT", "", 7, 2, None),
        "CERT-07"
    );
    // 6-digit padding
    assert_eq!(
        format_document_number("{prefix}-{number}", "BC", "", 123, 6, None),
        "BC-000123"
    );
    // Large number exceeding padding length
    assert_eq!(
        format_document_number("{prefix}-{number}", "DOC", "", 99999, 4, None),
        "DOC-99999"
    );
}

#[test]
fn test_format_document_number_with_suffix() {
    let formatted =
        format_document_number("{prefix}/{number}{suffix}", "ERAVAYA", "-EXP", 15, 3, None);
    assert_eq!(formatted, "ERAVAYA/015-EXP");
}

#[test]
fn test_save_template_payload_serialization() {
    let payload = SaveTemplatePayload {
        template_type: Some("transfer_certificate".into()),
        name: Some("Custom TC Layout".into()),
        config: json!({
            "header": { "title": "TRANSFER CERTIFICATE" },
            "fields": ["name", "admission_no", "dob"]
        }),
        is_published: Some(true),
    };

    let serialized = serde_json::to_string(&payload).unwrap();
    assert!(serialized.contains("transfer_certificate"));
    assert!(serialized.contains("Custom TC Layout"));
    assert!(serialized.contains("is_published\":true"));
}

#[test]
fn test_issue_report_payload_deserialization() {
    let json_str = r#"{
        "report_type": "transfer_certificate",
        "student_ids": ["c1b18d2b-1111-2222-3333-444455556666"],
        "academic_year": "2026-2027",
        "remarks": "Issued on parent request",
        "custom_variables": { "conduct": "Good" }
    }"#;

    let payload: IssueReportPayload = serde_json::from_str(json_str).unwrap();
    assert_eq!(payload.report_type, "transfer_certificate");
    assert_eq!(payload.student_ids.len(), 1);
    assert_eq!(payload.academic_year.as_deref(), Some("2026-2027"));
    assert_eq!(payload.remarks.as_deref(), Some("Issued on parent request"));
}

#[test]
fn test_update_sequence_payload_partial_fields() {
    let payload = UpdateSequencePayload {
        display_name: Some("High School TC".into()),
        prefix: Some("HSTC".into()),
        suffix: None,
        current_number: Some(100),
        padding_length: Some(5),
        format_pattern: None,
        academic_year: None,
        is_locked: Some(false),
        reset_annually: Some(true),
    };

    assert_eq!(payload.prefix.as_deref(), Some("HSTC"));
    assert_eq!(payload.current_number, Some(100));
    assert_eq!(payload.is_locked, Some(false));
    assert_eq!(payload.reset_annually, Some(true));
}

#[test]
fn test_issue_transfer_certificates_request() {
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let req = IssueTransferCertificatesRequest {
        user_ids: vec![id1, id2],
        reason: Some("Family relocation".into()),
    };

    assert_eq!(req.user_ids.len(), 2);
    assert_eq!(req.reason.as_deref(), Some("Family relocation"));
}

#[test]
fn test_student_lifecycle_tc_transitions() {
    let active = StudentLifecycleStatus::Active;
    let tc_process = StudentLifecycleStatus::TcProcess;
    let tc_issued = StudentLifecycleStatus::TcIssued;
    let tc_cancelled = StudentLifecycleStatus::TcCancelled;

    // Active can transition to TcProcess
    assert!(active.can_transition_to(tc_process));

    // TcProcess can transition to TcIssued or TcCancelled
    assert!(tc_process.can_transition_to(tc_issued));
    assert!(tc_process.can_transition_to(tc_cancelled));

    // TcCancelled can transition back to Active
    assert!(tc_cancelled.can_transition_to(active));

    // TcIssued cannot directly jump back to Provisional
    assert!(!tc_issued.can_transition_to(StudentLifecycleStatus::Provisional));
}
